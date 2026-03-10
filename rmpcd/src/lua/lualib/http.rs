use std::collections::HashMap;

use anyhow::Result;
use mlua::{ExternalResult, Lua, LuaSerdeExt, Table, Value};
use serde::{Deserialize, Serialize};
use tracing::error;

pub fn init(lua: &Lua) -> Result<()> {
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
    lua.globals().raw_set("http", tbl)?;

    Ok(())
}

async fn do_call(lua: Lua, url: &str, opts: Option<RequestOpts>) -> mlua::Result<Table> {
    let opts = opts.unwrap_or_default();
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

    let result = lua.create_table()?;

    let response = match client.send().await.and_then(|r| r.error_for_status()) {
        Ok(resp) => resp,
        Err(e) => {
            error!("HTTP request failed: {e}");
            result.set("error", e.to_string())?;
            result.set("code", e.status().map(|s| s.as_u16()))?;

            return Ok(result);
        }
    };

    result.set("code", response.status().as_u16())?;

    let body = match response.text().await {
        Ok(bytes) => bytes,
        Err(e) => {
            error!("Failed to read response body: {e}");
            result.set("error", e.to_string())?;
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
}

#[derive(Default, Debug, Serialize, Deserialize)]
struct GetOpts {
    #[serde(default)]
    headers: HashMap<String, String>,
    #[serde(default)]
    params: HashMap<String, String>,
}

#[derive(Default, Debug, Serialize, Deserialize)]
struct PostOpts {
    #[serde(default)]
    headers: HashMap<String, String>,
    body: Option<String>,
    #[serde(default)]
    params: HashMap<String, String>,
}

impl From<GetOpts> for RequestOpts {
    fn from(value: GetOpts) -> Self {
        Self { method: Method::Get, headers: value.headers, body: None, params: value.params }
    }
}

impl From<PostOpts> for RequestOpts {
    fn from(value: PostOpts) -> Self {
        Self {
            method: Method::Post,
            headers: value.headers,
            body: value.body,
            params: value.params,
        }
    }
}
