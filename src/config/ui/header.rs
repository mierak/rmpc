use anyhow::Result;
use serde::{Deserialize, Serialize};

use super::properties::{
    Property, PropertyFile, PropertyKind, PropertyKindFile, PropertyKindFileOrText, SongPropertyFile,
    StatusPropertyFile, WidgetPropertyFile,
};
use super::style::{Modifiers, StyleFile};

#[derive(Debug, Default)]
pub struct HeaderConfig {
    pub top_center: &'static [&'static Property<'static, PropertyKind>],
    pub bottom_center: &'static [&'static Property<'static, PropertyKind>],
    pub top_left: &'static [&'static Property<'static, PropertyKind>],
    pub bottom_left: &'static [&'static Property<'static, PropertyKind>],
    pub top_right: &'static [&'static Property<'static, PropertyKind>],
    pub bottom_right: &'static [&'static Property<'static, PropertyKind>],
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HeaderConfigFile {
    pub(super) top_center: Vec<PropertyFile<PropertyKindFile>>,
    pub(super) bottom_center: Vec<PropertyFile<PropertyKindFile>>,
    pub(super) top_left: Vec<PropertyFile<PropertyKindFile>>,
    pub(super) bottom_left: Vec<PropertyFile<PropertyKindFile>>,
    pub(super) top_right: Vec<PropertyFile<PropertyKindFile>>,
    pub(super) bottom_right: Vec<PropertyFile<PropertyKindFile>>,
}

impl Default for HeaderConfigFile {
    fn default() -> Self {
        Self {
            top_center: vec![PropertyFile {
                kind: PropertyKindFileOrText::Property(PropertyKindFile::Song(SongPropertyFile::Title)),
                default: Some(Box::new(PropertyFile {
                    kind: PropertyKindFileOrText::Text {
                        value: "No Song".to_string(),
                    },
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
            bottom_center: vec![
                PropertyFile {
                    kind: PropertyKindFileOrText::Property(PropertyKindFile::Song(SongPropertyFile::Artist)),
                    default: Some(Box::new(PropertyFile {
                        kind: PropertyKindFileOrText::Text {
                            value: "Unknown".to_string(),
                        },
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
                    kind: PropertyKindFileOrText::Text {
                        value: " - ".to_string(),
                    },
                    style: None,
                    default: None,
                },
                PropertyFile {
                    kind: PropertyKindFileOrText::Property(PropertyKindFile::Song(SongPropertyFile::Album)),
                    default: Some(Box::new(PropertyFile {
                        kind: PropertyKindFileOrText::Text {
                            value: "Unknown Album".to_string(),
                        },
                        style: None,
                        default: None,
                    })),
                    style: None,
                },
            ],
            top_left: vec![
                PropertyFile {
                    kind: PropertyKindFileOrText::Text { value: "[".to_string() },
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
                    kind: PropertyKindFileOrText::Text { value: "]".to_string() },
                    style: Some(StyleFile {
                        fg: Some("yellow".to_string()),
                        bg: None,
                        modifiers: Some(Modifiers::Bold),
                    }),
                    default: None,
                },
            ],
            bottom_left: vec![
                PropertyFile {
                    kind: PropertyKindFileOrText::Property(PropertyKindFile::Status(StatusPropertyFile::Elapsed)),
                    default: None,
                    style: None,
                },
                PropertyFile {
                    kind: PropertyKindFileOrText::Text {
                        value: " / ".to_string(),
                    },
                    default: None,
                    style: None,
                },
                PropertyFile {
                    kind: PropertyKindFileOrText::Property(PropertyKindFile::Status(StatusPropertyFile::Duration)),
                    default: None,
                    style: None,
                },
                PropertyFile {
                    kind: PropertyKindFileOrText::Text {
                        value: " (".to_string(),
                    },
                    default: None,
                    style: None,
                },
                PropertyFile {
                    kind: PropertyKindFileOrText::Property(PropertyKindFile::Status(StatusPropertyFile::Bitrate)),
                    default: None,
                    style: None,
                },
                PropertyFile {
                    kind: PropertyKindFileOrText::Text {
                        value: " kbps)".to_string(),
                    },
                    default: None,
                    style: None,
                },
            ],
            top_right: vec![PropertyFile {
                kind: PropertyKindFileOrText::Property(PropertyKindFile::Widget(WidgetPropertyFile::Volume)),
                style: Some(StyleFile {
                    fg: Some("blue".to_string()),
                    bg: None,
                    modifiers: None,
                }),
                default: None,
            }],
            bottom_right: vec![PropertyFile {
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
        }
    }
}

impl TryFrom<HeaderConfigFile> for HeaderConfig {
    type Error = anyhow::Error;

    fn try_from(value: HeaderConfigFile) -> Result<Self, Self::Error> {
        let top_left2 = value
            .top_left
            .into_iter()
            .map(TryInto::<&'static Property<'static, PropertyKind>>::try_into)
            .collect::<Result<Vec<_>>>()?;

        let top_left: &'static [&'static Property<'static, PropertyKind>] = Box::leak(Box::new(top_left2));

        let top_center = value
            .top_center
            .into_iter()
            .map(TryInto::<&'static Property<'static, PropertyKind>>::try_into)
            .collect::<Result<Vec<_>>>()?;
        let top_center: &'static [&'static Property<'static, PropertyKind>] = Box::leak(Box::new(top_center));

        let top_right = value
            .top_right
            .into_iter()
            .map(TryInto::<&'static Property<'static, PropertyKind>>::try_into)
            .collect::<Result<Vec<_>>>()?;
        let top_right: &'static [&'static Property<'static, PropertyKind>] = Box::leak(Box::new(top_right));

        let bottom_left = value
            .bottom_left
            .into_iter()
            .map(TryInto::<&'static Property<'static, PropertyKind>>::try_into)
            .collect::<Result<Vec<_>>>()?;
        let bottom_left: &'static [&'static Property<'static, PropertyKind>] = Box::leak(Box::new(bottom_left));

        let bottom_center = value
            .bottom_center
            .into_iter()
            .map(TryInto::<&'static Property<'static, PropertyKind>>::try_into)
            .collect::<Result<Vec<_>>>()?;
        let bottom_center: &'static [&'static Property<'static, PropertyKind>] = Box::leak(Box::new(bottom_center));

        let bottom_right = value
            .bottom_right
            .into_iter()
            .map(TryInto::<&'static Property<'static, PropertyKind>>::try_into)
            .collect::<Result<Vec<_>>>()?;
        let bottom_right: &'static [&'static Property<'static, PropertyKind>] = Box::leak(Box::new(bottom_right));

        Ok(Self {
            top_center,
            bottom_center,
            top_left,
            bottom_left,
            top_right,
            bottom_right,
        })
    }
}
