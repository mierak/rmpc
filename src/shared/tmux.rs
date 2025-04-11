use std::sync::LazyLock;

use anyhow::Result;

pub static IS_TMUX: LazyLock<bool> = LazyLock::new(|| {
    std::env::var("TMUX").is_ok_and(|v| !v.is_empty())
        && std::env::var("TMUX_PANE").is_ok_and(|v| !v.is_empty())
});

static TMUX_PANE: LazyLock<String> = LazyLock::new(|| {
    std::env::var("TMUX_PANE")
        .expect("TMUX_PANE environment variable to be defined when ran inside tmux")
});

pub fn is_inside_tmux() -> bool {
    *IS_TMUX
}

#[must_use]
#[derive(Debug)]
pub(crate) struct TmuxHooks {
    commands: Vec<std::process::Command>,
    pub(crate) visible: bool,
}

impl Drop for TmuxHooks {
    fn drop(&mut self) {
        if !*IS_TMUX {
            return;
        }

        for command in &mut self.commands {
            try_skip!(command.spawn(), "Failed to uninstall tmux hook");
        }
    }
}

impl TmuxHooks {
    pub fn new() -> Result<Option<TmuxHooks>> {
        if !*IS_TMUX {
            return Ok(None);
        }

        log::debug!("in tmux installing hooks");
        let mut commands = Vec::new();

        let pid = std::process::id();
        let current_exe = std::env::current_exe()?;
        let current_exe = current_exe.to_string_lossy();

        for hook in ["session-window-changed", "client-attached", "client-session-changed"] {
            let mut cmd = std::process::Command::new("tmux");
            let cmd = cmd.args([
                "set-hook",
                "-a",
                "-t",
                &TMUX_PANE,
                &format!("{hook}[{pid}]"),
                &format!("run-shell '{current_exe} remote --pid {pid} tmux {hook}'"),
            ]);
            log::debug!(cmd:?; "installing hook");
            let stdout = cmd.output()?.stdout;
            log::debug!(stdout:?; "hook installed");

            let mut command = std::process::Command::new("tmux");
            command.args([
                "set-hook",
                "-u",
                "-t",
                &TMUX_PANE,
                &format!("{hook}[{pid}]"),
                "run-shell",
            ]);
            commands.push(command);
        }

        let mut cmd = std::process::Command::new("tmux");
        let cmd = cmd.args([
            "set-hook",
            "-a",
            "-g",
            &format!("client-session-changed[{pid}]"),
            &format!("run-shell '{current_exe} remote --pid {pid} tmux client-session-changed'",),
        ]);
        log::debug!(cmd:?; "installing hook");
        let stdout = cmd.output()?.stdout;
        log::debug!(stdout:?; "hook installed");
        let mut command = std::process::Command::new("tmux");
        command.args(["set-hook", "-gu", &format!("client-session-changed[{pid}]")]);
        commands.push(command);

        Ok(Some(TmuxHooks { commands, visible: true }))
    }

    pub fn update_visible(&mut self) -> Result<()> {
        let val = {
            if !is_inside_tmux() {
                true
            } else if !session_has_attached_client()? {
                false
            } else {
                is_in_visible_pane()?
            }
        };

        self.visible = val;

        Ok(())
    }
}

pub fn is_passthrough_enabled() -> anyhow::Result<bool> {
    let mut cmd = std::process::Command::new("tmux");
    let cmd = cmd.args(["show", "-Ap", "allow-passthrough"]);
    let stdout = cmd.output()?.stdout;

    Ok(String::from_utf8_lossy(&stdout).trim_end().ends_with("on"))
}

pub fn enable_passthrough() -> anyhow::Result<()> {
    let mut cmd = std::process::Command::new("tmux");
    let cmd = cmd.args(["set", "-p", "allow-passthrough"]);
    match cmd.output() {
        Ok(_) => Ok(()),
        Err(e) => Err(anyhow::anyhow!("Failed to enable tmux passthrough, '{e}'")),
    }
}

pub fn is_in_visible_pane() -> Result<bool> {
    let mut cmd = std::process::Command::new("tmux");
    let cmd = cmd.args(["display-message", "-p", "-t", &TMUX_PANE, "-F", "#F"]);
    let stdout = cmd.output()?.stdout;
    let stdout = String::from_utf8(stdout)?;
    let stdout = stdout.trim();
    log::trace!(stdout; "got tmux pane visibility");

    Ok(stdout.starts_with('*'))
}

pub fn session_has_attached_client() -> Result<bool> {
    let mut cmd = std::process::Command::new("tmux");
    let cmd = cmd.args(["list-panes", "-F", "#{session_attached}", "-t", &TMUX_PANE]);
    let stdout = cmd.output()?.stdout;
    let stdout = String::from_utf8(stdout)?;
    let stdout = stdout.trim();
    log::trace!(stdout; "got attached clients to tmux session");
    let sum: u32 =
        stdout.lines().try_fold(0, |acc, line| -> Result<_> { Ok(acc + line.parse::<u32>()?) })?;

    Ok(sum > 0)
}

/// Returns true when rmpc is ran inside Tmux but its pane is not in a visible
/// window or the session has no attached clients.
pub fn is_in_tmux_and_hidden() -> Result<bool> {
    if !*IS_TMUX {
        return Ok(false);
    }

    let is_in_visible_pane = is_in_visible_pane()?;
    if !is_in_visible_pane {
        log::trace!("rmpc is not in a visible Tmux pane");
        return Ok(true);
    }

    if !session_has_attached_client()? {
        log::trace!(is_in_visible_pane; "rmpc's tmux session has no attached clients");
        return Ok(true);
    }

    Ok(false)
}

/// [write!] except it wraps the given sequence in TMUX's pass through if tmux
/// is detected
macro_rules! tmux_write {
    ( $w:ident, $($t:tt)* ) => {{
        if *crate::tmux::IS_TMUX {
            write!($w, "\x1bPtmux;")
                .and_then(|()| {
                    write!($w, "{}", format!($($t)*).replace('\x1b', "\x1b\x1b"))
                        .and_then(|()| write!($w, "\x1b\\"))
            })
        } else {
            write!($w, $($t)*)
        }
    }}
}
pub(crate) use tmux_write;

use crate::try_skip;
