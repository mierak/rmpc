use anyhow::Result;
use itertools::Itertools;
use serde::{Deserialize, Serialize};

use super::properties::{
    Property, PropertyFile, PropertyKind, PropertyKindFile, PropertyKindFileOrText, SongPropertyFile,
    StatusPropertyFile, WidgetPropertyFile,
};
use super::style::{Modifiers, StyleFile};

#[derive(Debug, Default)]
pub struct HeaderConfigRow {
    pub left: &'static [&'static Property<'static, PropertyKind>],
    pub center: &'static [&'static Property<'static, PropertyKind>],
    pub right: &'static [&'static Property<'static, PropertyKind>],
}

#[derive(Debug, Default)]
pub struct HeaderConfig {
    pub rows: Vec<HeaderConfigRow>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct HeaderConfigRowFile {
    pub(super) left: Vec<PropertyFile<PropertyKindFile>>,
    pub(super) center: Vec<PropertyFile<PropertyKindFile>>,
    pub(super) right: Vec<PropertyFile<PropertyKindFile>>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct HeaderConfigFile {
    pub(super) rows: Vec<HeaderConfigRowFile>,
}

impl Default for HeaderConfigFile {
    fn default() -> Self {
        Self {
            rows: vec![
                HeaderConfigRowFile {
                    left: vec![
                        PropertyFile {
                            kind: PropertyKindFileOrText::Text("[".to_string()),
                            style: Some(StyleFile {
                                fg: Some("yellow".to_string()),
                                bg: None,
                                modifiers: Some(Modifiers::Bold),
                            }),
                            default: None,
                        },
                        PropertyFile {
                            kind: PropertyKindFileOrText::Property(PropertyKindFile::Status(StatusPropertyFile::State)),
                            style: Some(StyleFile {
                                fg: Some("yellow".to_string()),
                                bg: None,
                                modifiers: Some(Modifiers::Bold),
                            }),
                            default: None,
                        },
                        PropertyFile {
                            kind: PropertyKindFileOrText::Text("]".to_string()),
                            style: Some(StyleFile {
                                fg: Some("yellow".to_string()),
                                bg: None,
                                modifiers: Some(Modifiers::Bold),
                            }),
                            default: None,
                        },
                    ],
                    center: vec![PropertyFile {
                        kind: PropertyKindFileOrText::Property(PropertyKindFile::Song(SongPropertyFile::Title)),
                        default: Some(Box::new(PropertyFile {
                            kind: PropertyKindFileOrText::Text("No Song".to_string()),
                            style: Some(StyleFile {
                                fg: None,
                                bg: None,
                                modifiers: Some(Modifiers::Bold),
                            }),
                            default: None,
                        })),
                        style: Some(StyleFile {
                            fg: None,
                            bg: None,
                            modifiers: Some(Modifiers::Bold),
                        }),
                    }],
                    right: vec![PropertyFile {
                        kind: PropertyKindFileOrText::Property(PropertyKindFile::Widget(WidgetPropertyFile::Volume)),
                        style: Some(StyleFile {
                            fg: Some("blue".to_string()),
                            bg: None,
                            modifiers: None,
                        }),
                        default: None,
                    }],
                },
                HeaderConfigRowFile {
                    left: vec![
                        PropertyFile {
                            kind: PropertyKindFileOrText::Property(PropertyKindFile::Status(
                                StatusPropertyFile::Elapsed,
                            )),
                            default: None,
                            style: None,
                        },
                        PropertyFile {
                            kind: PropertyKindFileOrText::Text(" / ".to_string()),
                            default: None,
                            style: None,
                        },
                        PropertyFile {
                            kind: PropertyKindFileOrText::Property(PropertyKindFile::Status(
                                StatusPropertyFile::Duration,
                            )),
                            default: None,
                            style: None,
                        },
                        PropertyFile {
                            kind: PropertyKindFileOrText::Text(" (".to_string()),
                            default: None,
                            style: None,
                        },
                        PropertyFile {
                            kind: PropertyKindFileOrText::Property(PropertyKindFile::Status(
                                StatusPropertyFile::Bitrate,
                            )),
                            default: None,
                            style: None,
                        },
                        PropertyFile {
                            kind: PropertyKindFileOrText::Text(" kbps)".to_string()),
                            default: None,
                            style: None,
                        },
                    ],
                    center: vec![
                        PropertyFile {
                            kind: PropertyKindFileOrText::Property(PropertyKindFile::Song(SongPropertyFile::Artist)),
                            default: Some(Box::new(PropertyFile {
                                kind: PropertyKindFileOrText::Text("Unknown".to_string()),
                                style: Some(StyleFile {
                                    fg: Some("yellow".to_string()),
                                    bg: None,
                                    modifiers: Some(Modifiers::Bold),
                                }),
                                default: None,
                            })),
                            style: Some(StyleFile {
                                fg: Some("yellow".to_string()),
                                bg: None,
                                modifiers: Some(Modifiers::Bold),
                            }),
                        },
                        PropertyFile {
                            kind: PropertyKindFileOrText::Text(" - ".to_string()),
                            style: None,
                            default: None,
                        },
                        PropertyFile {
                            kind: PropertyKindFileOrText::Property(PropertyKindFile::Song(SongPropertyFile::Album)),
                            default: Some(Box::new(PropertyFile {
                                kind: PropertyKindFileOrText::Text("Unknown Album".to_string()),
                                style: None,
                                default: None,
                            })),
                            style: None,
                        },
                    ],
                    right: vec![PropertyFile {
                        kind: PropertyKindFileOrText::Property(PropertyKindFile::Widget(WidgetPropertyFile::States {
                            active_style: Some(StyleFile {
                                fg: Some("white".to_string()),
                                bg: None,
                                modifiers: Some(Modifiers::Bold),
                            }),
                            separator_style: Some(StyleFile {
                                fg: Some("white".to_string()),
                                bg: None,
                                modifiers: None,
                            }),
                        })),
                        style: Some(StyleFile {
                            fg: Some("dark_gray".to_string()),
                            bg: None,
                            modifiers: None,
                        }),
                        default: None,
                    }],
                },
            ],
        }
    }
}

impl TryFrom<HeaderConfigFile> for HeaderConfig {
    type Error = anyhow::Error;

    fn try_from(value: HeaderConfigFile) -> Result<Self, Self::Error> {
        Ok(Self {
            rows: value
                .rows
                .into_iter()
                .map(|row| -> Result<_> {
                    let left = row
                        .left
                        .into_iter()
                        .map(TryInto::<&'static Property<'static, PropertyKind>>::try_into)
                        .collect::<Result<Vec<_>>>()?;
                    let center = row
                        .center
                        .into_iter()
                        .map(TryInto::<&'static Property<'static, PropertyKind>>::try_into)
                        .collect::<Result<Vec<_>>>()?;
                    let right = row
                        .right
                        .into_iter()
                        .map(TryInto::<&'static Property<'static, PropertyKind>>::try_into)
                        .collect::<Result<Vec<_>>>()?;

                    Ok(HeaderConfigRow {
                        left: Box::leak(Box::new(left)),
                        center: Box::leak(Box::new(center)),
                        right: Box::leak(Box::new(right)),
                    })
                })
                .try_collect()?,
        })
    }
}
