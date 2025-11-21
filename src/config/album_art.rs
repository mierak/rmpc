use serde::{Deserialize, Serialize};
use strum::Display;

use super::Size;
use crate::{mpd::mpd_client::AlbumArtOrder, shared::terminal::ImageBackend};

#[derive(Debug, Default, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct AlbumArtConfigFile {
    #[serde(default)]
    pub method: ImageMethodFile,
    #[serde(default)]
    pub order: AlbumArtOrderFile,
    #[serde(default)]
    pub max_size_px: Size,
    #[serde(default = "super::defaults::disabled_album_art_protos")]
    pub disabled_protocols: Vec<String>,
    #[serde(default)]
    pub vertical_align: VerticalAlignFile,
    #[serde(default)]
    pub horizontal_align: HorizontalAlignFile,
}

#[derive(Debug, Default, Clone)]
pub struct AlbumArtConfig {
    pub method: ImageMethod,
    pub order: AlbumArtOrder,
    pub max_size_px: Size,
    pub disabled_protocols: Vec<String>,
    pub vertical_align: VerticalAlign,
    pub horizontal_align: HorizontalAlign,
}

#[derive(Default, Display, Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
pub enum HorizontalAlignFile {
    Left,
    #[default]
    Center,
    Right,
}
#[derive(Default, Display, Debug, Clone, Copy, PartialEq, Eq)]
pub enum HorizontalAlign {
    Left,
    #[default]
    Center,
    Right,
}

#[derive(Default, Display, Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
pub enum AlbumArtOrderFile {
    #[default]
    EmbeddedFirst,
    FileFirst,
    EmbeddedOnly,
    FileOnly,
}

#[derive(Default, Display, Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
pub enum VerticalAlignFile {
    Top,
    #[default]
    Center,
    Bottom,
}
#[derive(Default, Display, Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerticalAlign {
    Top,
    #[default]
    Center,
    Bottom,
}

#[derive(Default, Display, Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
pub enum ImageMethodFile {
    Kitty,
    UeberzugWayland,
    UeberzugX11,
    Iterm2,
    Sixel,
    Block,
    None,
    #[default]
    Auto,
}

#[derive(Default, Display, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageMethod {
    Kitty,
    UeberzugWayland,
    UeberzugX11,
    Iterm2,
    Sixel,
    None,
    #[default]
    Block,
}

impl From<AlbumArtConfigFile> for AlbumArtConfig {
    fn from(value: AlbumArtConfigFile) -> Self {
        let size = value.max_size_px;
        AlbumArtConfig {
            method: ImageMethod::default(),
            order: match value.order {
                AlbumArtOrderFile::EmbeddedFirst => AlbumArtOrder::EmbeddedFirst,
                AlbumArtOrderFile::FileFirst => AlbumArtOrder::FileFirst,
                AlbumArtOrderFile::EmbeddedOnly => AlbumArtOrder::EmbeddedOnly,
                AlbumArtOrderFile::FileOnly => AlbumArtOrder::FileOnly,
            },
            max_size_px: Size {
                width: if size.width == 0 { u16::MAX } else { size.width },
                height: if size.height == 0 { u16::MAX } else { size.height },
            },
            disabled_protocols: value.disabled_protocols,
            vertical_align: value.vertical_align.into(),
            horizontal_align: value.horizontal_align.into(),
        }
    }
}

impl From<VerticalAlignFile> for VerticalAlign {
    fn from(value: VerticalAlignFile) -> Self {
        match value {
            VerticalAlignFile::Top => VerticalAlign::Top,
            VerticalAlignFile::Center => VerticalAlign::Center,
            VerticalAlignFile::Bottom => VerticalAlign::Bottom,
        }
    }
}

impl From<HorizontalAlignFile> for HorizontalAlign {
    fn from(value: HorizontalAlignFile) -> Self {
        match value {
            HorizontalAlignFile::Left => HorizontalAlign::Left,
            HorizontalAlignFile::Center => HorizontalAlign::Center,
            HorizontalAlignFile::Right => HorizontalAlign::Right,
        }
    }
}

impl From<ImageBackend> for ImageMethod {
    fn from(value: ImageBackend) -> Self {
        match value {
            ImageBackend::Kitty => ImageMethod::Kitty,
            ImageBackend::Iterm2 => ImageMethod::Iterm2,
            ImageBackend::Sixel => ImageMethod::Sixel,
            ImageBackend::UeberzugWayland => ImageMethod::UeberzugWayland,
            ImageBackend::UeberzugX11 => ImageMethod::UeberzugX11,
            ImageBackend::Block => ImageMethod::Block,
        }
    }
}
