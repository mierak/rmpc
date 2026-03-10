use mlua::{Lua, Table};
use tracing::error;

pub fn create(lua: &Lua) -> mlua::Result<Table> {
    let tbl = lua.create_table()?;

    let spawn_process = lua.create_async_function(
        async |_, cmd: Vec<String>| -> mlua::Result<(Option<u32>, Option<String>)> {
            let Some(first) = cmd.first().cloned() else {
                return Ok((None, Some("No command provided".to_string())));
            };

            let mut child = tokio::process::Command::new(&first)
                .args(cmd[1..].iter())
                .kill_on_drop(true)
                .spawn()
                .map_err(mlua::Error::external)?;
            let pid = child.id();

            tokio::task::spawn(async move {
                match child.wait().await {
                    Ok(status) => {
                        if !status.success() {
                            error!(
                                status = %status,
                                program = %first,
                                "Process exited with non-zero status"
                            );
                        }
                    }
                    Err(err) => {
                        error!(err = ?err, program = %first, "Failed to wait on child process");
                    }
                }
            });

            Ok((pid, None))
        },
    )?;

    tbl.raw_set("spawn", spawn_process)?;

    Ok(tbl)
}
