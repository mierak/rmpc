use ratatui::style::{Color, Style};
use serde::{Deserialize, Serialize};
use strum::Display;

use crate::config::ui::StyleFile;

use super::style::ToConfigOr;

#[derive(Debug, Serialize, Deserialize)]
pub enum SongPropertyFile {
    Filename {
        style: Option<StyleFile>,
    },
    Title {
        style: Option<StyleFile>,
        default: String,
    },
    Artist {
        style: Option<StyleFile>,
        default: String,
    },
    Album {
        style: Option<StyleFile>,
        default: String,
    },
    Duration {
        style: Option<StyleFile>,
        default: String,
    },
    Other {
        name: String,
        default: String,
        style: Option<StyleFile>,
    },
}

#[derive(Debug, Copy, Clone, Display)]
pub enum SongProperty {
    Filename {
        style: Style,
    },
    Title {
        style: Style,
        default: &'static str,
    },
    Artist {
        style: Style,
        default: &'static str,
    },
    Album {
        style: Style,
        default: &'static str,
    },
    Duration {
        style: Style,
        default: &'static str,
    },
    Other {
        name: &'static str,
        default: &'static str,
        style: Style,
    },
}

#[derive(Debug, Serialize, Deserialize)]
pub enum StatusPropertyFile {
    Volume { style: Option<StyleFile> },
    Repeat { style: Option<StyleFile> },
    Random { style: Option<StyleFile> },
    Single { style: Option<StyleFile> },
    Consume { style: Option<StyleFile> },
    State { style: Option<StyleFile> },
    Elapsed { style: Option<StyleFile> },
    Duration { style: Option<StyleFile> },
    Crossfade { style: Option<StyleFile>, default: String },
    Bitrate { style: Option<StyleFile>, default: String },
}

