use anyhow::Result;
use itertools::Itertools;
use ratatui::style::{Color, Style};
use serde::{Deserialize, Serialize};
use serde_with::skip_serializing_none;
use strum::Display;

use crate::config::theme::StyleFile;

use super::style::ToConfigOr;

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum SongPropertyFile {
    Filename,
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
    Title,
    Artist,
    Album,
    Duration,
    Track,
    Other(&'static str),
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum StatusPropertyFile {
    Volume,
    Repeat,
    Random,
    Single,
    Consume,
    State,
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
    State,
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
    Property(T),
}

#[skip_serializing_none]
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PropertyFile<T> {
    pub kind: PropertyKindFileOrText<T>,
    pub style: Option<StyleFile>,
    pub default: Option<Box<PropertyFile<T>>>,
}

#[derive(Debug, Clone, Copy)]
pub enum PropertyKindOrText<T> {
    Text(&'static str),
    Property(T),
}

#[derive(Debug, Clone)]
pub enum PropertyKind {
    Song(SongProperty),
    Status(StatusProperty),
    Widget(WidgetProperty),
}

#[derive(Debug, Clone)]
pub struct Property<'a, T> {
    pub kind: PropertyKindOrText<T>,
    pub style: Option<Style>,
    pub default: Option<&'a Property<'a, T>>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum WidgetPropertyFile {
    States {
        active_style: Option<StyleFile>,
        separator_style: Option<StyleFile>,
    },
    Volume,
}

#[derive(Debug, Display, Clone, Copy)]
pub enum WidgetProperty {
    States {
        active_style: Style,
        separator_style: Style,
    },
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
            SongPropertyFile::Title => SongProperty::Title,
            SongPropertyFile::Artist => SongProperty::Artist,
            SongPropertyFile::Album => SongProperty::Album,
            SongPropertyFile::Duration => SongProperty::Duration,
            SongPropertyFile::Track => SongProperty::Track,
            SongPropertyFile::Other(name) => SongProperty::Other(Box::leak(Box::new(name))),
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
            StatusPropertyFile::State => StatusProperty::State,
            StatusPropertyFile::Duration => StatusProperty::Duration {},
            StatusPropertyFile::Elapsed => StatusProperty::Elapsed {},
            StatusPropertyFile::Volume => StatusProperty::Volume {},
            StatusPropertyFile::Repeat => StatusProperty::Repeat {},
            StatusPropertyFile::Random => StatusProperty::Random {},
            StatusPropertyFile::Consume => StatusProperty::Consume {},
            StatusPropertyFile::Single => StatusProperty::Single {},
            StatusPropertyFile::Bitrate => StatusProperty::Bitrate,
            StatusPropertyFile::Crossfade => StatusProperty::Crossfade,
        })
    }
}

impl TryFrom<PropertyFile<PropertyKindFile>> for &'static Property<'static, PropertyKind> {
    type Error = anyhow::Error;

    fn try_from(value: PropertyFile<PropertyKindFile>) -> std::prelude::v1::Result<Self, Self::Error> {
        Property::<'static, PropertyKind>::try_from(value)
            .map(|v| Box::leak(Box::new(v)))
            .map(|v| {
                let v: &'static Property<_> = v;
                v
            })
    }
}

impl TryFrom<PropertyFile<PropertyKindFile>> for Property<'static, PropertyKind> {
    type Error = anyhow::Error;

    fn try_from(value: PropertyFile<PropertyKindFile>) -> std::result::Result<Self, Self::Error> {
        Ok(Self {
            kind: match value.kind {
                PropertyKindFileOrText::Text(value) => {
                    PropertyKindOrText::Text(Box::leak(Box::new(value)) as &'static str)
                }
                PropertyKindFileOrText::Property(prop) => PropertyKindOrText::Property(match prop {
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
                        separator_style: separator_style.to_config_or(Some(Color::White), None)?,
                    }),
                }),
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
        let properites: Vec<_> = value
            .0
            .into_iter()
            .map(|v| -> Result<_> {
                let res = Box::leak(Box::new(Property::try_from(v)?));
                Ok(res as &'static Property<'static, SongProperty>)
            })
            .try_collect()?;

        Ok(SongFormat(Box::leak(Box::new(properites))))
    }
}

impl Default for SongFormatFile {
    fn default() -> Self {
        Self(vec![
            PropertyFile {
                kind: PropertyKindFileOrText::Property(SongPropertyFile::Track),
                style: None,
                default: Some(Box::new(PropertyFile {
                    kind: PropertyKindFileOrText::Text("--".to_string()),
                    style: None,
                    default: None,
                })),
            },
            PropertyFile {
                kind: PropertyKindFileOrText::Text(" ".to_string()),
                style: None,
                default: None,
            },
            PropertyFile {
                kind: PropertyKindFileOrText::Property(SongPropertyFile::Artist),
                style: None,
                default: Some(Box::new(PropertyFile {
                    kind: PropertyKindFileOrText::Text("Unknown Artist".to_string()),
                    style: None,
                    default: None,
                })),
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
        ])
    }
}
