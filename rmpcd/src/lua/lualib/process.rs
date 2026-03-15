use mlua::{Lua, Table};
use tracing::{debug, error};

pub fn create(lua: &Lua) -> mlua::Result<Table> {
    let tbl = lua.create_table()?;

    let spawn_process = lua.create_async_function(
        async |_, cmd: Vec<String>| -> mlua::Result<(Option<u32>, Option<String>)> {
            let Some((program, args)) = cmd.split_first() else {
                return Ok((None, Some("No command provided".to_string())));
            };

            debug!(program = %program, args = ?args, "Running command");

            let mut child = tokio::process::Command::new(program)
                .args(args)
                .kill_on_drop(true)
                .spawn()
                .map_err(mlua::Error::external)?;
            let pid = child.id();

            let program = program.clone();
            tokio::task::spawn(async move {
                match child.wait().await {
                    Ok(status) => {
                        if !status.success() {
                            error!(
                                status = %status,
                                program = %program,
                                "Process exited with non-zero status"
                            );
                        }
                    }
                    Err(err) => {
                        error!(err = ?err, program = %program, "Failed to wait on child process");
                    }
                }
            });

            Ok((pid, None))
        },
    )?;

    tbl.raw_set("spawn", spawn_process)?;

    Ok(tbl)
}
