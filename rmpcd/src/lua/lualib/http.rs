use std::collections::HashMap;

use mlua::{ExternalResult, Lua, LuaSerdeExt, Table, Value};
use serde::{Deserialize, Serialize};
use tracing::{trace, warn};

pub fn create(lua: &Lua) -> mlua::Result<Table> {
    let tbl = lua.create_table()?;

    let call = lua.create_async_function(async |lua, (url, opts): (String, Option<Value>)| {
        let opts: RequestOpts =
            opts.map_or_else(|| Ok(RequestOpts::default()), |v| lua.from_value(v))?;

        do_call(lua, &url, Some(opts)).await
    })?;

    let get = lua.create_async_function(async |lua, (url, opts): (String, Option<Value>)| {
        let opts: GetOpts = opts.map_or_else(|| Ok(GetOpts::default()), |v| lua.from_value(v))?;

        do_call(lua, &url, Some(RequestOpts { method: Method::Get, ..opts.into() })).await
    })?;

    let post = lua.create_async_function(async |lua, (url, opts): (String, Option<Value>)| {
        let opts: PostOpts = opts.map_or_else(|| Ok(PostOpts::default()), |v| lua.from_value(v))?;

        do_call(lua, &url, Some(RequestOpts { method: Method::Post, ..opts.into() })).await
    })?;

    tbl.set("call", call)?;
    tbl.set("get", get)?;
    tbl.set("post", post)?;

    Ok(tbl)
}

async fn do_call(lua: Lua, url: &str, opts: Option<RequestOpts>) -> mlua::Result<Table> {
    let opts = opts.unwrap_or_default();
    let host = reqwest::Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(str::to_owned))
        .unwrap_or_default();
    trace!(method = ?opts.method, host = %host, "Sending HTTP request");

    let mut client = reqwest::Client::new().request(opts.method.into(), url);

    for (k, v) in opts.headers {
        client = client.header(k, v);
    }

    if !opts.params.is_empty() {
        client = client.query(&opts.params);
    }

    if let Some(body) = opts.body {
        client = client.body(body);
    }

    if let Some(ms) = opts.timeout_ms {
        client = client.timeout(std::time::Duration::from_millis(ms));
    }

    let result = lua.create_table()?;

    let response = match client.send().await {
        Ok(resp) => resp,
        Err(e) => {
            set_error(&result, &host, "HTTP request failed", e)?;
            return Ok(result);
        }
    };

    result.set("code", response.status().as_u16())?;

    if let Err(e) = response.error_for_status_ref() {
        set_error(&result, &host, "HTTP request failed", e)?;
    }

    let body = match response.text().await {
        Ok(bytes) => bytes,
        Err(e) => {
            set_error(&result, &host, "Failed to read response body", e)?;
            return Ok(result);
        }
    };

    result.raw_set("body", body)?;

    let json = lua.create_function(|lua, this: Table| {
        let body = &this.get::<String>("body")?;
        let json = serde_json::from_str::<serde_json::Value>(body).into_lua_err()?;
        lua.to_value(&json)
    })?;
    let text = lua.create_function(|_, this: Table| {
        let body = &this.get::<String>("body")?;
        Ok(body.clone())
    })?;

    result.set("json", json)?;
    result.set("text", text)?;

    Ok(result)
}

fn set_error(result: &Table, host: &str, context: &str, e: reqwest::Error) -> mlua::Result<()> {
    let e = e.without_url();
    warn!(host = %host, "{context}: {e}");

    if result.contains_key("error")? {
        return Ok(());
    }
    result.set("error", e.to_string())
}

#[derive(Default, Debug, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
enum Method {
    #[default]
    Get,
    Post,
    Put,
    Delete,
    Patch,
    Head,
    Options,
    Connect,
    Trace,
}

