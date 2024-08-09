use itertools::Itertools;

pub mod macros {
    macro_rules! try_ret {
        ( $e:expr, $msg:literal ) => {
            match $e {
                Ok(x) => x,
                Err(e) => {
                    log::error!(error:? = e; $msg);
                    return Err(anyhow::anyhow!("Message: '{}', inner error: '{:?}'", $msg, e))
                },
            }
        };
    }
    macro_rules! try_cont {
        ( $e:expr, $msg:literal ) => {
            match $e {
                Ok(x) => x,
                Err(e) => {
                    log::warn!(error:? = e; $msg);
                    continue
                },
            }
        };
    }

    macro_rules! try_skip {
        ( $e:expr, $msg:literal ) => {
            match $e {
                Ok(_) => {},
                Err(e) => {
                    log::warn!(error:? = e; $msg);
                },
            }
        };
    }

    macro_rules! status_info {
        ($($t:tt)*) => {{
            log::info!($($t)*);
            log::info!(target: "{status_bar}", $($t)*);
        }};
    }

    macro_rules! status_error {
        ($($t:tt)*) => {{
            log::error!($($t)*);
            log::error!(target: "{status_bar}", $($t)*);
        }};
    }

    macro_rules! status_warn {
        ($($t:tt)*) => {{
            log::warn!($($t)*);
            log::warn!(target: "{status_bar}", $($t)*);
        }};
    }

    macro_rules! status_trace {
        ($($t:tt)*) => {{
            log::trace!($($t)*);
            log::trace!(target: "{status_bar}", $($t)*);
        }};
    }

    macro_rules! status_debug {
        ($($t:tt)*) => {{
            log::debug!($($t)*);
            log::debug!(target: "{status_bar}", $($t)*);
        }};
    }

    #[allow(unused_imports)]
    pub(crate) use status_debug;
    #[allow(unused_imports)]
    pub(crate) use status_error;
    #[allow(unused_imports)]
    pub(crate) use status_info;
    #[allow(unused_imports)]
    pub(crate) use status_trace;
    #[allow(unused_imports)]
    pub(crate) use status_warn;
    #[allow(unused_imports)]
    pub(crate) use try_cont;
    #[allow(unused_imports)]
    pub(crate) use try_ret;
    #[allow(unused_imports)]
    pub(crate) use try_skip;
}

pub trait ErrorExt {
    fn to_status(&self) -> String;
}

impl ErrorExt for anyhow::Error {
    fn to_status(&self) -> String {
        self.chain().map(|e| e.to_string().replace('\n', "")).join(" ")
    }
}

#[allow(dead_code)]
pub mod tmux {
    pub fn is_inside_tmux() -> bool {
        std::env::var("TMUX").is_ok_and(|v| !v.is_empty()) && std::env::var("TMUX_PANE").is_ok_and(|v| !v.is_empty())
    }

    pub fn wrap(input: &str) -> String {
        format!("\x1bPtmux;{},\x1b\\", input.replace('\x1b', "\x1b\x1b"))
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
}

pub mod image_proto {
    use std::env;
    use std::io::Cursor;

    use anyhow::Context;
    use anyhow::Result;
    use image::codecs::jpeg::JpegEncoder;
    use image::DynamicImage;
    use rustix::path::Arg;

    #[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
    pub enum ImageProtocol {
        Kitty,
        UeberzugWayland,
        UeberzugX11,
        #[default]
        None,
        Iterm2,
    }

    const ITERM2_TERMINAL_ENV_VARS: [&str; 3] = ["WEZTERM_EXECUTABLE", "TABBY_CONFIG_DIRECTORY", "VSCODE_INJECTION"];
    const ITERM2_TERM_PROGRAMS: [&str; 3] = ["WezTerm", "vscode", "Tabby"];

    pub fn determine_image_support(is_tmux: bool) -> Result<ImageProtocol> {
        if is_iterm2_supported(is_tmux) {
            return Ok(ImageProtocol::Iterm2);
        }

        if is_kitty_supported(is_tmux)? {
            return Ok(ImageProtocol::Kitty);
        };

        if which::which("ueberzugpp").is_ok() {
            let session_type = std::env::var("XDG_SESSION_TYPE");
            match session_type.unwrap_or_default().as_str() {
                "wayland" => return Ok(ImageProtocol::UeberzugWayland),
                "x11" => return Ok(ImageProtocol::UeberzugX11),
                _ => {
                    log::warn!("XDG_SESSION_TYPE not set, will check display variables.");
                    if is_ueberzug_wayland_supported() {
                        return Ok(ImageProtocol::UeberzugWayland);
                    }

                    if is_ueberzug_x11_supported() {
                        return Ok(ImageProtocol::UeberzugX11);
                    }
                }
            }
        }

        return Ok(ImageProtocol::None);
    }

    pub fn is_iterm2_supported(is_tmux: bool) -> bool {
        if is_tmux {
            if ITERM2_TERMINAL_ENV_VARS
                .iter()
                .any(|v| env::var_os(v).is_some_and(|v| !v.is_empty()))
            {
                return true;
            }
        } else if ITERM2_TERM_PROGRAMS
            .iter()
            .any(|v| env::var_os("TERM_PROGRAM").is_some_and(|var| var.as_str().unwrap_or_default().contains(v)))
        {
            return true;
        }
        return false;
    }

    pub fn is_ueberzug_wayland_supported() -> bool {
        env::var("WAYLAND_DISPLAY").is_ok_and(|v| !v.is_empty())
    }

