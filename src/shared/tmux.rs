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

pub fn environment() -> Result<Vec<(String, String)>> {
    if !*IS_TMUX {
        return Ok(Vec::new());
    }

    let mut cmd = std::process::Command::new("tmux");
    let cmd = cmd.args(["show-environment", "-g"]);
    let stdout = cmd.output()?.stdout;
    let stdout = String::from_utf8_lossy(&stdout);

    Ok(stdout
        .lines()
        .filter_map(|line| line.trim().split_once('=').map(|(k, v)| (k.to_owned(), v.to_owned())))
        .collect())
}

pub fn version() -> anyhow::Result<Option<String>> {
    if !*IS_TMUX {
        return Ok(None);
    }

    let mut cmd = std::process::Command::new("tmux");
    let cmd = cmd.args(["display-message", "-p", "-t", &TMUX_PANE, "-F", "#{version}"]);
    let stdout = cmd.output()?.stdout;
    let stdout = String::from_utf8(stdout)?;
    let stdout = stdout.trim();
    log::trace!(stdout; "got tmux version");

    Ok(Some(String::from_utf8_lossy(stdout.as_bytes()).to_string()))
}

pub fn is_passthrough_enabled() -> anyhow::Result<bool> {
    if !*IS_TMUX {
        return Ok(false);
    }

    let mut cmd = std::process::Command::new("tmux");
    let cmd = cmd.args(["show", "-Ap", "allow-passthrough"]);
    let stdout = cmd.output()?.stdout;

    Ok(String::from_utf8_lossy(&stdout).trim_end().ends_with("on"))
}

pub fn enable_passthrough() -> anyhow::Result<()> {
    if !*IS_TMUX {
        return Ok(());
    }

    let mut cmd = std::process::Command::new("tmux");
    let cmd = cmd.args(["set", "-p", "allow-passthrough"]);
    match cmd.output() {
        Ok(_) => Ok(()),
        Err(e) => Err(anyhow::anyhow!("Failed to enable tmux passthrough, '{e}'")),
    }
}

fn is_in_visible_pane() -> Result<bool> {
    let mut cmd = std::process::Command::new("tmux");
    let cmd = cmd.args(["display-message", "-p", "-t", &TMUX_PANE, "-F", "#F"]);
    let stdout = cmd.output()?.stdout;
    let stdout = String::from_utf8(stdout)?;
    let stdout = stdout.trim();
    log::trace!(stdout; "got tmux pane visibility");

    Ok(stdout.starts_with('*'))
}

#[derive(Debug)]
pub enum StatusBarPosition {
    Top,
    Bottom,
}

/// Returns the (x, y) position of the top-left corner of the current tmux pane,
/// adjusted for the status bar if it's enabled.
pub fn pane_position() -> Result<(u16, u16)> {
    let mut cmd = std::process::Command::new("tmux");
    let cmd = cmd.args([
        "display-message",
        "-p",
        "-t",
        &TMUX_PANE,
        "-F",
        "#{pane_left}|#{pane_top}|#{status}|#{status-position}",
    ]);
    let stdout = cmd.output()?.stdout;
    let stdout = String::from_utf8(stdout)?;
    let stdout = stdout.trim();
    log::trace!(stdout; "got tmux pane geometry");

    let mut parts = stdout.split('|');
    let x = parts
        .next()
        .ok_or_else(|| anyhow::anyhow!("Failed to get pane x from tmux"))?
        .parse::<u16>()?;
    let y = parts
        .next()
        .ok_or_else(|| anyhow::anyhow!("Failed to get pane top from tmux"))?
        .parse::<u16>()?;
    let enabled = parts
        .next()
        .ok_or_else(|| anyhow::anyhow!("Failed to get pane top from tmux"))
        .and_then(|enabled| match enabled {
        "on" => Ok(true),
        "off" => Ok(false),
        _ => Err(anyhow::anyhow!("Unknown enabled state '{enabled}' for tmux bar info.")),
    })?;
    let position = parts
        .next()
        .ok_or_else(|| anyhow::anyhow!("Failed to get pane top from tmux"))
        .and_then(|position| match position {
            "top" => Ok(StatusBarPosition::Top),
            "bottom" => Ok(StatusBarPosition::Bottom),
            _ => Err(anyhow::anyhow!("Unknown position '{position}' for tmux bar info.")),
        })?;

    let result =
        if enabled && matches!(position, StatusBarPosition::Top) { (x, y + 1) } else { (x, y) };

    log::debug!(x, y, stdout:?; "calculated tmux pane position");

    Ok(result)
}

fn session_has_attached_client() -> Result<bool> {
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
macro_rules! tmux_write_bytes {
    ( $w:ident, $data:ident ) => {{
        if *crate::tmux::IS_TMUX {
            for i in (0..$data.len()).rev() {
                if $data[i] == b"\x1b"[0] {
                    $data.insert(i, b"\x1b"[0]);
                }
            }
            $w.write("\x1bPtmux;".as_bytes())
                .and_then(|_| $w.write(&$data).and_then(|_| $w.write("\x1b\\".as_bytes())))
        } else {
            $w.write(&$data)
        }
    }};
}

pub(crate) use tmux_write;
pub(crate) use tmux_write_bytes;

use crate::try_skip;
