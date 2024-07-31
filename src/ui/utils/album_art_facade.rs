use std::{
    hash::{DefaultHasher, Hash, Hasher},
    io::Write,
};

use anyhow::Result;
use ratatui::{layout::Rect, widgets::Block, Frame};

use crate::{
    config::Config,
    ui::{
        utils::ueberzug::{Layer, Ueberzug},
        widgets::kitty_image::{KittyImage, KittyImageState},
    },
    utils::image_proto::ImageProtocol,
    AppEvent,
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
}

#[derive(Debug)]
enum ImageState {
    Kitty(KittyImageState),
    Ueberzug(Ueberzug),
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
                ImageProtocol::None => ImageState::None,
            },
            image_data: None,
            image_data_hash: 0,
            needs_rerender: false,
            default_album_art,
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
        }
    }

    pub fn hide_image(&mut self) -> Result<()> {
        match &mut self.image_state {
            ImageState::Kitty(_state) => {}
            ImageState::Ueberzug(state) => {
                self.image_data_hash = 0;
                self.needs_rerender = false;
                state.remove_image()?;
            }
            ImageState::None => {}
        }
        Ok(())
    }

    pub fn render(&mut self, frame: &mut Frame, area: Rect, config: &Config) -> anyhow::Result<()> {
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
        }
    }

    pub(crate) fn cleanup(&mut self) -> Result<()> {
        match &mut self.image_state {
            ImageState::Kitty(_) => Ok(()),
            ImageState::Ueberzug(state) => state.cleanup(),
            ImageState::None => Ok(()),
        }
    }
}
