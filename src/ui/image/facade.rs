use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use anyhow::Result;
use ratatui::layout::Rect;

use crate::config::album_art::ImageMethod;
use crate::config::Config;
use crate::shared::image::ImageProtocol;

use super::{iterm2::Iterm2, kitty::Kitty, Backend};
use super::{
    sixel::Sixel,
    ueberzug::{Layer, Ueberzug},
};

pub static IS_SHOWING: AtomicBool = AtomicBool::new(false);

#[derive(Debug)]
pub struct AlbumArtFacade {
    image_state: ImageState,
    current_album_art: Option<Arc<Vec<u8>>>,
    default_album_art: Arc<Vec<u8>>,
    last_size: Rect,
}

#[derive(Debug, Default)]
enum ImageState {
    Kitty(Kitty),
    Ueberzug(Ueberzug),
    Iterm2(Iterm2),
    Sixel(Sixel),
    #[default]
    None,
}

impl AlbumArtFacade {
    pub fn new(config: &Config) -> Self {
        let max_size = config.album_art.max_size_px;
        let bg_color = config.theme.background_color;
        let valign = config.album_art.vertical_align;
        let halign = config.album_art.horizontal_align;
        let proto = match config.album_art.method.into() {
            ImageProtocol::Kitty => ImageState::Kitty(Kitty::new(max_size, bg_color, halign, valign)),
            ImageProtocol::UeberzugWayland => ImageState::Ueberzug(Ueberzug::new(Layer::Wayland, max_size)),
            ImageProtocol::UeberzugX11 => ImageState::Ueberzug(Ueberzug::new(Layer::X11, max_size)),
            ImageProtocol::Iterm2 => ImageState::Iterm2(Iterm2::new(max_size, bg_color, halign, valign)),
            ImageProtocol::Sixel => ImageState::Sixel(Sixel::new(max_size, bg_color, halign, valign)),
            ImageProtocol::None => ImageState::None,
        };
        Self {
            image_state: proto,
            current_album_art: None,
            last_size: Rect::default(),
            default_album_art: Arc::new(config.theme.default_album_art.to_vec()),
        }
    }

    pub fn show_default(&mut self) -> Result<()> {
        self.current_album_art = None;
        IS_SHOWING.store(true, Ordering::Relaxed);

        let data = Arc::clone(&self.default_album_art);
        log::debug!(bytes = data.len(), area:? = self.last_size; "Displaying default image");

        match &mut self.image_state {
            ImageState::Kitty(kitty) => kitty.show(data, self.last_size),
            ImageState::Ueberzug(ueberzug) => ueberzug.show(data, self.last_size),
            ImageState::Iterm2(iterm2) => iterm2.show(data, self.last_size),
            ImageState::Sixel(s) => s.show(data, self.last_size),
            ImageState::None => Ok(()),
        }
    }

    pub fn show_current(&mut self) -> Result<()> {
        let Some(ref current_album_art) = self.current_album_art else {
            log::warn!("Tried to display current album art but none was present");
            return Ok(());
        };

        IS_SHOWING.store(true, Ordering::Relaxed);

        let data = Arc::clone(current_album_art);
        log::debug!(bytes = data.len(), area:? = self.last_size; "Displaying current image again",);

        match &mut self.image_state {
            ImageState::Kitty(kitty) => kitty.show(data, self.last_size),
            ImageState::Ueberzug(ueberzug) => ueberzug.show(data, self.last_size),
            ImageState::Iterm2(iterm2) => iterm2.show(data, self.last_size),
            ImageState::Sixel(s) => s.show(data, self.last_size),
            ImageState::None => Ok(()),
        }
    }

    pub fn show(&mut self, data: Vec<u8>) -> Result<()> {
        IS_SHOWING.store(true, Ordering::Relaxed);

        log::debug!(bytes = data.len(), area:? = self.last_size; "New image received",);
        let data = Arc::new(data);
        self.current_album_art = Some(Arc::clone(&data));

        match &mut self.image_state {
            ImageState::Kitty(kitty) => kitty.show(data, self.last_size),
            ImageState::Ueberzug(ueberzug) => ueberzug.show(data, self.last_size),
            ImageState::Iterm2(iterm2) => iterm2.show(data, self.last_size),
            ImageState::Sixel(s) => s.show(data, self.last_size),
            ImageState::None => Ok(()),
        }
    }

    pub fn hide(&mut self) -> Result<()> {
        IS_SHOWING.store(false, Ordering::Relaxed);
        match &mut self.image_state {
            ImageState::Kitty(kitty) => kitty.hide(self.last_size)?,
            ImageState::Ueberzug(ueberzug) => ueberzug.hide(self.last_size)?,
            ImageState::Iterm2(iterm2) => iterm2.hide(self.last_size)?,
            ImageState::Sixel(s) => s.hide(self.last_size)?,
            ImageState::None => {}
        }
        Ok(())
    }

    pub fn cleanup(&mut self) -> Result<()> {
        let state = std::mem::take(&mut self.image_state);
        IS_SHOWING.store(false, Ordering::Relaxed);
        match state {
            ImageState::Kitty(kitty) => Box::new(kitty).cleanup(self.last_size),
            ImageState::Ueberzug(ueberzug) => Box::new(ueberzug).cleanup(self.last_size),
            ImageState::Iterm2(iterm2) => Box::new(iterm2).cleanup(self.last_size),
            ImageState::Sixel(s) => Box::new(s).cleanup(self.last_size),
            ImageState::None => Ok(()),
        }
    }

    pub fn set_size(&mut self, area: Rect) {
        self.last_size = area;
    }
}

impl From<ImageMethod> for ImageProtocol {
    fn from(value: ImageMethod) -> Self {
        match value {
            ImageMethod::Kitty => ImageProtocol::Kitty,
            ImageMethod::UeberzugWayland => ImageProtocol::UeberzugWayland,
            ImageMethod::UeberzugX11 => ImageProtocol::UeberzugX11,
            ImageMethod::Iterm2 => ImageProtocol::Iterm2,
            ImageMethod::Sixel => ImageProtocol::Sixel,
            ImageMethod::None | ImageMethod::Unsupported => ImageProtocol::None,
        }
    }
}
