use anyhow::Result;
use ratatui::style::{Color, Style};
use serde::{Deserialize, Serialize};
use serde_with::skip_serializing_none;
use strum::Display;

use crate::config::ui::StyleFile;

use super::style::ToConfigOr;

#[derive(Debug, Serialize, Deserialize)]
pub enum SongPropertyFile {
    Filename,
    Title,
    Artist,
    Album,
    Duration,
    Other { name: String },
}

#[derive(Debug, Copy, Clone, Display)]
pub enum SongProperty {
    Filename,
    Title,
    Artist,
    Album,
    Duration,
    Other { name: &'static str },
}

#[derive(Debug, Serialize, Deserialize)]
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

#[derive(Debug, Serialize, Deserialize)]
pub enum PropertyKindFile {
    Song(SongPropertyFile),
    Status(StatusPropertyFile),
    Widget(WidgetPropertyFile),
}

#[derive(Debug, Serialize, Deserialize)]
pub enum PropertyKindFileOrText<T> {
    Text { value: String },
    Property(T),
}

#[skip_serializing_none]
#[derive(Debug, Serialize, Deserialize)]
pub struct PropertyFile<T> {
    pub kind: PropertyKindFileOrText<T>,
    pub style: Option<StyleFile>,
    pub default: Option<Box<PropertyFile<T>>>,
}

#[derive(Debug, Clone)]
pub enum PropertyKindOrText<T> {
    Text { value: String },
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

fn mapstyle(style: Option<&StyleFile>) -> String {
    style.map_or("none".to_string(), ToString::to_string)
}

impl std::fmt::Display for PropertyFile<PropertyKindFile> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.kind {
            PropertyKindFileOrText::Text { value } => write!(f, "Text({value})"),
            PropertyKindFileOrText::Property(PropertyKindFile::Song(s)) => match s {
                SongPropertyFile::Duration => write!(f, "Song::Duration"),
                SongPropertyFile::Title => write!(f, "Song::Title"),
                SongPropertyFile::Album => write!(f, "Song::Album"),
                SongPropertyFile::Artist => write!(f, "Song::Artist"),
                SongPropertyFile::Other { name } => write!(f, "Song::Other({name})"),
                SongPropertyFile::Filename => write!(f, "Song::Filename"),
            },
            PropertyKindFileOrText::Property(PropertyKindFile::Status(s)) => match s {
                StatusPropertyFile::Volume => write!(f, "Status::Volume"),
                StatusPropertyFile::State => write!(f, "Status::State"),
                StatusPropertyFile::Repeat => write!(f, "Status::Repeat"),
                StatusPropertyFile::Random => write!(f, "Status::Random"),
                StatusPropertyFile::Single => write!(f, "Status::Single"),
                StatusPropertyFile::Consume => write!(f, "Status::Consume"),
                StatusPropertyFile::Elapsed => write!(f, "Status::Elapsed"),
                StatusPropertyFile::Bitrate => write!(f, "Status::Bitrate"),
                StatusPropertyFile::Crossfade => write!(f, "Status::Crossfade"),
                StatusPropertyFile::Duration => write!(f, "Status::Duration"),
            },
            PropertyKindFileOrText::Property(PropertyKindFile::Widget(w)) => match w {
                WidgetPropertyFile::Volume => write!(f, "Widget::Volume"),
                WidgetPropertyFile::States {
                    active_style,
                    separator_style,
                } => {
                    write!(
                        f,
                        "Widget::States;{};{}",
                        mapstyle(active_style.as_ref()),
                        mapstyle(separator_style.as_ref()),
                    )
                }
            },
        }?;

        if let Some(ref style) = self.style {
            write!(f, ":{style}")?;
        }

        if let Some(ref default) = self.default {
            write!(f, " ?? {default}")?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::config::ui::style::Modifiers;

    use super::*;

    #[test]
    fn test_display() {
        let prop = PropertyFile::<PropertyKindFile> {
            kind: PropertyKindFileOrText::Property(PropertyKindFile::Song(SongPropertyFile::Other {
                name: "albumartist".to_string(),
            })),
            style: Some(StyleFile {
                fg: Some("yellow".to_string()),
                bg: None,
                modifiers: Some(Modifiers::Bold),
            }),
            default: Some(Box::new(PropertyFile {
                kind: PropertyKindFileOrText::Property(PropertyKindFile::Song(SongPropertyFile::Album)),
                style: Some(StyleFile {
                    fg: Some("red".to_string()),
                    bg: Some("black".to_string()),
                    modifiers: Some(Modifiers::Italic | Modifiers::Bold),
                }),
                default: Some(Box::new(PropertyFile {
                    kind: PropertyKindFileOrText::Text {
                        value: "Unknown".to_string(),
                    },
                    style: None,
                    default: None,
                })),
            })),
        };

        assert_eq!(
            format!("${{{prop}}}"),
            "${Song::Duration;yellow;none;b ?? Text(Unknown)}"
        );
    }
}

#[derive(Debug, Serialize, Deserialize)]
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
            SongPropertyFile::Filename => SongProperty::Filename,
            SongPropertyFile::Title => SongProperty::Title,
            SongPropertyFile::Artist => SongProperty::Artist,
            SongPropertyFile::Album => SongProperty::Album,
            SongPropertyFile::Duration => SongProperty::Duration,
            SongPropertyFile::Other { name } => SongProperty::Other {
                name: Box::leak(Box::new(name)),
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
                PropertyKindFileOrText::Text { value } => PropertyKindOrText::Text { value },
                PropertyKindFileOrText::Property(prop) => PropertyKindOrText::Property(match prop {
                    PropertyKindFile::Song(s) => PropertyKind::Song(s.try_into()?),
                    PropertyKindFile::Status(s) => PropertyKind::Status(s.try_into()?),
                    PropertyKindFile::Widget(WidgetPropertyFile::Volume) => {
                        PropertyKind::Widget(WidgetProperty::Volume {
                            style: value.style.to_config_or(None, None)?,
                        })
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