impl From<Method> for reqwest::Method {
    fn from(value: Method) -> Self {
        match value {
            Method::Get => reqwest::Method::GET,
            Method::Post => reqwest::Method::POST,
            Method::Put => reqwest::Method::PUT,
            Method::Delete => reqwest::Method::DELETE,
            Method::Patch => reqwest::Method::PATCH,
            Method::Head => reqwest::Method::HEAD,
            Method::Options => reqwest::Method::OPTIONS,
            Method::Connect => reqwest::Method::CONNECT,
            Method::Trace => reqwest::Method::TRACE,
        }
    }
}

#[derive(Default, Debug, Serialize, Deserialize)]
struct RequestOpts {
    method: Method,
    #[serde(default)]
    headers: HashMap<String, String>,
    body: Option<String>,
    #[serde(default)]
    params: HashMap<String, String>,
    timeout_ms: Option<u64>,
}

#[derive(Default, Debug, Serialize, Deserialize)]
struct GetOpts {
    #[serde(default)]
    headers: HashMap<String, String>,
    #[serde(default)]
    params: HashMap<String, String>,
    timeout_ms: Option<u64>,
}

#[derive(Default, Debug, Serialize, Deserialize)]
struct PostOpts {
    #[serde(default)]
    headers: HashMap<String, String>,
    body: Option<String>,
    #[serde(default)]
    params: HashMap<String, String>,
    timeout_ms: Option<u64>,
}

impl From<GetOpts> for RequestOpts {
    fn from(value: GetOpts) -> Self {
        Self {
            method: Method::Get,
            headers: value.headers,
            body: None,
            params: value.params,
            timeout_ms: value.timeout_ms,
        }
    }
}

impl From<PostOpts> for RequestOpts {
    fn from(value: PostOpts) -> Self {
        Self {
            method: Method::Post,
            headers: value.headers,
            body: value.body,
            params: value.params,
            timeout_ms: value.timeout_ms,
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use mlua::Lua;
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };

    async fn get_with_secret(url: String) -> (Option<u16>, Option<String>, Option<String>) {
        let lua = Lua::new();
        lua.globals().set("http", super::create(&lua).unwrap()).unwrap();
        lua.globals().set("url", url).unwrap();

        lua.load("local r = http.get(url, { params = { sk = 'secret' } }) return r.code, r.error, r.body")
            .eval_async()
            .await
            .unwrap()
    }

    async fn serve_once(response: &'static str) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/", listener.local_addr().unwrap());
        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let _ = stream.read(&mut [0; 4096]).await.unwrap();
            stream.write_all(response.as_bytes()).await.unwrap();
        });
        url
    }

    #[tokio::test]
    async fn error_status_keeps_body_and_hides_query() {
        let url = serve_once(
            "HTTP/1.1 403 Forbidden\r\nContent-Length: 11\r\nConnection: close\r\n\r\n{\"error\":9}",
        )
        .await;

        let (code, error, body) = get_with_secret(url).await;

        assert_eq!(code, Some(403));
        assert_eq!(body.as_deref(), Some(r#"{"error":9}"#));
        let error = error.expect("4xx should still set error");
        assert!(error.contains("403"), "error should describe the status: {error}");
        assert!(!error.contains("secret"), "error leaks the query string: {error}");
    }

    #[tokio::test]
    async fn failed_body_read_keeps_status_error() {
        let url =
            serve_once("HTTP/1.1 500 Internal Server Error\r\nContent-Length: 100\r\n\r\nshort")
                .await;

        let (code, error, body) = get_with_secret(url).await;

        assert_eq!(code, Some(500));
        assert_eq!(body, None);
        let error = error.expect("5xx should set error");
        assert!(error.contains("500"), "status error was overwritten: {error}");
    }

    #[tokio::test]
    async fn connection_error_hides_query() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/", listener.local_addr().unwrap());
        drop(listener);

        let (code, error, _) = get_with_secret(url).await;

        assert_eq!(code, None);
        let error = error.expect("connection failure should set error");
        assert!(!error.contains("secret"), "error leaks the query string: {error}");
    }
}
