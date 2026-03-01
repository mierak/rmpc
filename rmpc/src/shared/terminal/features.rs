use anyhow::Result;
use rmpc_shared::env::ENV;

use crate::shared::{dependencies::UEBERZUGPP, terminal::tty::Tty, tmux::IS_TMUX};

pub(super) fn detect_kitty_keyboard() -> Result<bool> {
    let kitty_keyboard_protocol = if *IS_TMUX {
        false
    } else {
        Tty::query_device_attrs("\x1b[?u\x1b[0c")?.contains("\x1b[?0u")
    };
    log::debug!(kitty_keyboard_protocol :?; "Kitty keyboard protocol support");

    Ok(kitty_keyboard_protocol)
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, strum::VariantArray)]
pub enum ImageBackend {
    Kitty,
    Iterm2,
    Sixel,
    UeberzugWayland,
    UeberzugX11,
    #[default]
    Block,
}

pub(super) fn detect_ueberzug_wayland() -> bool {
    if !UEBERZUGPP.installed {
        return false;
    }

    if ENV.var("XDG_SESSION_TYPE").unwrap_or_default() == "wayland" {
        return true;
    }

    return ENV.var("WAYLAND_DISPLAY").is_ok_and(|v| !v.is_empty());
}

pub(super) fn detect_ueberzug_x11() -> bool {
    if !UEBERZUGPP.installed {
        return false;
    }

    if ENV.var("XDG_SESSION_TYPE").unwrap_or_default() == "x11" {
        return true;
    }

    return ENV.var("DISPLAY").is_ok_and(|v| !v.is_empty());
}

pub(super) fn detect_kitty_graphics() -> Result<bool> {
    let buf = Tty::query_device_attrs("\x1b_Gi=31,s=1,v=1,a=q,t=d,f=24;AAAA\x1b\\\x1b[c")?;
    let result = buf.contains("_Gi=31;OK");
    log::debug!(kitty_graphics: ? = result; "Kitty graphics protocol support");

    Ok(result)
}

pub(super) fn detect_sixel() -> Result<bool> {
    let buf = Tty::query_device_attrs("\x1b[c")?;
    let result = buf.contains(";4;") || buf.contains(";4c");
    log::debug!(sixel: ? = result; "Sixel graphics protocol support");

    Ok(result)
}
