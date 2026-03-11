use mlua::{Function, IntoLua, Lua, Table};
use tokio::select;
use tokio_util::sync::CancellationToken;
use tracing::{error, trace};

pub fn create(lua: &Lua) -> mlua::Result<Table> {
    let tbl = lua.create_table()?;

    let set_timeout = lua.create_async_function(
        async |_, args: (u64, Function)| -> mlua::Result<TimeoutHandle> {
            let token = CancellationToken::new();
            let sleep = tokio::time::sleep(tokio::time::Duration::from_millis(args.0));
            let handle = TimeoutHandle { token: token.clone() };

            tokio::spawn(async move {
                select! {
                    () = sleep => {
                        trace!("Timeout expired");
                        if let Err(err) = args.1.call_async::<()>(()).await {
                            error!(err = ?err, "Failed to call timeout callback");
                        }
                    }
                    () = token.cancelled() => {
                        trace!("Timeout cancelled");
                    }
                }
            });

            Ok(handle)
        },
    )?;

    let interval = lua.create_async_function(
        async |_, args: (u64, Function)| -> mlua::Result<TimeoutHandle> {
            let token = CancellationToken::new();
            let handle = TimeoutHandle { token: token.clone() };

            let handle_clone = handle.clone();
            tokio::spawn(async move {
                let mut interval =
                    tokio::time::interval(tokio::time::Duration::from_millis(args.0));
                loop {
                    select! {
                        _ = interval.tick() => {
                            trace!("Interval tick");
                            if let Err(err) = args.1.call_async::<()>(handle_clone.clone()).await {
                                error!(err = ?err, "Failed to call interval callback");
                            }
                        }
                        () = token.cancelled() => {
                            trace!("Interval cancelled");
                            break;
                        }
                    }
                }
            });

            Ok(handle)
        },
    )?;

    tbl.raw_set("set_timeout", set_timeout)?;
    tbl.raw_set("set_interval", interval)?;

    Ok(tbl)
}

#[derive(Clone)]
struct TimeoutHandle {
    token: CancellationToken,
}

impl IntoLua for TimeoutHandle {
    fn into_lua(self, lua: &Lua) -> mlua::Result<mlua::Value> {
        let tbl = lua.create_table()?;
        let token = self.token.clone();

        let cancel_fn = lua.create_function(move |_, ()| {
            token.cancel();
            Ok(())
        })?;

        tbl.set("cancel", cancel_fn)?;
        Ok(mlua::Value::Table(tbl))
    }
}
