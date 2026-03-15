use std::{collections::HashSet, fmt::Write};

use mlua::{Lua, Table, Value};
use tracing::info;

pub fn create(lua: &Lua) -> mlua::Result<Table> {
    let tbl = lua.create_table()?;

    let dump_table = lua.create_function(|_, tbl: Table| {
        let mut seen = HashSet::new();
        info!("{}", dump(&mlua::Value::Table(tbl), 0, &mut seen));
        Ok(())
    })?;

    let md5 = lua.create_function(|_, data: String| {
        let digest = md5::compute(data.as_bytes());
        Ok(format!("{digest:x}"))
    })?;

    let which = lua.create_function(|_, data: String| Ok(which::which(data).is_ok()))?;

    let nil_or_null = lua.create_function(|_, val: Value| Ok(val.is_nil() || val.is_null()))?;

    tbl.set("dump_table", dump_table)?;
    tbl.set("md5", md5)?;
    tbl.set("which", which)?;
    tbl.set("nil_or_null", nil_or_null)?;

    Ok(tbl)
}

pub fn dump(v: &Value, indent: usize, seen: &mut HashSet<usize>) -> String {
    match v {
        Value::Nil => "nil".into(),
        Value::Boolean(b) => b.to_string(),
        Value::Integer(i) => i.to_string(),
        Value::Number(n) => n.to_string(),
        Value::String(s) => {
            format!("{:?}", s.to_str().map(|v| v.to_string()).unwrap_or("<non-utf8>".to_string()))
        }
        Value::Table(t) => dump_table(t, indent, seen),
        Value::Function(_) => "<function>".into(),
        Value::Thread(_) => "<thread>".into(),
        Value::UserData(_) => "<userdata>".into(),
        Value::LightUserData(_) => "<lightuserdata>".into(),
        Value::Error(e) => format!("<error: {e}>"),
        Value::Other(val) => format!("<other: {val:?}>"),
    }
}

fn dump_table(t: &Table, indent: usize, seen: &mut HashSet<usize>) -> String {
    let id = t.to_pointer() as usize;
    if !seen.insert(id) {
        return "{<cycle>}".into();
    }

    let ind = "  ".repeat(indent);
    let ind2 = "  ".repeat(indent + 1);

    let mut out = String::from("{\n");
    for pair in t.clone().pairs::<Value, Value>() {
        let (k, v) = match pair {
            Ok(p) => p,
            Err(_) => continue,
        };
        out.push_str(&ind2);
        let _ = writeln!(out, "[{}] = {},", dump(&k, 0, seen), dump(&v, indent + 1, seen));
    }
    out.push_str(&ind);
    out.push('}');

    seen.remove(&id);
    out
}
