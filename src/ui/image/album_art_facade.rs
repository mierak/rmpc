use std::{
    hash::{DefaultHasher, Hash, Hasher},
    io::Write,
};

use anyhow::Result;
use ratatui::{layout::Rect, style::Color, widgets::Block, Frame};

use crate::{
    config::{Config, ImageMethod},
    utils::image_proto::ImageProtocol,
    AppEvent,
};

use super::{iterm2::Iterm2, kitty_image::KittyImageState, ImageProto};
use super::{
    kitty_image::KittyImage,
    ueberzug::{Layer, Ueberzug},
};

const UEBERZUG_ALBUM_ART_PATH: &str = "/tmp/rmpc/albumart";
const UEBERZUG_ALBUM_ART_DIR: &str = "/tmp/rmpc";

#[derive(Debug)]
pub struct AlbumArtFacade {
    image_state: ImageState,
    image_data: Option<Vec<u8>>,
    default_album_art: &'static [u8],
    image_data_hash: u64,
    needs_rerender: bool,
    last_size: Rect,
}

#[derive(Debug)]
enum ImageState {
    Kitty(KittyImageState),
    Ueberzug(Ueberzug),
    Iterm2(Iterm2),
    None,
}

impl AlbumArtFacade {
    pub fn new(
        protocol: ImageProtocol,
        default_album_art: &'static [u8],
        app_event_sender: std::sync::mpsc::Sender<AppEvent>,
    ) -> Self {
        Self {
            image_state: match protocol {
                ImageProtocol::Kitty => ImageState::Kitty(KittyImageState::new(app_event_sender, default_album_art)),
                ImageProtocol::UeberzugWayland => ImageState::Ueberzug(Ueberzug::new().init(Layer::Wayland)),
                ImageProtocol::UeberzugX11 => ImageState::Ueberzug(Ueberzug::new().init(Layer::X11)),
                ImageProtocol::Iterm2 => ImageState::Iterm2(Iterm2::new(app_event_sender, default_album_art)),
                ImageProtocol::None => ImageState::None,
            },
            image_data: None,
            image_data_hash: 0,
            needs_rerender: false,
            default_album_art,
            last_size: Rect::default(),
        }
    }

    pub fn transfer_image_data(&mut self, mut data: Option<Vec<u8>>) -> Result<()> {
        let mut hasher = DefaultHasher::new();
        data.hash(&mut hasher);
        let hash = hasher.finish();

        if hash == self.image_data_hash {
            return Ok(());
        }

        match &mut self.image_state {
            ImageState::Kitty(state) => {
                state.image(&mut data);
            }
            ImageState::Ueberzug(_state) => {
                std::fs::create_dir_all(UEBERZUG_ALBUM_ART_DIR)?;
                self.needs_rerender = true;
                let mut file = std::fs::OpenOptions::new()
                    .write(true)
                    .create(true)
                    .truncate(true)
                    .open(UEBERZUG_ALBUM_ART_PATH)?;
                if let Some(data) = &data {
                    file.write_all(data)?;
                } else {
                    file.write_all(self.default_album_art)?;
                }
            }
            ImageState::None => {}
            ImageState::Iterm2(iterm2) => iterm2.set_data(data.take()),
        }

        self.image_data = data;
        self.image_data_hash = hash;
        Ok(())
    }

    pub fn rerender_image(&mut self) {
        match &mut self.image_state {
            ImageState::Kitty(state) => {
                state.force_transfer();
            }
            ImageState::Ueberzug(_state) => {
                self.needs_rerender = true;
            }
            ImageState::None => {}
            ImageState::Iterm2(iterm2) => iterm2.show(),
        }
    }

    pub fn show(&mut self) {
        match &mut self.image_state {
            ImageState::Kitty(_) => {}
            ImageState::Ueberzug(_) => {}
            ImageState::Iterm2(iterm2) => iterm2.show(),
            ImageState::None => {}
        }
    }

    pub fn hide_image(&mut self, bg_color: Option<Color>) -> Result<()> {
        match &mut self.image_state {
            ImageState::Kitty(_state) => {}
            ImageState::Ueberzug(state) => {
                self.image_data_hash = 0;
                self.needs_rerender = false;
                state.remove_image()?;
            }
            ImageState::None => {}
            ImageState::Iterm2(iterm2) => iterm2.hide(bg_color, self.last_size)?,
        }
        Ok(())
    }

    pub fn render(&mut self, frame: &mut Frame, area: Rect, config: &Config) -> anyhow::Result<()> {
        self.last_size = area;
        match &mut self.image_state {
            ImageState::Kitty(state) => {
                frame.render_stateful_widget(
                    KittyImage::default().block(Block::default().border_style(config.as_border_style())),
                    area,
                    state,
                );
            }
            ImageState::Ueberzug(state) if self.needs_rerender => {
                state.show_image(UEBERZUG_ALBUM_ART_PATH, area.x, area.y, area.width, area.height)?;
                self.needs_rerender = false;
            }
            ImageState::Ueberzug(_) => {}
            ImageState::None => {}
            ImageState::Iterm2(iterm2) => {
                iterm2.render(area)?;
            }
        };
        Ok(())
    }

    pub(crate) fn handle_resize(&mut self, _columns: u16, _rows: u16) -> Result<()> {
        match &mut self.image_state {
            ImageState::Kitty(state) => {
                state.force_transfer();
                Ok(())
            }
            ImageState::Ueberzug(_state) => {
                self.needs_rerender = true;
                self.transfer_image_data(self.image_data.clone())
            }
            ImageState::None => Ok(()),
            ImageState::Iterm2(iterm2) => {
                iterm2.resize();
                Ok(())
            }
        }
    }

    pub(crate) fn cleanup(&mut self) -> Result<()> {
        match &mut self.image_state {
            ImageState::Kitty(_) => Ok(()),
            ImageState::Ueberzug(state) => state.cleanup(),
            ImageState::None => Ok(()),
            ImageState::Iterm2(_) => Ok(()),
        }
    }

    pub(crate) fn post_render(&mut self, frame: &mut Frame, config: &Config) -> std::result::Result<(), anyhow::Error> {
        match &mut self.image_state {
            ImageState::Kitty(_) => Ok(()),
            ImageState::Ueberzug(_) => Ok(()),
            ImageState::Iterm2(iterm2) => {
                iterm2.post_render(frame.buffer_mut(), config.theme.background_color, self.last_size)
            }
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
            ImageMethod::None => ImageProtocol::None,
            ImageMethod::Unsupported => ImageProtocol::None,
        }
    }
}
