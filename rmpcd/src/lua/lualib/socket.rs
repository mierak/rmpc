use std::{sync::Arc, time::Duration};

use mlua::{ExternalResult, IntoLuaMulti, Lua, Table, UserData, UserDataMethods, Value};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{UnixStream, unix::OwnedWriteHalf},
    select,
    sync::{Mutex, mpsc::UnboundedSender},
};
use tokio_util::sync::CancellationToken;
use tracing::{debug, trace};

use crate::{ext::SenderExt, lua::plugin::PluginEvent};

const READ_BUFFER_SIZE: usize = 64 * 1024;
const DEFAULT_WRITE_TIMEOUT: Duration = Duration::from_millis(5000);

struct UnixSocket {
    writer: Arc<Mutex<Option<OwnedWriteHalf>>>,
    token: CancellationToken,
    write_timeout: Duration,
}

impl Drop for UnixSocket {
    fn drop(&mut self) {
        self.token.cancel();
    }
}

impl UserData for UnixSocket {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_async_method("write", |lua, this, data: mlua::String| async move {
            let mut guard = this.writer.lock().await;
            let Some(writer) = guard.as_mut() else {
                return (false, Some("closed".to_string())).into_lua_multi(&lua);
            };

            match tokio::time::timeout(this.write_timeout, writer.write_all(&data.as_bytes())).await
            {
                Ok(Ok(())) => (true, Option::<String>::None).into_lua_multi(&lua),
                Ok(Err(err)) => (false, Some(err.to_string())).into_lua_multi(&lua),
                Err(_) => {
                    debug!(timeout = ?this.write_timeout, "Unix socket write timed out, closing");
                    this.token.cancel();
                    guard.take();
                    (false, Some("timeout".to_string())).into_lua_multi(&lua)
                }
            }
        });

        methods.add_async_method("close", |_, this, ()| async move {
            this.token.cancel();
            this.writer.lock().await.take();
            Ok(())
        });
    }
}

