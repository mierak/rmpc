use anyhow::Result;

use crate::shared::{
    env::ENV,
    terminal::tty::Tty,
    tmux::{self, IS_TMUX},
};

#[derive(Debug, Default, Clone, Copy, strum::Display, Eq, PartialEq)]
pub enum Emulator {
    Konsole,
    Ghostty,
    Kitty,
    Foot,
    WezTerm,
    VSCode,
    Iterm2,
    Tabby,
    #[default]
    Unknown,
}

impl Emulator {
    pub fn detect() -> Result<Emulator> {
        // XTVERSION - https://invisible-island.net/xterm/ctlseqs/ctlseqs.html
        let response = Tty::query_device_attrs("\x1b[>q")?;
        let from_dev_attr = if response.contains("ghostty") {
            Some(Emulator::Ghostty)
        } else if response.contains("Konsole") {
            Some(Emulator::Konsole)
        } else if response.contains("foot") {
            Some(Emulator::Foot)
        } else if response.contains("kitty") {
            Some(Emulator::Kitty)
        } else if response.contains("WezTerm") {
            Some(Emulator::WezTerm)
        } else if response.contains("iTerm2") {
            Some(Emulator::Iterm2)
        } else {
            None
        };

        if let Some(emul) = from_dev_attr {
            log::debug!(emul:?; "Detected terminal emulator from DA1 response");
            return Ok(emul);
        }

        let env = if *IS_TMUX {
            tmux::environment()?
                .into_iter()
                .find(|(k, _)| k == "TERM_PROGRAM")
                .map(|(_, v)| v)
                .unwrap_or_default()
        } else {
            ENV.var("TERM_PROGRAM").unwrap_or_default()
        };

        let term_program = match env.as_str() {
            "WezTerm" => Some(Emulator::WezTerm),
            "vscode" => Some(Emulator::VSCode),
            "Tabby" => Some(Emulator::Tabby),
            _ => None,
        };

        if let Some(emul) = term_program {
            log::debug!(emul:?, term_program:? = env; "Detected terminal emulator from TERM_PROGRAM");
            return Ok(emul);
        }

        let term_env = if !ENV.var_os("WEZTERM_EXECUTABLE").unwrap_or_default().is_empty() {
            Some(Emulator::WezTerm)
        } else if !ENV.var_os("TABBY_CONFIG_DIRECTORY").unwrap_or_default().is_empty() {
            Some(Emulator::Tabby)
        } else if !ENV.var_os("VSCODE_INJECTION").unwrap_or_default().is_empty() {
            Some(Emulator::VSCode)
        } else {
            None
        };

        if let Some(emul) = term_env {
            log::debug!(emul:?; "Detected terminal emulator from terminal specific env variable");
            return Ok(emul);
        }

        log::debug!("Unknown terminal emulator");
        Ok(Emulator::Unknown)
    }
}
