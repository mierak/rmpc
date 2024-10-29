use std::env;
use std::io::Cursor;

use anyhow::Context;
use anyhow::Result;
use crossterm::terminal::WindowSize;
use image::codecs::gif::GifDecoder;
use image::codecs::jpeg::JpegEncoder;
use image::AnimationDecoder;
use image::DynamicImage;
use image::ImageDecoder;
use rustix::path::Arg;

use crate::config::Size;

use super::dependencies::UEBERZUGPP;

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

pub struct GifData<'frames> {
    pub frames: image::Frames<'frames>,
    pub dimensions: (u32, u32),
}

pub fn get_gif_frames(data: &[u8]) -> Result<Option<GifData<'_>>> {
    // http://www.matthewflickinger.com/lab/whatsinagif/bits_and_bytes.asp
    if data.len() < 6 || data[0..6] != *b"GIF89a" {
        return Ok(None);
    }

    if GifDecoder::new(Cursor::new(data))?.into_frames().take(2).count() > 1 {
        let gif = GifDecoder::new(Cursor::new(data))?;
        Ok(Some(GifData {
            dimensions: gif.dimensions(),
            frames: gif.into_frames(),
        }))
    } else {
        Ok(None)
    }
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