#[derive(Debug, Copy, Clone, Display)]
pub enum StatusProperty {
    Volume { style: Style },
    Repeat { style: Style },
    Random { style: Style },
    Single { style: Style },
    Consume { style: Style },
    State { style: Style },
    Elapsed { style: Style },
    Duration { style: Style },
    Crossfade { style: Style, default: &'static str },
    Bitrate { style: Style, default: &'static str },
}

#[derive(Debug, Serialize, Deserialize)]
pub enum PropertyFile {
    Song(SongPropertyFile),
    Status(StatusPropertyFile),
    Widget(WidgetPropertyFile),
    Text { value: String, style: Option<StyleFile> },
}

#[derive(Debug)]
pub enum Property {
    Song(SongProperty),
    Status(StatusProperty),
    Widget(WidgetProperty),
    Text { value: &'static str, style: Style },
}

#[derive(Debug, Serialize, Deserialize)]
pub enum WidgetPropertyFile {
    States {
        active_style: Option<StyleFile>,
        inactive_style: Option<StyleFile>,
        separator_style: Option<StyleFile>,
    },
    Volume {
        style: Option<StyleFile>,
    },
}

#[derive(Debug, Display, Clone, Copy)]
pub enum WidgetProperty {
    States {
        active_style: Style,
        inactive_style: Style,
        separator_style: Style,
    },
    Volume {
        style: Style,
    },
}

#[derive(Debug, Serialize, Deserialize, Copy, Clone)]
pub enum Alignment {
    Left,
    Right,
    Center,
}

impl TryFrom<SongPropertyFile> for SongProperty {
    type Error = anyhow::Error;

    fn try_from(value: SongPropertyFile) -> std::result::Result<Self, Self::Error> {
        Ok(match value {
            SongPropertyFile::Filename { style } => SongProperty::Filename {
                style: style.to_config_or(None, None)?,
            },
            SongPropertyFile::Title { style, default } => SongProperty::Title {
                style: style.to_config_or(None, None)?,
                default: Box::leak(Box::new(default)),
            },
            SongPropertyFile::Artist { style, default } => SongProperty::Artist {
                style: style.to_config_or(None, None)?,
                default: Box::leak(Box::new(default)),
            },
            SongPropertyFile::Album { style, default } => SongProperty::Album {
                style: style.to_config_or(None, None)?,
                default: Box::leak(Box::new(default)),
            },
            SongPropertyFile::Duration { style, default } => SongProperty::Duration {
                style: style.to_config_or(None, None)?,
                default: Box::leak(Box::new(default)),
            },
            SongPropertyFile::Other { name, default, style } => SongProperty::Other {
                name: Box::leak(Box::new(name)),
                style: style.to_config_or(None, None)?,
                default: Box::leak(Box::new(default)),
            },
        })
    }
}

impl From<Alignment> for ratatui::layout::Alignment {
    fn from(value: Alignment) -> Self {
        match value {
            Alignment::Left => Self::Left,
            Alignment::Right => Self::Right,
            Alignment::Center => Self::Center,
        }
    }
}

impl TryFrom<PropertyFile> for Property {
    type Error = anyhow::Error;

    fn try_from(value: PropertyFile) -> std::result::Result<Self, Self::Error> {
        Ok(match value {
            PropertyFile::Song(SongPropertyFile::Filename { style }) => Property::Song(SongProperty::Filename {
                style: style.to_config_or(None, None)?,
            }),
            PropertyFile::Song(SongPropertyFile::Title { style, default }) => Property::Song(SongProperty::Title {
                style: style.to_config_or(None, None)?,
                default: Box::leak(Box::new(default)),
            }),
            PropertyFile::Song(SongPropertyFile::Artist { style, default }) => Property::Song(SongProperty::Artist {
                style: style.to_config_or(None, None)?,
                default: Box::leak(Box::new(default)),
            }),
            PropertyFile::Song(SongPropertyFile::Album { style, default }) => Property::Song(SongProperty::Album {
                style: style.to_config_or(None, None)?,
                default: Box::leak(Box::new(default)),
            }),
            PropertyFile::Song(SongPropertyFile::Duration { style, default }) => {
                Property::Song(SongProperty::Duration {
                    style: style.to_config_or(None, None)?,
                    default: Box::leak(Box::new(default)),
                })
            }
            PropertyFile::Song(SongPropertyFile::Other { name, style, default }) => {
                Property::Song(SongProperty::Other {
                    name: Box::leak(Box::new(name)),
                    style: style.to_config_or(None, None)?,
                    default: Box::leak(Box::new(default)),
                })
            }
            PropertyFile::Status(StatusPropertyFile::State { style }) => Property::Status(StatusProperty::State {
                style: style.to_config_or(None, None)?,
            }),
            PropertyFile::Status(StatusPropertyFile::Duration { style }) => {
                Property::Status(StatusProperty::Duration {
                    style: style.to_config_or(None, None)?,
                })
            }
            PropertyFile::Status(StatusPropertyFile::Elapsed { style }) => Property::Status(StatusProperty::Elapsed {
                style: style.to_config_or(None, None)?,
            }),
            PropertyFile::Status(StatusPropertyFile::Volume { style }) => Property::Status(StatusProperty::Volume {
                style: style.to_config_or(None, None)?,
            }),
            PropertyFile::Status(StatusPropertyFile::Repeat { style }) => Property::Status(StatusProperty::Repeat {
                style: style.to_config_or(None, None)?,
            }),
            PropertyFile::Status(StatusPropertyFile::Random { style }) => Property::Status(StatusProperty::Random {
                style: style.to_config_or(None, None)?,
            }),
            PropertyFile::Status(StatusPropertyFile::Consume { style }) => Property::Status(StatusProperty::Consume {
                style: style.to_config_or(None, None)?,
            }),
            PropertyFile::Status(StatusPropertyFile::Single { style }) => Property::Status(StatusProperty::Single {
                style: style.to_config_or(None, None)?,
            }),
            PropertyFile::Status(StatusPropertyFile::Bitrate { style, default }) => {
                Property::Status(StatusProperty::Bitrate {
                    style: style.to_config_or(None, None)?,
                    default: Box::leak(Box::new(default)),
                })
            }
            PropertyFile::Status(StatusPropertyFile::Crossfade { style, default }) => {
                Property::Status(StatusProperty::Crossfade {
                    style: style.to_config_or(None, None)?,
                    default: Box::leak(Box::new(default)),
                })
            }
            PropertyFile::Widget(WidgetPropertyFile::States {
                active_style,
                inactive_style,
                separator_style,
            }) => Property::Widget(WidgetProperty::States {
                active_style: active_style.to_config_or(Some(Color::White), None)?,
                inactive_style: inactive_style.to_config_or(Some(Color::DarkGray), None)?,
                separator_style: separator_style.to_config_or(Some(Color::White), None)?,
            }),
            PropertyFile::Widget(WidgetPropertyFile::Volume { style }) => Property::Widget(WidgetProperty::Volume {
                style: style.to_config_or(Some(Color::Blue), None)?,
            }),
            PropertyFile::Text { value: text, style } => Property::Text {
                value: Box::leak(Box::new(text)),
                style: style.to_config_or(None, None)?,
            },
        })
    }
}
