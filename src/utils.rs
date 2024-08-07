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
        std::env::var("TERM_PROGRAM").is_ok_and(|v| v == "tmux")
            && std::env::var("TMUX").is_ok_and(|v| !v.is_empty())
            && std::env::var("TMUX_PANE").is_ok_and(|v| !v.is_empty())
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

    use super::tmux;
    use anyhow::Context;
    use anyhow::Result;
    use image::codecs::jpeg::JpegEncoder;
    use image::DynamicImage;

    #[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
    pub enum ImageProtocol {
        Kitty,
        UeberzugWayland,
        UeberzugX11,
        #[default]
        None,
        Iterm2,
    }

    pub fn determine_image_support() -> Result<ImageProtocol> {
        if is_kitty_supported()? {
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

    pub fn is_ueberzug_wayland_supported() -> bool {
        env::var("WAYLAND_DISPLAY").is_ok_and(|v| !v.is_empty())
    }
    pub fn is_ueberzug_x11_supported() -> bool {
        env::var("DISPLAY").is_ok_and(|v| !v.is_empty())
    }

    // todo
    pub fn is_iterm2_supported() -> bool {
        if env::var("TERM_PROGRAM").is_ok_and(|v| v == "WezTerm") {
            return true;
        }
        env::var("DISPLAY").is_ok_and(|v| !v.is_empty())
    }

    pub fn is_kitty_supported() -> anyhow::Result<bool> {
        let query = if tmux::is_inside_tmux() {
            if !tmux::is_passthrough_enabled()? {
                tmux::enable_passthrough()?;
            }
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

    pub fn get_image_size(area_width: usize, area_height: usize) -> Result<(u32, u32)> {
        let size = crossterm::terminal::window_size().context("Unable to query terminal size")?;
        let w = if size.width == 0 {
            800
        } else {
            let cell_width = size.width / size.columns;
            u32::try_from(cell_width as usize * area_width)?
        };
        let h = if size.height == 0 {
            600
        } else {
            let cell_height = size.height / size.rows;
            u32::try_from(cell_height as usize * area_height)?
        };
        Ok((w, h))
    }

    pub fn resize_image(image_data: &[u8], width_px: u32, hegiht_px: u32) -> Result<DynamicImage> {
        Ok(image::io::Reader::new(Cursor::new(image_data))
            .with_guessed_format()
            .context("Unable to guess image format")?
            .decode()
            .context("Unable to decode image")?
            .resize(width_px, hegiht_px, image::imageops::FilterType::Lanczos3))
    }

    pub fn jpg_encode(img: &DynamicImage) -> Result<Vec<u8>> {
        let mut jpg = Vec::new();
        JpegEncoder::new(&mut jpg).encode_image(img)?;
        Ok(jpg)
    }
}
