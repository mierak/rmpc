use std::sync::LazyLock;

use anyhow::Result;
use crossterm::{
    event::{
        DisableMouseCapture,
        EnableMouseCapture,
        KeyboardEnhancementFlags,
        PushKeyboardEnhancementFlags,
    },
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::prelude::Backend;

use crate::{
    config::album_art::{ImageMethod, ImageMethodFile},
    shared::{
        terminal::{crossterm_backend::CrosstermLockingBackend, tty::Tty},
        tmux::IS_TMUX,
    },
};

mod crossterm_backend;
mod emulator;
mod features;
mod tty;

pub use emulator::Emulator;
pub use features::ImageBackend;
pub use tty::{TtyReader, TtyWriter};

pub struct Terminal {
    tty: Tty,
    emulator: Emulator,
    kitty_keyboard_protocol: bool,
    kitty_graphics: LazyLock<bool>,
    sixel: LazyLock<bool>,
    ueberzug_x11: LazyLock<bool>,
    ueberzug_wayland: LazyLock<bool>,
}

pub static TERMINAL: LazyLock<Terminal> = LazyLock::new(Terminal::init);

#[allow(dead_code)]
impl Terminal {
    pub fn init() -> Self {
        let kitty_keyboard_protocol = features::detect_kitty_keyboard()
            .inspect_err(
                |err| log::error!(err:?; "Failed to determine kitty keyboard protocol support"),
            )
            .unwrap_or_default();
        let emulator = Emulator::detect()
            .inspect_err(|err| log::error!(err:?; "Failed to detect terminal emulator"))
            .unwrap_or_default();
        let sixel: LazyLock<_> = LazyLock::new(|| {
            features::detect_sixel()
                .inspect_err(|err| log::error!(err:?; "Failed to determine sixel support"))
                .unwrap_or_default()
        });
        let kitty_graphics: LazyLock<_> = LazyLock::new(|| {
            features::detect_kitty_graphics()
                .inspect_err(|err| log::error!(err:?; "Failed to determine kitty graphics support"))
                .unwrap_or_default()
        });

        let ueberzug_x11: LazyLock<bool> = LazyLock::new(features::detect_ueberzug_x11);
        let ueberzug_wayland: LazyLock<bool> = LazyLock::new(features::detect_ueberzug_wayland);

        Terminal {
            tty: Tty::new(),
            emulator,
            kitty_keyboard_protocol,
            kitty_graphics,
            sixel,
            ueberzug_x11,
            ueberzug_wayland,
        }
    }

    pub fn reader(&self) -> TtyReader {
        self.tty.reader()
    }

    pub fn writer(&self) -> TtyWriter {
        self.tty.writer()
    }

    pub fn emulator(&self) -> Emulator {
        self.emulator
    }

    pub fn ueberzug_x11(&self) -> bool {
        *self.ueberzug_x11
    }

    pub fn ueberzug_wayland(&self) -> bool {
        *self.ueberzug_wayland
    }

    pub fn resolve_image_backend(&self, requested_backend: ImageMethodFile) -> ImageMethod {
        let result = match requested_backend {
            ImageMethodFile::UeberzugWayland if self.ueberzug_wayland() => {
                ImageMethod::UeberzugWayland
            }
            ImageMethodFile::UeberzugWayland => ImageMethod::Unsupported,
            ImageMethodFile::UeberzugX11 if self.ueberzug_x11() => ImageMethod::UeberzugX11,
            ImageMethodFile::UeberzugX11 => ImageMethod::Unsupported,
            ImageMethodFile::Iterm2 => ImageMethod::Iterm2,
            ImageMethodFile::Kitty => ImageMethod::Kitty,
            ImageMethodFile::Sixel => ImageMethod::Sixel,
            ImageMethodFile::Block => ImageMethod::Block,
            ImageMethodFile::None => ImageMethod::None,
            ImageMethodFile::Auto => self.autodetect_image_backend().into(),
        };

        log::debug!(requested_backend:?, resolved_backend:? = result, tmux = *IS_TMUX; "Resolved image backend");

        result
    }

    pub fn autodetect_image_backend(&self) -> ImageBackend {
        use ImageBackend as B;

        let mut all_backends = vec![B::Kitty, B::Iterm2, B::Sixel];

        match self.emulator {
            // Konsole supports kitty but its implementation is incomplete and cannot work with
            // rmpc because the unicode placeholders support is missing.
            // Sixel and Iterm2 are also supported by Konsole but they have other issues like the
            // screen not clearing up properly.
            // This means that we cannot reliably support any of the preferred image backends and
            // have to rely on the fallback ones.
            Emulator::Konsole => all_backends.clear(),
            // These mostly support just Iterm2. Since Iterm2 does not have (to my knowledge) a
            // proper way to reliably test for support we have to explicitly list terminals that are
            // supposed to use Iterm2.
            Emulator::WezTerm => all_backends.retain(|b| matches!(b, B::Iterm2 | B::Sixel)),
            Emulator::VSCode => all_backends.retain(|b| matches!(b, B::Iterm2)),
            Emulator::Tabby => all_backends.retain(|b| matches!(b, B::Iterm2)),
            Emulator::Iterm2 => all_backends.retain(|b| matches!(b, B::Iterm2)),
            // All other terminals use full feature detection so Iterm2 should be removed from
            // tested backends.
            _ => all_backends.retain(|b| !matches!(b, B::Iterm2)),
        }

        // Ueberzugpp should be tested for for all terminals if no other backend was
        // found before it.
        all_backends.push(B::UeberzugWayland);
        all_backends.push(B::UeberzugX11);

        for backend in all_backends {
            if self.is_backend_supported(backend) {
                return backend;
            }
        }

        // Use Block as a fallback as that should work pretty much anywhere
        return ImageBackend::Block;
    }

    fn is_backend_supported(&self, backend: ImageBackend) -> bool {
        match backend {
            ImageBackend::Kitty => *self.kitty_graphics,
            // Iterm2 does not have feature deteciton, assume it is supported if it is asked for.
            ImageBackend::Iterm2 => true,
            ImageBackend::Sixel => *self.sixel,
            ImageBackend::UeberzugWayland => *self.ueberzug_wayland,
            ImageBackend::UeberzugX11 => *self.ueberzug_x11,
            // Block should be supported everywhere.
            ImageBackend::Block => true,
        }
    }

    pub fn restore<B: Backend + std::io::Write>(
        terminal: &mut ratatui::Terminal<B>,
        enable_mouse: bool,
    ) -> Result<()> {
        let mut writer = TERMINAL.writer();
        if enable_mouse {
            execute!(writer, DisableMouseCapture)?;
        }
        if TERMINAL.kitty_keyboard_protocol {
            execute!(
                writer,
                PushKeyboardEnhancementFlags(
                    KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
                        | KeyboardEnhancementFlags::REPORT_ALTERNATE_KEYS,
                )
            )?;
        }
        disable_raw_mode()?;
        execute!(writer, LeaveAlternateScreen)?;
        Ok(terminal.show_cursor()?)
    }

    pub fn setup(enable_mouse: bool) -> Result<ratatui::Terminal<CrosstermLockingBackend>> {
        enable_raw_mode()?;
        let mut writer = TERMINAL.writer();
        execute!(writer, EnterAlternateScreen)?;
        if enable_mouse {
            execute!(writer, EnableMouseCapture)?;
        }

        if TERMINAL.kitty_keyboard_protocol {
            execute!(
                writer,
                PushKeyboardEnhancementFlags(
                    KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
                        | KeyboardEnhancementFlags::REPORT_ALTERNATE_KEYS,
                )
            )?;
        }
        let mut terminal = ratatui::Terminal::new(CrosstermLockingBackend::new(writer))?;
        terminal.clear()?;
        Ok(terminal)
    }
}
