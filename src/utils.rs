use itertools::Itertools;

use crate::mpd::errors::MpdError;

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

impl ErrorExt for MpdError {
    fn to_status(&self) -> String {
        match self {
            MpdError::Parse(e) => format!("Failed to parse: {e}"),
            MpdError::UnknownCode(e) => format!("Unkown code: {e}"),
            MpdError::Generic(e) => format!("Generic error: {e}"),
            MpdError::ClientClosed => "Client closed".to_string(),
            MpdError::Mpd(e) => format!("MPD Error: {e}"),
            MpdError::ValueExpected(e) => format!("Expected Value but got '{e}'"),
            MpdError::UnsupportedMpdVersion(e) => format!("Unsuported MPD version: {e}"),
        }
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
    use crossterm::terminal::WindowSize;
    use image::codecs::jpeg::JpegEncoder;
    use image::DynamicImage;
    use rustix::path::Arg;

    use crate::config::Size;
    use crate::deps::UEBERZUGPP;

    #[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
    pub enum ImageProtocol {
        Kitty,
        UeberzugWayland,
        UeberzugX11,
        Iterm2,
        Sixel,
        #[default]
        None,
    }

    const ITERM2_TERMINAL_ENV_VARS: [&str; 3] = ["WEZTERM_EXECUTABLE", "TABBY_CONFIG_DIRECTORY", "VSCODE_INJECTION"];
    const ITERM2_TERM_PROGRAMS: [&str; 3] = ["WezTerm", "vscode", "Tabby"];

    pub fn determine_image_support(is_tmux: bool) -> Result<ImageProtocol> {
        if is_iterm2_supported(is_tmux) {
            return Ok(ImageProtocol::Iterm2);
        }

        match query_device_attrs(is_tmux)? {
            ImageProtocol::Kitty => return Ok(ImageProtocol::Kitty),
            ImageProtocol::Sixel => return Ok(ImageProtocol::Sixel),
            _ => {}
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

    pub fn query_device_attrs(is_tmux: bool) -> Result<ImageProtocol> {
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

        log::debug!(buf:?; "devattr response");

        if buf.contains("_Gi=31;OK") {
            return Ok(ImageProtocol::Kitty);
        } else if buf.contains(";4;") || buf.contains(";4c") {
            return Ok(ImageProtocol::Sixel);
        }
        Ok(ImageProtocol::None)
    }

    pub fn is_ueberzug_wayland_supported() -> bool {
        env::var("WAYLAND_DISPLAY").is_ok_and(|v| !v.is_empty()) && UEBERZUGPP.installed
    }

    pub fn is_ueberzug_x11_supported() -> bool {
        env::var("DISPLAY").is_ok_and(|v| !v.is_empty()) && UEBERZUGPP.installed
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

    pub fn get_image_area_size_px(area_width_col: u16, area_height_col: u16, max_size_px: Size) -> Result<(u16, u16)> {
        let size = crossterm::terminal::window_size().context("Unable to query terminal size")?;

        // TODO: Figure out how to execute and read CSI sequences without it messing up crossterm

        // if size.width == 0 || size.height == 0 {
        //     if let Ok(Some((width, height))) = read_size_csi() {
        //         size.width = width;
        //         size.height = height;
        //     }
        // }

        // TODO calc correct max size

        let (w, h) = clamp_image_size(&size, area_width_col, area_height_col, max_size_px);

        log::debug!(w, h, size:?; "Resolved terminal size");
        Ok((w, h))
    }

    pub fn resize_image(image_data: &[u8], width_px: u16, hegiht_px: u16) -> Result<DynamicImage> {
        Ok(image::ImageReader::new(Cursor::new(image_data))
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

    fn clamp_image_size(size: &WindowSize, area_width_col: u16, area_height_col: u16, max_size_px: Size) -> (u16, u16) {
        if size.width == 0 || size.height == 0 {
            return (max_size_px.width, max_size_px.height);
        }

        let cell_width = size.width / size.columns;
        let cell_height = size.height / size.rows;

        let w = cell_width * area_width_col;
        let h = cell_height * area_height_col;

        (w.min(max_size_px.width), h.min(max_size_px.height))
    }

    #[cfg(test)]
    mod test {
        use crossterm::terminal::WindowSize;
        use test_case::test_case;

        use crate::config::Size;

        use super::clamp_image_size;

        #[test_case(&WindowSize { width: 0, height: 0, columns: 10, rows: 10 }, 10, 10, Size { width: 500, height: 500 }, Size { width: 500, height: 500 }; "size not reported")]
        #[test_case(&WindowSize { width: 500, height: 500, columns: 10, rows: 10 }, 50, 10, Size { width: 500, height: 500 }, Size { width: 500, height: 500 }; "wider area")]
        #[test_case(&WindowSize { width: 500, height: 500, columns: 10, rows: 10 }, 10, 50, Size { width: 500, height: 500 }, Size { width: 500, height: 500 }; "taller area")]
        #[test_case(&WindowSize { width: 500, height: 500, columns: 10, rows: 10 }, 10, 10, Size { width: 5000, height: 500 }, Size { width: 500, height: 500 }; "square area")]
        fn uses_max_size_if_size_not_reported(
            window: &WindowSize,
            area_width: u16,
            area_height: u16,
            max_size: Size,
            expected: Size,
        ) {
            let (w, h) = clamp_image_size(window, area_width, area_height, max_size);

            assert_eq!(w, expected.width, "width not correct");
            assert_eq!(h, expected.height, "height not correct");
        }
    }
}

pub trait DurationExt {
    fn to_string(&self) -> String;
}

impl DurationExt for std::time::Duration {
    fn to_string(&self) -> String {
        let secs = self.as_secs();
        let min = secs / 60;
        format!("{}:{:0>2}", min, secs - min * 60)
    }
}

pub mod id {
    use std::sync::atomic::{AtomicUsize, Ordering};

    static LAST_ID: AtomicUsize = AtomicUsize::new(1);

    #[derive(Debug, derive_more::Deref, Clone, Copy, Eq, PartialEq, Hash)]
    pub struct Id(usize);

    pub fn new() -> Id {
        Id(LAST_ID.fetch_add(1, Ordering::Relaxed))
    }
}

pub mod percent {
    use anyhow::{anyhow, Context, Error};
    use derive_more::Into;
    use std::str::FromStr;

    #[derive(Debug, derive_more::Deref, Into, Clone, Copy, Eq, PartialEq)]
    #[into(u16, u32, u64, u128)]
    pub struct Percent(u16);

    impl FromStr for Percent {
        type Err = Error;

        fn from_str(s: &str) -> Result<Self, Self::Err> {
            Ok(Self(
                s.strip_suffix("%")
                    .context(anyhow!("Invalid percent format '{}'", s))?
                    .parse()
                    .context(anyhow!("Invalid percent format '{}'", s))?,
            ))
        }
    }
}

pub mod env {
    use std::{
        ffi::{OsStr, OsString},
        sync::LazyLock,
    };

    pub struct Env {
        #[cfg(test)]
        vars: std::sync::Arc<std::sync::Mutex<std::collections::HashMap<String, String>>>,
    }

    pub static ENV: LazyLock<Env> = LazyLock::new(|| Env {
        #[cfg(test)]
        vars: std::sync::Arc::new(std::sync::Mutex::new(std::collections::HashMap::default())),
    });

    #[cfg(not(test))]
    impl Env {
        pub fn var<K: AsRef<OsStr>>(&self, key: K) -> Result<String, std::env::VarError> {
            std::env::var(key)
        }

        pub fn var_os<K: AsRef<OsStr>>(&self, key: K) -> Option<OsString> {
            std::env::var_os(key)
        }
    }

    #[allow(clippy::unwrap_used)]
    #[cfg(test)]
    impl Env {
        pub fn var<K: AsRef<OsStr>>(&self, key: K) -> Result<String, std::env::VarError> {
            let Some(key) = key.as_ref().to_str() else {
                return Err(std::env::VarError::NotUnicode("".into()));
            };

            self.vars
                .lock()
                .unwrap()
                .get(key)
                .cloned()
                .ok_or(std::env::VarError::NotPresent)
        }

        pub fn var_os<K: AsRef<OsStr>>(&self, key: K) -> Option<OsString> {
            key.as_ref()
                .to_str()
                .and_then(|v| self.vars.lock().unwrap().get(v).cloned())
                .map(|v| v.into())
        }
        pub fn set(&self, key: String, value: String) {
            self.vars.lock().unwrap().insert(key, value);
        }

        pub fn clear(&self) {
            self.vars.lock().unwrap().clear();
        }

        pub fn remove(&self, key: &str) {
            self.vars.lock().unwrap().remove(key);
        }
    }
}

pub mod mpsc {
    #[allow(dead_code)]
    pub trait RecvLast<T> {
        fn recv_last(&self) -> Result<T, std::sync::mpsc::RecvError>;
        fn try_recv_last(&self) -> Result<T, std::sync::mpsc::TryRecvError>;
    }

    impl<T> RecvLast<T> for std::sync::mpsc::Receiver<T> {
        /// recv the last message in the channel and drop all the other ones
        fn recv_last(&self) -> Result<T, std::sync::mpsc::RecvError> {
            self.recv().map(|data| {
                let mut result = data;
                while let Ok(newer_data) = self.try_recv() {
                    result = newer_data;
                }
                result
            })
        }

        /// recv the last message in the channel in a non-blocking manner and drop all the other ones
        fn try_recv_last(&self) -> Result<T, std::sync::mpsc::TryRecvError> {
            self.try_recv().map(|data| {
                let mut result = data;
                while let Ok(newer_data) = self.try_recv() {
                    result = newer_data;
                }
                result
            })
        }
    }
}

pub mod mouse_event {
    use std::time::{Duration, Instant};

    use crossterm::event::{MouseEvent as CTMouseEvent, MouseEventKind};
    use ratatui::layout::Position;

    // maybe make the timout configurable?
    const DOUBLE_CLICK_TIMEOUT: Duration = Duration::from_millis(500);

    #[derive(Debug, Clone, Copy)]
    pub struct MouseEvent {
        pub x: u16,
        pub y: u16,
        pub kind: MouseEventKind,
    }

    #[derive(Debug, Clone, Copy)]
    pub struct TimedEvent<T: std::cmp::Eq> {
        data: T,
        time: Instant,
    }

    impl<T: std::cmp::Eq> TimedEvent<T> {
        pub fn new(data: T) -> Self {
            Self {
                data,
                time: Instant::now(),
            }
        }

        pub fn is_doubled(&self, data: &T) -> bool {
            if data != &self.data {
                return false;
            }

            self.time.elapsed() < DOUBLE_CLICK_TIMEOUT
        }
    }

    impl From<CTMouseEvent> for MouseEvent {
        fn from(value: CTMouseEvent) -> Self {
            Self {
                x: value.column,
                y: value.row,
                kind: value.kind,
            }
        }
    }

    impl From<MouseEvent> for Position {
        fn from(value: MouseEvent) -> Self {
            Self { x: value.x, y: value.y }
        }
    }
}
