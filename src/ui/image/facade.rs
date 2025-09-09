use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use anyhow::Result;
use ratatui::layout::Rect;

use super::{
    Backend,
    block::Block,
    iterm2::Iterm2,
    kitty::Kitty,
    sixel::Sixel,
    ueberzug::{Layer, Ueberzug},
};
use crate::config::{Config, album_art::ImageMethod};

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
    Block(Block),
    #[default]
    None,
}

impl AlbumArtFacade {
    pub fn new(config: &Config) -> Self {
        let image_state = match config.album_art.method {
            ImageMethod::Kitty => ImageState::Kitty(Kitty::new(config.into())),
            ImageMethod::UeberzugWayland => ImageState::Ueberzug(Ueberzug::new(Layer::Wayland)),
            ImageMethod::UeberzugX11 => ImageState::Ueberzug(Ueberzug::new(Layer::X11)),
            ImageMethod::Iterm2 => ImageState::Iterm2(Iterm2::new(config.into())),
            ImageMethod::Sixel => ImageState::Sixel(Sixel::new(config.into())),
            ImageMethod::Block => ImageState::Block(Block::new(config.into())),
            ImageMethod::None | ImageMethod::Unsupported => ImageState::None,
        };
        Self {
            image_state,
            current_album_art: None,
            last_size: Rect::default(),
            default_album_art: Arc::new(config.theme.default_album_art.to_vec()),
        }
    }

    pub fn show_default(&mut self) -> Result<()> {
        self.current_album_art = Some(Arc::clone(&self.default_album_art));
        self.show_current()
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
            ImageState::Block(s) => s.show(data, self.last_size),
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
            ImageState::Block(s) => s.show(data, self.last_size),
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
            ImageState::Block(s) => s.hide(self.last_size)?,
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
            ImageState::Block(s) => Box::new(s).cleanup(self.last_size),
            ImageState::None => Ok(()),
        }
    }

    pub fn set_size(&mut self, area: Rect) {
        self.last_size = area;
    }

    pub fn set_config(&mut self, config: &Config) -> Result<()> {
        match &mut self.image_state {
            ImageState::Kitty(kitty) => kitty.set_config(config.into())?,
            ImageState::Ueberzug(ueberzug) => ueberzug.set_config(config.into())?,
            ImageState::Iterm2(iterm2) => iterm2.set_config(config.into())?,
            ImageState::Sixel(sixel) => sixel.set_config(config.into())?,
            ImageState::Block(block) => block.set_config(config.into())?,
            ImageState::None => {}
        }
        Ok(())
    }
}
