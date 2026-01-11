use std::collections::BTreeMap;

use anyhow::Result;
use bon::Builder;
use itertools::Itertools;
use ratatui::style::{Color, Style};
use serde::{Deserialize, Serialize};
use serde_with::skip_serializing_none;
use strum::Display;

use super::style::ToConfigOr;
use crate::config::{defaults, theme::StyleFile};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SongPropertyFile {
    Filename,
    File,
    FileExtension,
    Title,
    Artist,
    Album,
    Duration,
    Track,
    Disc,
    Position,
    SampleRate(),
    Bits(),
    Channels(),
    Added(),
    LastModified(),
    Other(String),
}

#[derive(Debug, Clone, Display, Hash, Eq, PartialEq)]
pub enum SongProperty {
    Filename,
    File,
    FileExtension,
    Title,
    Artist,
    Album,
    Duration,
    Track,
    Disc,
    Position,
    SampleRate(),
    Bits(),
    Channels(),
    Added(),
    LastModified(),
    Other(String),
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub enum StatusPropertyFile {
    Volume,
    Repeat,
    Random,
    Single,
    Consume,
    State,
    Partition,
    RepeatV2 {
        #[serde(default = "defaults::default_on_label")]
        on_label: String,
        #[serde(default = "defaults::default_off_label")]
        off_label: String,
        #[serde(default)]
        on_style: Option<StyleFile>,
        #[serde(default)]
        off_style: Option<StyleFile>,
    },
    RandomV2 {
        #[serde(default = "defaults::default_on_label")]
        on_label: String,
        #[serde(default = "defaults::default_off_label")]
        off_label: String,
        #[serde(default)]
        on_style: Option<StyleFile>,
        #[serde(default)]
        off_style: Option<StyleFile>,
    },
    SingleV2 {
        #[serde(default = "defaults::default_on_label")]
        on_label: String,
        #[serde(default = "defaults::default_off_label")]
        off_label: String,
        #[serde(default = "defaults::default_oneshot_label")]
        oneshot_label: String,
        #[serde(default)]
        on_style: Option<StyleFile>,
        #[serde(default)]
        off_style: Option<StyleFile>,
        #[serde(default)]
        oneshot_style: Option<StyleFile>,
    },
    ConsumeV2 {
        #[serde(default = "defaults::default_on_label")]
        on_label: String,
        #[serde(default = "defaults::default_off_label")]
        off_label: String,
        #[serde(default = "defaults::default_oneshot_label")]
        oneshot_label: String,
        #[serde(default)]
        on_style: Option<StyleFile>,
        #[serde(default)]
        off_style: Option<StyleFile>,
        #[serde(default)]
        oneshot_style: Option<StyleFile>,
    },
    StateV2 {
        #[serde(default = "defaults::default_playing_label")]
        playing_label: String,
        #[serde(default = "defaults::default_paused_label")]
        paused_label: String,
        #[serde(default = "defaults::default_stopped_label")]
        stopped_label: String,
        #[serde(default)]
        playing_style: Option<StyleFile>,
        #[serde(default)]
        paused_style: Option<StyleFile>,
        #[serde(default)]
        stopped_style: Option<StyleFile>,
    },
    Elapsed,
    Duration,
    Crossfade,
    Bitrate,
    QueueLength {
        #[serde(default = "defaults::default_thousands_separator")]
        thousands_separator: String,
    },
    QueueTimeTotal {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        separator: Option<String>,
    },
    QueueTimeRemaining {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        separator: Option<String>,
    },
    ActiveTab,
    InputBuffer(),
    SampleRate(),
    Bits(),
    Channels(),
}

#[derive(Debug, Clone, Display, Hash, Eq, PartialEq)]
pub enum StatusProperty {
    Volume,
    Repeat {
        on_label: String,
        off_label: String,
        on_style: Option<Style>,
        off_style: Option<Style>,
    },
    Random {
        on_label: String,
        off_label: String,
        on_style: Option<Style>,
        off_style: Option<Style>,
    },
    Single {
        on_label: String,
        off_label: String,
        oneshot_label: String,
        on_style: Option<Style>,
        off_style: Option<Style>,
        oneshot_style: Option<Style>,
    },
    Consume {
        on_label: String,
        off_label: String,
        oneshot_label: String,
        on_style: Option<Style>,
        off_style: Option<Style>,
        oneshot_style: Option<Style>,
    },
    State {
        playing_label: String,
        paused_label: String,
        stopped_label: String,
        playing_style: Option<Style>,
        paused_style: Option<Style>,
        stopped_style: Option<Style>,
    },
    Partition,
    Elapsed,
    Duration,
    Crossfade,
    Bitrate,
    QueueLength {
        thousands_separator: String,
    },
    QueueTimeTotal {
        separator: Option<String>,
    },
    QueueTimeRemaining {
        separator: Option<String>,
    },
    ActiveTab,
    InputBuffer(),
    SampleRate(),
    Bits(),
    Channels(),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum PropertyKindFile {
    Song(SongPropertyFile),
    Status(StatusPropertyFile),
    Widget(WidgetPropertyFile),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum PropertyKindFileOrText<T: Clone> {
    Text(String),
    Sticker(String),
    Property(T),
    Group(Vec<PropertyFile<T>>),
    Transform(TransformFile<T>),
}

#[skip_serializing_none]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PropertyFile<T: Clone> {
    pub kind: PropertyKindFileOrText<T>,
    pub style: Option<StyleFile>,
    pub default: Option<Box<PropertyFile<T>>>,
}

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub enum PropertyKindOrText<T> {
    Text(String),
    Sticker(String),
    Property(T),
    Group(Vec<Property<T>>),
    Transform(Transform<T>),
}

impl<T: Clone> PropertyKindOrText<T> {
    pub fn contains_stickers(&self) -> bool {
        match self {
            PropertyKindOrText::Text(_) => false,
            PropertyKindOrText::Transform(_) => false,
            PropertyKindOrText::Sticker(_) => true,
            PropertyKindOrText::Property(_) => false,
            PropertyKindOrText::Group(group) => {
                group.iter().any(|prop| prop.kind.contains_stickers())
            }
        }
    }

