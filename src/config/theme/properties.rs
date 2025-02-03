use anyhow::Result;
use itertools::Itertools;
use ratatui::style::{Color, Style};
use serde::{Deserialize, Serialize};
use serde_with::skip_serializing_none;
use strum::Display;

use super::style::ToConfigOr;
use crate::config::{Leak, defaults, theme::StyleFile};

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum SongPropertyFile {
    Filename,
    File,
    Title,
    Artist,
    Album,
    Duration,
    Track,
    Other(String),
}

#[derive(Debug, Copy, Clone, Display)]
pub enum SongProperty {
    Filename,
    File,
    Title,
    Artist,
    Album,
    Duration,
    Track,
    Other(&'static str),
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
pub enum StatusPropertyFile {
    Volume,
    Repeat,
    Random,
    Single,
    Consume,
    State,
    StateV2 {
        #[serde(default = "defaults::default_playing_label")]
        playing_label: String,
        #[serde(default = "defaults::default_paused_label")]
        paused_label: String,
        #[serde(default = "defaults::default_stopped_label")]
        stopped_label: String,
    },
    Elapsed,
    Duration,
    Crossfade,
    Bitrate,
}

#[derive(Debug, Clone, Display)]
pub enum StatusProperty {
    Volume,
    Repeat,
    Random,
    Single,
    Consume,
    State { playing_label: &'static str, paused_label: &'static str, stopped_label: &'static str },
    Elapsed,
    Duration,
    Crossfade,
    Bitrate,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum PropertyKindFile {
    Song(SongPropertyFile),
    Status(StatusPropertyFile),
    Widget(WidgetPropertyFile),
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum PropertyKindFileOrText<T> {
    Text(String),
    Sticker(String),
    Property(T),
    Group(Vec<PropertyFile<T>>),
}

#[skip_serializing_none]
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PropertyFile<T> {
    pub kind: PropertyKindFileOrText<T>,
    pub style: Option<StyleFile>,
    pub default: Option<Box<PropertyFile<T>>>,
}

#[derive(Debug, Clone, Copy)]
pub enum PropertyKindOrText<'a, T> {
    Text(&'a str),
    Sticker(&'a str),
    Property(T),
    Group(&'a [&'a Property<'a, T>]),
}

impl<T> PropertyKindOrText<'_, T> {
    pub fn contains_stickers(&self) -> bool {
        match self {
            PropertyKindOrText::Text(_) => false,
            PropertyKindOrText::Sticker(_) => true,
            PropertyKindOrText::Property(_) => false,
            PropertyKindOrText::Group(group) => {
                group.iter().any(|prop| prop.kind.contains_stickers())
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum PropertyKind {
    Song(SongProperty),
    Status(StatusProperty),
    Widget(WidgetProperty),
}

#[derive(Debug, Clone)]
pub struct Property<'a, T> {
    pub kind: PropertyKindOrText<'a, T>,
    pub style: Option<Style>,
    pub default: Option<&'a Property<'a, T>>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum WidgetPropertyFile {
    States { active_style: Option<StyleFile>, separator_style: Option<StyleFile> },
    Volume,
}

#[derive(Debug, Display, Clone, Copy)]
pub enum WidgetProperty {
    States { active_style: Style, separator_style: Style },
    Volume,
}

#[derive(Debug, Serialize, Deserialize, Copy, Clone, PartialEq, Eq)]
pub enum Alignment {
    Left,
    Right,
    Center,
}

impl TryFrom<SongPropertyFile> for SongProperty {
    type Error = anyhow::Error;

    fn try_from(value: SongPropertyFile) -> std::result::Result<Self, Self::Error> {
        Ok(match value {
            SongPropertyFile::Filename => SongProperty::Filename,
            SongPropertyFile::File => SongProperty::File,
            SongPropertyFile::Title => SongProperty::Title,
            SongPropertyFile::Artist => SongProperty::Artist,
            SongPropertyFile::Album => SongProperty::Album,
            SongPropertyFile::Duration => SongProperty::Duration,
            SongPropertyFile::Track => SongProperty::Track,
            SongPropertyFile::Other(name) => SongProperty::Other(name.leak()),
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

impl TryFrom<StatusPropertyFile> for StatusProperty {
    type Error = anyhow::Error;

    fn try_from(value: StatusPropertyFile) -> Result<Self, Self::Error> {
        Ok(match value {
            StatusPropertyFile::StateV2 {
                playing_label: play_label,
                paused_label: pause_label,
                stopped_label: stop_label,
            } => StatusProperty::State {
                playing_label: play_label.leak(),
                paused_label: pause_label.leak(),
                stopped_label: stop_label.leak(),
            },
            StatusPropertyFile::State => StatusProperty::State {
                playing_label: defaults::default_playing_label().leak(),
                paused_label: defaults::default_paused_label().leak(),
                stopped_label: defaults::default_stopped_label().leak(),
            },
            StatusPropertyFile::Duration => StatusProperty::Duration,
            StatusPropertyFile::Elapsed => StatusProperty::Elapsed,
            StatusPropertyFile::Volume => StatusProperty::Volume,
            StatusPropertyFile::Repeat => StatusProperty::Repeat,
            StatusPropertyFile::Random => StatusProperty::Random,
            StatusPropertyFile::Consume => StatusProperty::Consume,
            StatusPropertyFile::Single => StatusProperty::Single,
            StatusPropertyFile::Bitrate => StatusProperty::Bitrate,
            StatusPropertyFile::Crossfade => StatusProperty::Crossfade,
        })
    }
}

impl TryFrom<PropertyFile<PropertyKindFile>> for &'static Property<'static, PropertyKind> {
    type Error = anyhow::Error;

    fn try_from(
        value: PropertyFile<PropertyKindFile>,
    ) -> std::prelude::v1::Result<Self, Self::Error> {
        Property::<'static, PropertyKind>::try_from(value).map(|v| v.leak())
    }
}

impl TryFrom<PropertyFile<PropertyKindFile>> for Property<'static, PropertyKind> {
    type Error = anyhow::Error;

    fn try_from(value: PropertyFile<PropertyKindFile>) -> std::result::Result<Self, Self::Error> {
        Ok(Self {
            kind: match value.kind {
                PropertyKindFileOrText::Text(value) => PropertyKindOrText::Text(value.leak()),
                PropertyKindFileOrText::Sticker(value) => PropertyKindOrText::Sticker(value.leak()),
                PropertyKindFileOrText::Property(prop) => {
                    PropertyKindOrText::Property(match prop {
                        PropertyKindFile::Song(s) => PropertyKind::Song(s.try_into()?),
                        PropertyKindFile::Status(s) => PropertyKind::Status(s.try_into()?),
                        PropertyKindFile::Widget(WidgetPropertyFile::Volume) => {
                            PropertyKind::Widget(WidgetProperty::Volume)
                        }
                        PropertyKindFile::Widget(WidgetPropertyFile::States {
                            active_style,
                            separator_style,
                        }) => PropertyKind::Widget(WidgetProperty::States {
                            active_style: active_style.to_config_or(Some(Color::White), None)?,
                            separator_style: separator_style
                                .to_config_or(Some(Color::White), None)?,
                        }),
                    })
                }
                PropertyKindFileOrText::Group(group) => {
                    let res: Vec<_> = group
                        .into_iter()
                        .map(|p| -> Result<&'static Property<'static, PropertyKind>> {
                            p.try_into()
                        })
                        .try_collect()?;
                    PropertyKindOrText::Group(res.leak())
                }
            },
            style: Some(value.style.to_config_or(None, None)?),
            default: value
                .default
                .map(|v| TryFrom::<PropertyFile<PropertyKindFile>>::try_from(*v))
                .transpose()?,
        })
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SongFormatFile(pub Vec<PropertyFile<SongPropertyFile>>);

#[derive(Default, Clone, Copy)]
pub struct SongFormat(pub &'static [&'static Property<'static, SongProperty>]);

impl TryFrom<SongFormatFile> for SongFormat {
    type Error = anyhow::Error;

    fn try_from(value: SongFormatFile) -> Result<Self, Self::Error> {
        let properites: Vec<_> = value.0.into_iter().map(|v| v.try_into()).try_collect()?;
        Ok(SongFormat(properites.leak()))
    }
}

impl Default for SongFormatFile {
    fn default() -> Self {
        Self(vec![
            PropertyFile {
                kind: PropertyKindFileOrText::Group(vec![
                    PropertyFile {
                        kind: PropertyKindFileOrText::Property(SongPropertyFile::Track),
                        style: None,
                        default: None,
                    },
                    PropertyFile {
                        kind: PropertyKindFileOrText::Text(" ".to_string()),
                        style: None,
                        default: None,
                    },
                ]),
                style: None,
                default: None,
            },
            PropertyFile {
                kind: PropertyKindFileOrText::Group(vec![
                    PropertyFile {
                        kind: PropertyKindFileOrText::Property(SongPropertyFile::Artist),
                        style: None,
                        default: None,
                    },
                    PropertyFile {
                        kind: PropertyKindFileOrText::Text(" - ".to_string()),
                        style: None,
                        default: None,
                    },
                    PropertyFile {
                        kind: PropertyKindFileOrText::Property(SongPropertyFile::Title),
                        style: None,
                        default: None,
                    },
                ]),
                style: None,
                default: Some(Box::new(PropertyFile {
                    kind: PropertyKindFileOrText::Property(SongPropertyFile::Filename),
                    style: None,
                    default: None,
                })),
            },
        ])
    }
}
