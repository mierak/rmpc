use mlua::{ExternalResult, Function, IntoLua, IntoLuaMulti, Lua, Table, Value, Variadic};
use tokio::{select, sync::mpsc::UnboundedSender};
use tokio_util::sync::CancellationToken;
use tracing::trace;

use crate::{ext::SenderExt, lua::plugin::PluginEvent};

pub fn create(lua: &Lua) -> mlua::Result<Table> {
    let tbl = lua.create_table()?;

    let set_timeout = lua.create_async_function(
        async |lua, (timeout, func): (u64, Function)| -> mlua::Result<TimeoutHandle> {
            let token = CancellationToken::new();
            let sleep = tokio::time::sleep(tokio::time::Duration::from_millis(timeout));
            let handle = TimeoutHandle { token: token.clone() };

            let tx = lua
                .try_app_data_ref::<UnboundedSender<PluginEvent>>()
                .into_lua_err()?
                .expect("Expected plugin event sender to exist in lua")
                .clone();

            tokio::spawn(async move {
                select! {
                    () = sleep => {
                        trace!("Timeout expired");
                        tx.send_safe(PluginEvent::Callback { func, args: None });
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
        async |lua, (timeout, func): (u64, Function)| -> mlua::Result<TimeoutHandle> {
            let token = CancellationToken::new();
            let handle = TimeoutHandle { token: token.clone() };

            let handle_clone = handle.clone().into_lua_multi(&lua)?;

            let tx = lua
                .try_app_data_ref::<UnboundedSender<PluginEvent>>()
                .into_lua_err()?
                .expect("Expected plugin event sender to exist in lua")
                .clone();

            tokio::spawn(async move {
                let mut interval =
                    tokio::time::interval(tokio::time::Duration::from_millis(timeout));
                loop {
                    select! {
                        _ = interval.tick() => {
                            trace!("Interval tick");
                            tx.send_safe(PluginEvent::Callback {
                                func: func.clone(),
                                args: Some(handle_clone.clone())
                            });
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

    let debounce =
        lua.create_function(|lua, (timeout, func): (u64, Function)| -> mlua::Result<Function> {
            let tx = lua
                .try_app_data_ref::<UnboundedSender<PluginEvent>>()
                .into_lua_err()?
                .expect("Expected plugin event sender to exist in lua")
                .clone();

            let mut token: Option<CancellationToken> = None;

            lua.create_function_mut(move |lua, args: Variadic<Value>| -> mlua::Result<()> {
                if let Some(t) = token.take() {
                    t.cancel();
                }

                let new_token = CancellationToken::new();
                let sleep = tokio::time::sleep(tokio::time::Duration::from_millis(timeout));

                token = Some(new_token.clone());

                let func = func.clone();
                let tx = tx.clone();
                let args = if args.is_empty() { None } else { Some(args.into_lua_multi(lua)?) };

                tokio::spawn(async move {
                    select! {
                        () = sleep => {
                            trace!(?args, "Debounce fired");
                            tx.send_safe(PluginEvent::Callback { func, args });
                        }
                        () = new_token.cancelled() => {
                            trace!("Debounce cancelled");
                        }
                    }
                });

                Ok(())
            })
        })?;

    tbl.raw_set("set_timeout", set_timeout)?;
    tbl.raw_set("set_interval", interval)?;
    tbl.raw_set("debounce", debounce)?;

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
