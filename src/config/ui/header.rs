use itertools::Itertools;
use serde::{Deserialize, Serialize};

use super::properties::{Property, PropertyFile, SongPropertyFile, StatusPropertyFile, WidgetPropertyFile};
use super::style::{Modifiers, StyleFile};

#[derive(Debug)]
pub struct HeaderConfig {
    pub top_center: Vec<Property>,
    pub bottom_center: Vec<Property>,
    pub top_left: Vec<Property>,
    pub bottom_left: Vec<Property>,
    pub top_right: Vec<Property>,
    pub bottom_right: Vec<Property>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HeaderConfigFile {
    pub(super) top_center: Vec<PropertyFile>,
    pub(super) bottom_center: Vec<PropertyFile>,
    pub(super) top_left: Vec<PropertyFile>,
    pub(super) bottom_left: Vec<PropertyFile>,
    pub(super) top_right: Vec<PropertyFile>,
    pub(super) bottom_right: Vec<PropertyFile>,
}

impl Default for HeaderConfigFile {
    fn default() -> Self {
        Self {
            top_center: vec![PropertyFile::Song(SongPropertyFile::Title {
                default: "No Song".to_string(),
                style: Some(StyleFile {
                    fg: None,
                    bg: None,
                    modifiers: Some(Modifiers::Bold),
                }),
            })],
            bottom_center: vec![
                PropertyFile::Song(SongPropertyFile::Artist {
                    default: "Unknown".to_string(),
                    style: Some(StyleFile {
                        fg: Some("yellow".to_string()),
                        bg: None,
                        modifiers: Some(Modifiers::Bold),
                    }),
                }),
                PropertyFile::Text {
                    value: " - ".to_string(),
                    style: None,
                },
                PropertyFile::Song(SongPropertyFile::Album {
                    default: "Unknown Album".to_string(),
                    style: Some(StyleFile {
                        fg: Some("blue".to_string()),
                        bg: None,
                        modifiers: Some(Modifiers::Bold),
                    }),
                }),
            ],
            top_left: vec![
                PropertyFile::Text {
                    value: "[".to_string(),
                    style: Some(StyleFile {
                        fg: Some("yellow".to_string()),
                        bg: None,
                        modifiers: Some(Modifiers::Bold),
                    }),
                },
                PropertyFile::Status(StatusPropertyFile::State {
                    style: Some(StyleFile {
                        fg: Some("yellow".to_string()),
                        bg: None,
                        modifiers: Some(Modifiers::Bold),
                    }),
                }),
                PropertyFile::Text {
                    value: "]".to_string(),
                    style: Some(StyleFile {
                        fg: Some("yellow".to_string()),
                        bg: None,
                        modifiers: Some(Modifiers::Bold),
                    }),
                },
            ],
            bottom_left: vec![
                PropertyFile::Status(StatusPropertyFile::Elapsed { style: None }),
                PropertyFile::Text {
                    value: "/".to_string(),
                    style: None,
                },
                PropertyFile::Text {
                    value: " (".to_string(),
                    style: None,
                },
                PropertyFile::Status(StatusPropertyFile::Bitrate {
                    style: None,
                    default: "-".to_string(),
                }),
                PropertyFile::Text {
                    value: " kbps)".to_string(),
                    style: None,
                },
            ],
            top_right: vec![PropertyFile::Widget(WidgetPropertyFile::Volume {
                style: Some(StyleFile {
                    fg: Some("blue".to_string()),
                    bg: None,
                    modifiers: None,
                }),
            })],
            bottom_right: vec![PropertyFile::Widget(WidgetPropertyFile::States {
                active_style: Some(StyleFile {
                    fg: Some("white".to_string()),
                    bg: None,
                    modifiers: Some(Modifiers::Bold),
                }),
                inactive_style: Some(StyleFile {
                    fg: Some("dark_gray".to_string()),
                    bg: None,
                    modifiers: None,
                }),
                separator_style: Some(StyleFile {
                    fg: Some("white".to_string()),
                    bg: None,
                    modifiers: None,
                }),
            })],
        }
    }
}

impl TryFrom<HeaderConfigFile> for HeaderConfig {
    type Error = anyhow::Error;

    fn try_from(value: HeaderConfigFile) -> Result<Self, Self::Error> {
        Ok(Self {
            top_left: value
                .top_left
                .into_iter()
                .map(TryInto::<Property>::try_into)
                .try_collect()?,
            top_center: value
                .top_center
                .into_iter()
                .map(TryInto::<Property>::try_into)
                .try_collect()?,
            top_right: value
                .top_right
                .into_iter()
                .map(TryInto::<Property>::try_into)
                .try_collect()?,
            bottom_left: value
                .bottom_left
                .into_iter()
                .map(TryInto::<Property>::try_into)
                .try_collect()?,
            bottom_center: value
                .bottom_center
                .into_iter()
                .map(TryInto::<Property>::try_into)
                .try_collect()?,
            bottom_right: value
                .bottom_right
                .into_iter()
                .map(TryInto::<Property>::try_into)
                .try_collect()?,
        })
    }
}