pub fn create(lua: &Lua) -> mlua::Result<Table> {
    let tbl = lua.create_table()?;

    let connect_unix =
        lua.create_async_function(async |lua, (path, handlers): (String, Table)| {
            let on_data: mlua::Function = handlers.get("on_data")?;
            let mut on_close: Option<mlua::Function> = handlers.get("on_close")?;
            let write_timeout = handlers
                .get::<Option<u64>>("write_timeout_ms")?
                .map_or(DEFAULT_WRITE_TIMEOUT, Duration::from_millis);

            let tx = lua
                .try_app_data_ref::<UnboundedSender<PluginEvent>>()
                .into_lua_err()?
                .expect("Expected plugin event sender to exist in lua")
                .clone();

            let stream = match UnixStream::connect(&path).await {
                Ok(stream) => stream,
                Err(err) => {
                    trace!(path, err = ?err, "Failed to connect to unix socket");
                    return (Value::Nil, Some(err.to_string())).into_lua_multi(&lua);
                }
            };

            let (mut read_half, write_half) = stream.into_split();
            let writer = Arc::new(Mutex::new(Some(write_half)));
            let token = CancellationToken::new();

            {
                let writer = Arc::clone(&writer);
                let token = token.clone();
                tokio::spawn(async move {
                    let mut buf = vec![0u8; READ_BUFFER_SIZE];
                    loop {
                        select! {
                            biased;
                            () = token.cancelled() => {
                                trace!("Unix socket reader cancelled");
                                break;
                            }
                            res = read_half.read(&mut buf) => match res {
                                Ok(0) => {
                                    trace!("Unix socket peer closed");
                                    if let Some(func) = on_close.take() {
                                        tx.send_safe(PluginEvent::SocketClosed {
                                            func,
                                            reason: None,
                                            token: token.clone(),
                                        });
                                    }
                                    break;
                                }
                                Ok(n) => {
                                    tx.send_safe(PluginEvent::SocketData {
                                        func: on_data.clone(),
                                        data: buf[..n].to_vec(),
                                        token: token.clone(),
                                    });
                                }
                                Err(err) => {
                                    debug!(err = ?err, "Unix socket read failed");
                                    if let Some(func) = on_close.take() {
                                        tx.send_safe(PluginEvent::SocketClosed {
                                            func,
                                            reason: Some(err.to_string()),
                                            token: token.clone(),
                                        });
                                    }
                                    break;
                                }
                            }
                        }
                    }
                    writer.lock().await.take();
                });
            }

            (UnixSocket { writer, token, write_timeout }, Option::<String>::None)
                .into_lua_multi(&lua)
        })?;

    tbl.raw_set("connect_unix", connect_unix)?;

    Ok(tbl)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::{
        path::{Path, PathBuf},
        sync::Arc,
        time::Duration,
    };

    use mlua::Lua;
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::UnixListener,
        sync::mpsc::UnboundedReceiver,
    };

    use crate::{
        async_client::AsyncClient,
        lua::{
            create,
            plugin::{LuaPlugin, PluginEvent},
        },
    };

    fn create_test_lua() -> (Lua, tokio::sync::mpsc::UnboundedReceiver<PluginEvent>) {
        let client = Arc::new(AsyncClient::new(|_| {}, || {}));
        let lua = create(None, &client, None).expect("test lua state");
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<PluginEvent>();
        lua.set_app_data(tx);
        (lua, rx)
    }

    const CONNECT: &str = r"
        received = {}
        closed = nil
        sock, err = socket.connect_unix(path, {
            on_data = function(d) table.insert(received, d) end,
            on_close = function(r) closed = { reason = r } end,
            write_timeout_ms = write_timeout_ms,
        })
        return sock ~= nil, err
    ";

    fn socket_path(name: &str) -> PathBuf {
        let path =
            std::env::temp_dir().join(format!("rmpcd-test-{}-{name}.sock", std::process::id()));
        let _ = std::fs::remove_file(&path);
        path
    }

    async fn connect(lua: &Lua, path: &Path) {
        connect_with(lua, path, None).await;
    }

    async fn connect_with(lua: &Lua, path: &Path, write_timeout_ms: Option<u64>) {
        lua.globals().set("path", path.to_string_lossy().to_string()).unwrap();
        lua.globals().set("write_timeout_ms", write_timeout_ms).unwrap();
        let (ok, err): (bool, Option<String>) = lua.load(CONNECT).eval_async().await.unwrap();
        assert!(ok, "connect failed: {err:?}");
    }

    async fn drain(
        lua: &Lua,
        state: &mlua::Table,
        rx: &mut UnboundedReceiver<PluginEvent>,
        silence_ms: u64,
    ) {
        while let Ok(Some(ev)) =
            tokio::time::timeout(Duration::from_millis(silence_ms), rx.recv()).await
        {
            LuaPlugin::handle_event("test", lua, state, ev).await.unwrap();
        }
    }

    #[tokio::test]
    async fn connect_unix_missing_path_returns_error() {
        let (lua, _rx) = create_test_lua();
        let (is_nil, has_err): (bool, bool) = lua
            .load("local s, e = socket.connect_unix('/nonexistent/rmpcd.sock', { on_data = function() end }) return s == nil, e ~= nil")
            .eval_async()
            .await
            .unwrap();
        assert!(is_nil && has_err);
    }

    #[tokio::test]
    async fn write_is_received_by_listener() {
        let path = socket_path("write");
        let listener = UnixListener::bind(&path).unwrap();
        let (lua, _rx) = create_test_lua();
        connect(&lua, &path).await;

        let (ok, err): (bool, Option<String>) =
            lua.load("return sock:write('hello')").eval_async().await.unwrap();
        assert!(ok, "write failed: {err:?}");

        let (mut stream, _) = listener.accept().await.unwrap();
        let mut buf = [0u8; 5];
        stream.read_exact(&mut buf).await.unwrap();
        assert_eq!(&buf, b"hello");
    }

    #[tokio::test]
    async fn listener_writes_arrive_as_on_data_events_in_order() {
        let path = socket_path("order");
        let listener = UnixListener::bind(&path).unwrap();
        let (lua, mut rx) = create_test_lua();
        let state = lua.create_table().unwrap();
        connect(&lua, &path).await;

        let (mut stream, _) = listener.accept().await.unwrap();
        stream.write_all(b"ab").await.unwrap();
        tokio::time::sleep(Duration::from_millis(20)).await;
        stream.write_all(b"cd").await.unwrap();
        drain(&lua, &state, &mut rx, 100).await;

        let joined: String = lua.load("return table.concat(received)").eval().unwrap();
        assert_eq!(joined, "abcd");
    }

    #[tokio::test]
    async fn peer_close_delivers_on_close_with_nil_reason() {
        let path = socket_path("peerclose");
        let listener = UnixListener::bind(&path).unwrap();
        let (lua, mut rx) = create_test_lua();
        let state = lua.create_table().unwrap();
        connect(&lua, &path).await;

        let (stream, _) = listener.accept().await.unwrap();
        drop(stream);
        drain(&lua, &state, &mut rx, 100).await;

        let closed_with_nil: bool =
            lua.load("return closed ~= nil and closed.reason == nil").eval().unwrap();
        assert!(closed_with_nil);
    }

    #[tokio::test]
    async fn explicit_close_delivers_no_on_close_and_write_fails() {
        let path = socket_path("close");
        let listener = UnixListener::bind(&path).unwrap();
        let (lua, mut rx) = create_test_lua();
        let state = lua.create_table().unwrap();
        connect(&lua, &path).await;

        let (_stream, _) = listener.accept().await.unwrap();
        lua.load("sock:close()").exec_async().await.unwrap();
        drain(&lua, &state, &mut rx, 100).await;

        let no_close: bool = lua.load("return closed == nil").eval().unwrap();
        assert!(no_close);
        let (ok, err): (bool, Option<String>) =
            lua.load("return sock:write('x')").eval_async().await.unwrap();
        assert!(!ok);
        assert_eq!(err.as_deref(), Some("closed"));
    }

    #[tokio::test]
    async fn write_times_out_when_peer_does_not_read_and_closes_socket() {
        let path = socket_path("write-timeout");
        let listener = UnixListener::bind(&path).unwrap();
        let (lua, mut rx) = create_test_lua();
        let state = lua.create_table().unwrap();
        connect_with(&lua, &path, Some(100)).await;
        let (_stream, _) = listener.accept().await.unwrap();

        let (ok, err): (bool, Option<String>) = tokio::time::timeout(
            Duration::from_secs(5),
            lua.load("return sock:write(string.rep('x', 4 * 1024 * 1024))").eval_async(),
        )
        .await
        .expect("write did not return within 5s")
        .unwrap();
        assert!(!ok);
        assert_eq!(err.as_deref(), Some("timeout"));

        let (ok, err): (bool, Option<String>) =
            lua.load("return sock:write('x')").eval_async().await.unwrap();
        assert!(!ok);
        assert_eq!(err.as_deref(), Some("closed"));

        drain(&lua, &state, &mut rx, 100).await;
        let no_close: bool = lua.load("return closed == nil").eval().unwrap();
        assert!(no_close);
    }
}