    pub fn is_ueberzug_x11_supported() -> bool {
        env::var("DISPLAY").is_ok_and(|v| !v.is_empty())
    }

    pub fn is_kitty_supported(is_tmux: bool) -> anyhow::Result<bool> {
        let query = if is_tmux {
            "\x1bPtmux;\x1b\x1b_Gi=31,s=1,v=1,a=q,t=d,f=24;AAAA\x1b\x1b\\\x1b\x1b[c\x1b\\"
        } else {
            "\x1b_Gi=31,s=1,v=1,a=q,t=d,f=24;AAAA\x1b\\\x1b[c"
        };

        let stdin = rustix::stdio::stdin();
        let termios_orig = rustix::termios::tcgetattr(stdin)?;
        let mut termios = termios_orig.clone();

        termios.local_modes &= !rustix::termios::LocalModes::ICANON;
        termios.local_modes &= !rustix::termios::LocalModes::ECHO;
        // Set read timeout to 100ms as we cannot reliably check for end of terminal response
        termios.special_codes[rustix::termios::SpecialCodeIndex::VTIME] = 1;
        // Set read minimum to 0
        termios.special_codes[rustix::termios::SpecialCodeIndex::VMIN] = 0;

        rustix::termios::tcsetattr(stdin, rustix::termios::OptionalActions::Drain, &termios)?;

        rustix::io::write(rustix::stdio::stdout(), query.as_bytes())?;

        let mut buf = String::new();
        loop {
            let mut charbuffer = [0; 1];
            rustix::io::read(stdin, &mut charbuffer)?;

            buf.push(charbuffer[0].into());

            if charbuffer[0] == b'\0' || buf.ends_with(";c") {
                break;
            }
        }

        // Reset to previous attrs
        rustix::termios::tcsetattr(stdin, rustix::termios::OptionalActions::Now, &termios_orig)?;

        Ok(buf.contains("_Gi=31;OK"))
    }

    #[allow(dead_code)]
    pub fn read_size_csi() -> Result<Option<(u16, u16)>> {
        let stdin = rustix::stdio::stdin();
        let termios_orig = rustix::termios::tcgetattr(stdin)?;
        let mut termios = termios_orig.clone();

        termios.local_modes &= !rustix::termios::LocalModes::ICANON;
        termios.local_modes &= !rustix::termios::LocalModes::ECHO;
        // Set read timeout to 100ms as we cannot reliably check for end of terminal response
        termios.special_codes[rustix::termios::SpecialCodeIndex::VTIME] = 1;
        // Set read minimum to 0
        termios.special_codes[rustix::termios::SpecialCodeIndex::VMIN] = 0;

        rustix::termios::tcsetattr(stdin, rustix::termios::OptionalActions::Drain, &termios)?;

        let stdout = rustix::stdio::stdout();
        rustix::io::write(stdout, b"\x1b[14t")?;

        let mut buf = String::new();
        loop {
            let mut charbuffer = [0; 1];
            rustix::io::read(stdin, &mut charbuffer)?;

            buf.push(charbuffer[0].into());

            if charbuffer[0] == b'\0' || buf.ends_with('t') {
                break;
            }
        }

        // Reset to previous attrs
        rustix::termios::tcsetattr(stdin, rustix::termios::OptionalActions::Now, &termios_orig)?;

        let Some(buf) = buf.strip_prefix("\u{1b}[4;") else {
            return Ok(None);
        };

        let Some(buf) = buf.strip_suffix('t') else {
            return Ok(None);
        };

        if let Some((w, h)) = buf.split_once(';') {
            let w: u16 = w.parse()?;
            let h: u16 = h.parse()?;
            return Ok(Some((w, h)));
        }

        Ok(None)
    }

    pub fn get_image_size(
        area_width_col: usize,
        area_height_col: usize,
        max_width_px: u16,
        max_height_px: u16,
    ) -> Result<(u16, u16)> {
        let size = crossterm::terminal::window_size().context("Unable to query terminal size")?;

        // TODO: Figure out how to execute and read CSI sequences without it messing up crossterm

        // if size.width == 0 || size.height == 0 {
        //     if let Ok(Some((width, height))) = read_size_csi() {
        //         size.width = width;
        //         size.height = height;
        //     }
        // }

        let w = if size.width == 0 {
            max_width_px
        } else {
            let cell_width = size.width / size.columns;
            u16::try_from(cell_width as usize * area_width_col)?
        }
        .min(max_width_px);
        let h = if size.height == 0 {
            max_height_px
        } else {
            let cell_height = size.height / size.rows;
            u16::try_from(cell_height as usize * area_height_col)?
        }
        .max(max_height_px);
        // TODO calc correct max size

        log::debug!(size:?, area_width_col, area_height_col; "Resolved terminal size");
        Ok((w, h))
    }

    pub fn resize_image(image_data: &[u8], width_px: u16, hegiht_px: u16) -> Result<DynamicImage> {
        Ok(image::io::Reader::new(Cursor::new(image_data))
            .with_guessed_format()
            .context("Unable to guess image format")?
            .decode()
            .context("Unable to decode image")?
            .resize(
                u32::from(width_px),
                u32::from(hegiht_px),
                image::imageops::FilterType::Lanczos3,
            ))
    }

    pub fn jpg_encode(img: &DynamicImage) -> Result<Vec<u8>> {
        let mut jpg = Vec::new();
        JpegEncoder::new(&mut jpg).encode_image(img)?;
        Ok(jpg)
    }
}