    pub fn collect_properties(&self) -> Vec<T> {
        let mut buf = Vec::new();
        Self::collect_properties_inner(self, &mut buf);
        buf
    }

    fn collect_properties_inner(prop: &PropertyKindOrText<T>, buf: &mut Vec<T>) {
        match prop {
            PropertyKindOrText::Text(_) => {}
            PropertyKindOrText::Sticker(_) => {}
            PropertyKindOrText::Transform(_) => {}
            PropertyKindOrText::Property(p) => buf.push(p.clone()),
            PropertyKindOrText::Group(items) => {
                for p in items {
                    Self::collect_properties_inner(&p.kind, buf);
                }
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReplacementFile<T: Clone> {
    pub r#match: String,
    pub replace: PropertyFile<T>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TransformFile<T: Clone> {
    Truncate {
        content: Box<PropertyFile<T>>,
        length: usize,
        #[serde(default = "defaults::bool::<false>")]
        from_start: bool,
    },
    Replace {
        content: Box<PropertyFile<T>>,
        replacements: Vec<ReplacementFile<T>>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Transform<T> {
    Truncate { content: Box<Property<T>>, length: usize, from_start: bool },
    Replace { content: Box<Property<T>>, replacements: BTreeMap<String, Property<T>> },
}

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub enum PropertyKind {
    Song(SongProperty),
    Status(StatusProperty),
    Widget(WidgetProperty),
}

#[derive(Debug, Clone, Hash, Eq, PartialEq, Builder)]
pub struct Property<T> {
    pub kind: PropertyKindOrText<T>,
    pub style: Option<Style>,
    pub default: Option<Box<Property<T>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum WidgetPropertyFile {
    ScanStatus,
    States { active_style: Option<StyleFile>, separator_style: Option<StyleFile> },
    Volume,
}

#[derive(Debug, Display, Clone, Copy, Hash, Eq, PartialEq)]
pub enum WidgetProperty {
    ScanStatus,
    States { active_style: Style, separator_style: Style },
    Volume,
}

#[derive(Debug, Default, Serialize, Deserialize, Copy, Clone, PartialEq, Eq, Hash)]
pub enum Alignment {
    #[default]
    Left,
    Right,
    Center,
}

impl From<SongPropertyFile> for SongProperty {
    fn from(value: SongPropertyFile) -> Self {
        match value {
            SongPropertyFile::Filename => SongProperty::Filename,
            SongPropertyFile::FileExtension => SongProperty::FileExtension,
            SongPropertyFile::File => SongProperty::File,
            SongPropertyFile::Title => SongProperty::Title,
            SongPropertyFile::Artist => SongProperty::Artist,
            SongPropertyFile::Album => SongProperty::Album,
            SongPropertyFile::Duration => SongProperty::Duration,
            SongPropertyFile::Track => SongProperty::Track,
            SongPropertyFile::Disc => SongProperty::Disc,
            SongPropertyFile::Other(name) => SongProperty::Other(name),
            SongPropertyFile::Position => SongProperty::Position,
            SongPropertyFile::SampleRate() => SongProperty::SampleRate(),
            SongPropertyFile::Bits() => SongProperty::Bits(),
            SongPropertyFile::Channels() => SongProperty::Channels(),
            SongPropertyFile::Added() => SongProperty::Added(),
            SongPropertyFile::LastModified() => SongProperty::LastModified(),
        }
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
                playing_label,
                paused_label,
                stopped_label,
                playing_style,
                paused_style,
                stopped_style,
            } => StatusProperty::State {
                playing_label,
                paused_label,
                stopped_label,
                playing_style: playing_style
                    .map(|s| -> Result<_> { s.to_config_or(None, None) })
                    .transpose()?,
                paused_style: paused_style
                    .map(|s| -> Result<_> { s.to_config_or(None, None) })
                    .transpose()?,
                stopped_style: stopped_style
                    .map(|s| -> Result<_> { s.to_config_or(None, None) })
                    .transpose()?,
            },
            StatusPropertyFile::State => StatusProperty::State {
                playing_label: defaults::default_playing_label(),
                paused_label: defaults::default_paused_label(),
                stopped_label: defaults::default_stopped_label(),
                playing_style: None,
                paused_style: None,
                stopped_style: None,
            },
            StatusPropertyFile::Partition => StatusProperty::Partition,
            StatusPropertyFile::Duration => StatusProperty::Duration,
            StatusPropertyFile::Elapsed => StatusProperty::Elapsed,
            StatusPropertyFile::Volume => StatusProperty::Volume,
            StatusPropertyFile::Bitrate => StatusProperty::Bitrate,
            StatusPropertyFile::Crossfade => StatusProperty::Crossfade,
            StatusPropertyFile::Repeat => StatusProperty::Repeat {
                on_label: defaults::default_on_label(),
                off_label: defaults::default_off_label(),
                on_style: None,
                off_style: None,
            },
            StatusPropertyFile::Random => StatusProperty::Random {
                on_label: defaults::default_on_label(),
                off_label: defaults::default_off_label(),
                on_style: None,
                off_style: None,
            },
            StatusPropertyFile::Consume => StatusProperty::Consume {
                on_label: defaults::default_on_label(),
                off_label: defaults::default_off_label(),
                oneshot_label: defaults::default_oneshot_label(),
                on_style: None,
                off_style: None,
                oneshot_style: None,
            },
            StatusPropertyFile::Single => StatusProperty::Single {
                on_label: defaults::default_on_label(),
                off_label: defaults::default_off_label(),
                oneshot_label: defaults::default_oneshot_label(),
                on_style: None,
                off_style: None,
                oneshot_style: None,
            },
            StatusPropertyFile::RepeatV2 { on_label, off_label, on_style, off_style } => {
                StatusProperty::Repeat {
                    on_label,
                    off_label,
                    on_style: on_style
                        .map(|s| -> Result<_> { s.to_config_or(None, None) })
                        .transpose()?,
                    off_style: off_style
                        .map(|s| -> Result<_> { s.to_config_or(None, None) })
                        .transpose()?,
                }
            }
            StatusPropertyFile::RandomV2 { on_label, off_label, on_style, off_style } => {
                StatusProperty::Random {
                    on_label,
                    off_label,
                    on_style: on_style
                        .map(|s| -> Result<_> { s.to_config_or(None, None) })
                        .transpose()?,
                    off_style: off_style
                        .map(|s| -> Result<_> { s.to_config_or(None, None) })
                        .transpose()?,
                }
            }
            StatusPropertyFile::ConsumeV2 {
                on_label,
                off_label,
                oneshot_label,
                on_style,
                off_style,
                oneshot_style,
            } => StatusProperty::Consume {
                on_label,
                off_label,
                oneshot_label,
                on_style: on_style
                    .map(|s| -> Result<_> { s.to_config_or(None, None) })
                    .transpose()?,
                off_style: off_style
                    .map(|s| -> Result<_> { s.to_config_or(None, None) })
                    .transpose()?,
                oneshot_style: oneshot_style
                    .map(|s| -> Result<_> { s.to_config_or(None, None) })
                    .transpose()?,
            },
            StatusPropertyFile::SingleV2 {
                on_label,
                off_label,
                oneshot_label,
                on_style,
                off_style,
                oneshot_style,
            } => StatusProperty::Single {
                on_label,
                off_label,
                oneshot_label,
                on_style: on_style
                    .map(|s| -> Result<_> { s.to_config_or(None, None) })
                    .transpose()?,
                off_style: off_style
                    .map(|s| -> Result<_> { s.to_config_or(None, None) })
                    .transpose()?,
                oneshot_style: oneshot_style
                    .map(|s| -> Result<_> { s.to_config_or(None, None) })
                    .transpose()?,
            },
            StatusPropertyFile::QueueLength { thousands_separator } => {
                StatusProperty::QueueLength { thousands_separator }
            }
            StatusPropertyFile::QueueTimeTotal { separator } => {
                StatusProperty::QueueTimeTotal { separator }
            }
            StatusPropertyFile::QueueTimeRemaining { separator } => {
                StatusProperty::QueueTimeRemaining { separator }
            }
            StatusPropertyFile::ActiveTab => StatusProperty::ActiveTab,
            StatusPropertyFile::InputBuffer() => StatusProperty::InputBuffer(),
            StatusPropertyFile::SampleRate() => StatusProperty::SampleRate(),
            StatusPropertyFile::Bits() => StatusProperty::Bits(),
            StatusPropertyFile::Channels() => StatusProperty::Channels(),
        })
    }
}

impl TryFrom<PropertyFile<PropertyKindFile>> for Property<PropertyKind> {
    type Error = anyhow::Error;

    fn try_from(value: PropertyFile<PropertyKindFile>) -> std::result::Result<Self, Self::Error> {
        Ok(Self {
            kind: match value.kind {
                PropertyKindFileOrText::Text(value) => PropertyKindOrText::Text(value),
                PropertyKindFileOrText::Transform(TransformFile::Truncate {
                    content,
                    length,
                    from_start,
                }) => PropertyKindOrText::Transform(Transform::Truncate {
                    content: Box::new((*content).try_into()?),
                    length,
                    from_start,
                }),
                PropertyKindFileOrText::Transform(TransformFile::Replace {
                    content,
                    replacements,
                }) => PropertyKindOrText::Transform(Transform::Replace {
                    content: Box::new((*content).try_into()?),
                    replacements: replacements
                        .into_iter()
                        .map(|r| -> Result<_> { Ok((r.r#match, r.replace.try_into()?)) })
                        .try_collect()?,
                }),
                PropertyKindFileOrText::Sticker(value) => PropertyKindOrText::Sticker(value),
                PropertyKindFileOrText::Property(prop) => {
                    PropertyKindOrText::Property(match prop {
                        PropertyKindFile::Song(s) => PropertyKind::Song(s.into()),
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
                        PropertyKindFile::Widget(WidgetPropertyFile::ScanStatus) => {
                            PropertyKind::Widget(WidgetProperty::ScanStatus)
                        }
                    })
                }
                PropertyKindFileOrText::Group(group) => {
                    let res: Vec<_> = group
                        .into_iter()
                        .map(|p| -> Result<Property<PropertyKind>> { p.try_into() })
                        .try_collect()?;
                    PropertyKindOrText::Group(res)
                }
            },
            style: Some(value.style.to_config_or(None, None)?),
            default: value
                .default
                .map(|v| -> Result<_> {
                    Ok(Box::new(TryFrom::<PropertyFile<PropertyKindFile>>::try_from(*v)?))
                })
                .transpose()?,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SongFormatFile(pub Vec<PropertyFile<SongPropertyFile>>);

#[derive(Debug, Default, Clone)]
pub struct SongFormat(pub Vec<Property<SongProperty>>);

impl TryFrom<SongFormatFile> for SongFormat {
    type Error = anyhow::Error;

    fn try_from(value: SongFormatFile) -> Result<Self, Self::Error> {
        let properties: Vec<_> = value.0.into_iter().map(|v| v.convert()).try_collect()?;
        Ok(SongFormat(properties))
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
