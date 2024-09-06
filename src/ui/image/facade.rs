use std::hash::{DefaultHasher, Hash, Hasher};

use anyhow::Result;
use ratatui::{layout::Rect, style::Color, Frame};

use crate::{
    config::{Config, ImageMethod, Size},
    utils::image_proto::ImageProtocol,
};

use super::{iterm2::Iterm2, kitty::KittyImageState, ImageProto};
use super::{
    sixel::Sixel,
    ueberzug::{Layer, Ueberzug},
};

#[derive(Debug)]
pub struct AlbumArtFacade {
    image_state: ImageState,
    image_data: Option<Vec<u8>>,
    image_data_hash: u64,
    last_size: Rect,
}

#[derive(Debug)]
enum ImageState {
    Kitty(KittyImageState),
    Ueberzug(Ueberzug),
    Iterm2(Iterm2),
    Sixel(Sixel),
    None,
}

impl AlbumArtFacade {
    pub fn new(
        protocol: ImageProtocol,
        default_album_art: &'static [u8],
        max_size: Size,
        request_render: impl Fn(bool) + Send + 'static,
    ) -> Self {
        let proto = match protocol {
            ImageProtocol::Kitty => {
                ImageState::Kitty(KittyImageState::new(default_album_art, max_size, request_render))
            }
            ImageProtocol::UeberzugWayland => {
                ImageState::Ueberzug(Ueberzug::new(default_album_art, Layer::Wayland, max_size))
            }
            ImageProtocol::UeberzugX11 => ImageState::Ueberzug(Ueberzug::new(default_album_art, Layer::X11, max_size)),
            ImageProtocol::Iterm2 => ImageState::Iterm2(Iterm2::new(default_album_art, max_size, request_render)),
            ImageProtocol::Sixel => ImageState::Sixel(Sixel::new(default_album_art, max_size, request_render)),
            ImageProtocol::None => ImageState::None,
        };
        Self {
            image_state: proto,
            image_data: None,
            image_data_hash: 0,
            last_size: Rect::default(),
        }
    }

    pub fn set_image(&mut self, mut data: Option<Vec<u8>>) -> Result<()> {
        let mut hasher = DefaultHasher::new();
        data.hash(&mut hasher);
        self.last_size.hash(&mut hasher);
        let hash = hasher.finish();

        if hash == self.image_data_hash {
            return Ok(());
        }

        match &mut self.image_state {
            ImageState::Kitty(state) => state.set_data(data.take())?,
            ImageState::Ueberzug(ueberzug) => ueberzug.set_data(data.take())?,
            ImageState::Iterm2(iterm2) => iterm2.set_data(data.take())?,
            ImageState::Sixel(s) => s.set_data(data.take())?,
            ImageState::None => {}
        }

        self.image_data = data;
        self.image_data_hash = hash;
        Ok(())
    }

    pub fn show(&mut self) {
        match &mut self.image_state {
            ImageState::Kitty(kitty) => kitty.show(),
            ImageState::Ueberzug(ueberzug) => ueberzug.show(),
            ImageState::Iterm2(iterm2) => iterm2.show(),
            ImageState::Sixel(s) => s.show(),
            ImageState::None => {}
        }
    }

    pub fn hide(&mut self, bg_color: Option<Color>) -> Result<()> {
        match &mut self.image_state {
            ImageState::Kitty(kitty) => kitty.hide(bg_color, self.last_size)?,
            ImageState::Ueberzug(ueberzug) => ueberzug.hide(bg_color, self.last_size)?,
            ImageState::Iterm2(iterm2) => iterm2.hide(bg_color, self.last_size)?,
            ImageState::Sixel(s) => s.hide(bg_color, self.last_size)?,
            ImageState::None => {}
        }
        Ok(())
    }

    pub fn render(&mut self, frame: &mut Frame, area: Rect, _config: &Config) -> anyhow::Result<()> {
        self.last_size = area;
        match &mut self.image_state {
            ImageState::Kitty(state) => state.render(frame.buffer_mut(), area)?,
            ImageState::Ueberzug(state) => state.render(frame.buffer_mut(), area)?,
            ImageState::Iterm2(iterm2) => iterm2.render(frame.buffer_mut(), area)?,
            ImageState::Sixel(s) => s.render(frame.buffer_mut(), area)?,
            ImageState::None => {}
        };
        Ok(())
    }

    pub fn resize(&mut self, _columns: u16, _rows: u16) {
        match &mut self.image_state {
            ImageState::Kitty(state) => state.resize(),
            ImageState::Ueberzug(ueberzug) => ueberzug.resize(),
            ImageState::Iterm2(iterm2) => iterm2.resize(),
            ImageState::Sixel(s) => s.resize(),
            ImageState::None => {}
        }
    }

    pub fn cleanup(&mut self) -> Result<()> {
        let state = std::mem::replace(&mut self.image_state, ImageState::None);
        match state {
            ImageState::Kitty(kitty) => Box::new(kitty).cleanup(),
            ImageState::Ueberzug(ueberzug) => Box::new(ueberzug).cleanup(),
            ImageState::Iterm2(iterm2) => Box::new(iterm2).cleanup(),
            ImageState::Sixel(s) => Box::new(s).cleanup(),
            ImageState::None => Ok(()),
        }
    }

    pub fn post_render(&mut self, frame: &mut Frame, config: &Config) -> std::result::Result<(), anyhow::Error> {
        match &mut self.image_state {
            ImageState::Kitty(kitty) => {
                kitty.post_render(frame.buffer_mut(), config.theme.background_color, self.last_size)
            }
            ImageState::Ueberzug(ueberzug) => {
                ueberzug.post_render(frame.buffer_mut(), config.theme.background_color, self.last_size)
            }
            ImageState::Iterm2(iterm2) => {
                iterm2.post_render(frame.buffer_mut(), config.theme.background_color, self.last_size)
            }
            ImageState::Sixel(s) => s.post_render(frame.buffer_mut(), config.theme.background_color, self.last_size),
            ImageState::None => Ok(()),
        }
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
            ImageMethod::None => ImageProtocol::None,
            ImageMethod::Unsupported => ImageProtocol::None,
        }
    }
}
