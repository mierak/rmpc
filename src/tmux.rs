use anyhow::{Context, Result};
use std::sync::LazyLock;

pub static IS_TMUX: LazyLock<bool> = LazyLock::new(|| {
    std::env::var("TMUX").is_ok_and(|v| !v.is_empty()) && std::env::var("TMUX_PANE").is_ok_and(|v| !v.is_empty())
});

static TMUX_PANE: LazyLock<String> = LazyLock::new(|| {
    std::env::var("TMUX_PANE").expect("TMUX_PANE environment variable to be defined when ran inside tmux")
});

pub fn is_inside_tmux() -> bool {
    *IS_TMUX
}

pub fn wrap_print_if_needed(input: &str) {
    if *IS_TMUX {
        print!("\x1bPtmux;");
        print!("{}", input.replace('\x1b', "\x1b\x1b"));
        print!("\x1b\\");
    } else {
        print!("{input}");
    }
}

pub fn wrap_print(input: &str) {
    print!("\x1bPtmux;");
    print!("{}", input.replace('\x1b', "\x1b\x1b"));
    print!("\x1b\\");
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

    Ok(stdout
        .parse::<u32>()
        .context("Invalid tmux response when querying session_attached")?
        > 0)
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
