use std::num::{ParseFloatError, ParseIntError};

use anyhow::{Context, Result, bail};
use itertools::Itertools;
use ratatui::layout::Constraint;
use serde::{Deserialize, Serialize};
use serde_with::skip_serializing_none;
use thiserror::Error;

use super::{
    StyleFile,
    properties::{
        Alignment,
        Property,
        PropertyFile,
        PropertyKindFileOrText,
        PropertyKindOrText,
        SongProperty,
        SongPropertyFile,
        Transform,
        TransformFile,
    },
    style::ToConfigOr,
};

#[derive(Debug, Clone, Copy)]
pub enum PercentOrLength {
    Percent(u16),
    Length(u16),
    Ratio(f64),
}

impl PercentOrLength {
    pub fn into_constraint(self, parent_size: u16) -> Constraint {
        match self {
            PercentOrLength::Percent(val) => Constraint::Percentage(val),
            PercentOrLength::Length(val) => Constraint::Length(val),
            PercentOrLength::Ratio(val) => {
                Constraint::Length((f64::from(parent_size) * val).round() as u16)
            }
        }
    }
}

#[derive(Error, Debug)]
pub enum ParseSizeError {
    #[error("Invalid size format: '{0}'")]
    ParseIntError(#[from] ParseIntError),
    #[error("Invalid size format: '{0}'")]
    ParseFloatError(#[from] ParseFloatError),
}

impl std::str::FromStr for PercentOrLength {
    type Err = ParseSizeError;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        if s.ends_with('r') {
            Ok(PercentOrLength::Ratio(s.trim_end_matches('r').parse()?))
        } else if s.ends_with('%') {
            Ok(PercentOrLength::Percent(s.trim_end_matches('%').parse()?))
        } else {
            Ok(PercentOrLength::Length(s.parse()?))
        }
    }
}

#[skip_serializing_none]
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SongTableColumnFile {
    /// Property to display in the column
    /// Can be one of: `Duration`, `Filename`, `Artist`, `AlbumArtist`, `Title`,
    /// `Album`, `Date`, `Genre` or `Comment`
    pub(super) prop: PropertyFile<SongPropertyFile>,
    /// Label to display in the column header
    /// If not set, the property name will be used
    pub(super) label: Option<String>,
    pub(super) label_prop: Option<PropertyFile<SongPropertyFile>>,
    /// Width of the column in percent
    pub(super) width_percent: Option<u16>,
    pub(super) width: Option<String>,
    /// Text alignment of the text in the column
    pub(super) alignment: Option<Alignment>,
}

#[derive(Debug, Clone)]
pub struct SongTableColumn {
    pub prop: Property<SongProperty>,
    pub label: Property<SongProperty>,
    pub width: PercentOrLength,
    pub alignment: Alignment,
}

#[derive(Debug)]
pub(super) struct QueueTableColumns(pub Vec<SongTableColumn>);

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct QueueTableColumnsFile(pub Vec<SongTableColumnFile>);

impl Default for QueueTableColumnsFile {
    fn default() -> Self {
        QueueTableColumnsFile(vec![
            SongTableColumnFile {
                prop: PropertyFile {
                    kind: PropertyKindFileOrText::Property(SongPropertyFile::Artist),
                    default: Some(Box::new(PropertyFile {
                        kind: PropertyKindFileOrText::Text("Unknown".to_string()),
                        style: None,
                        default: None,
                    })),
                    style: None,
                },
                label: None,
                label_prop: Some(PropertyFile {
                    kind: PropertyKindFileOrText::Text("Artist".to_string()),
                    default: None,
                    style: None,
                }),
                width_percent: None,
                width: Some("20%".to_string()),
                alignment: None,
            },
            SongTableColumnFile {
                prop: PropertyFile {
                    kind: PropertyKindFileOrText::Property(SongPropertyFile::Title),
                    default: Some(Box::new(PropertyFile {
                        kind: PropertyKindFileOrText::Text("Unknown".to_string()),
                        style: None,
                        default: None,
                    })),
                    style: None,
                },
                label: None,
                label_prop: Some(PropertyFile {
                    kind: PropertyKindFileOrText::Text("Title".to_string()),
                    default: None,
                    style: None,
                }),
                width_percent: None,
                width: Some("35%".to_string()),
                alignment: None,
            },
            SongTableColumnFile {
                prop: PropertyFile {
                    kind: PropertyKindFileOrText::Property(SongPropertyFile::Album),
                    default: Some(Box::new(PropertyFile {
                        kind: PropertyKindFileOrText::Text("Unknown Album".to_string()),
                        style: Some(StyleFile {
                            fg: Some("white".to_string()),
                            bg: None,
                            modifiers: None,
                        }),
                        default: None,
                    })),
                    style: Some(StyleFile {
                        fg: Some("white".to_string()),
                        bg: None,
                        modifiers: None,
                    }),
                },
                label: None,
                label_prop: Some(PropertyFile {
                    kind: PropertyKindFileOrText::Text("Album".to_string()),
                    default: None,
                    style: None,
                }),
                width_percent: None,
                width: Some("30%".to_string()),
                alignment: None,
            },
            SongTableColumnFile {
                prop: PropertyFile {
                    kind: PropertyKindFileOrText::Property(SongPropertyFile::Duration),
                    default: Some(Box::new(PropertyFile {
                        kind: PropertyKindFileOrText::Text("-".to_string()),
                        style: None,
                        default: None,
                    })),
                    style: None,
                },
                label: None,
                label_prop: Some(PropertyFile {
                    kind: PropertyKindFileOrText::Text("Duration".to_string()),
                    default: None,
                    style: None,
                }),
                width_percent: None,
                width: Some("15%".to_string()),
                alignment: Some(Alignment::Right),
            },
        ])
    }
}

impl TryFrom<QueueTableColumnsFile> for QueueTableColumns {
    type Error = anyhow::Error;

    fn try_from(value: QueueTableColumnsFile) -> Result<Self, Self::Error> {
        Ok(QueueTableColumns(
            value
                .0
                .into_iter()
                .map(|v| -> Result<_> {
                    let prop: Property<SongProperty> = v.prop.convert()?;

                    let text = |label: String| Property {
                        kind: PropertyKindOrText::Text(label),
                        default: None,
                        style: None,
                    };

                    let label = match (v.label, v.label_prop) {
                        (Some(_), Some(new)) | (None, Some(new)) => {
                            new.convert_text_group_only()?
                        }
                        (Some(old), None) => text(old.clone()),
                        (None, None) => match &prop.kind {
                            PropertyKindOrText::Text { .. } => text(String::new()),
                            PropertyKindOrText::Sticker { .. } => text(String::new()),
                            PropertyKindOrText::Transform { .. } => text(String::new()),
                            PropertyKindOrText::Property(prop) => text(prop.to_string()),
                            PropertyKindOrText::Group(_) => text(String::new()),
                        },
                    };

                    Ok(SongTableColumn {
                        prop,
                        label,
                        width: v
                            .width
                            .as_ref()
                            .map_or_else(
                                || -> Result<Option<PercentOrLength>> {
                                    Ok(v.width_percent.map(PercentOrLength::Percent))
                                },
                                |width| -> Result<Option<PercentOrLength>> {
                                    match width.parse() {
                                        Ok(PercentOrLength::Ratio(_)) => {
                                            bail!("song_table_format cannot contain ratio widths.")
                                        }
                                        Ok(val) => Ok(Some(val)),
                                        Err(err) => {
                                            bail!("Invalid width format: '{width}'. Error: {err}")
                                        }
                                    }
                                },
                            )
                            .context("Failed to parse width in song table column width")?
                            .context(
                                "Invalid width config. Song table column width must be specified",
                            )?,
                        alignment: v.alignment.unwrap_or(Alignment::Left),
                    })
                })
                .try_collect()?,
        ))
    }
}

impl PropertyFile<SongPropertyFile> {
    pub fn convert(self) -> Result<Property<SongProperty>> {
        Ok(Property {
            kind: match self.kind {
                PropertyKindFileOrText::Text(value) => PropertyKindOrText::Text(value),
                PropertyKindFileOrText::Transform(TransformFile::Truncate {
                    content,
                    length,
                    from_start,
                }) => PropertyKindOrText::Transform(Transform::Truncate {
                    content: Box::new((*content).convert()?),
                    length,
                    from_start,
                }),
                PropertyKindFileOrText::Transform(TransformFile::Replace {
                    content,
                    replacements,
                }) => PropertyKindOrText::Transform(Transform::Replace {
                    content: Box::new((*content).convert()?),
                    replacements: replacements
                        .into_iter()
                        .map(|r| -> Result<_> { Ok((r.r#match, r.replace.convert()?)) })
                        .try_collect()?,
                }),
                PropertyKindFileOrText::Sticker(value) => PropertyKindOrText::Sticker(value),
                PropertyKindFileOrText::Property(prop) => PropertyKindOrText::Property(prop.into()),
                PropertyKindFileOrText::Group(group) => {
                    let res: Vec<_> = group
                        .into_iter()
                        .map(|p| -> Result<Property<SongProperty>> { p.convert() })
                        .try_collect()?;
                    PropertyKindOrText::Group(res)
                }
            },
            style: Some(self.style.to_config_or(None, None)?),
            default: self
                .default
                .map(|v| -> Result<_> { Ok(Box::new((*v).convert()?)) })
                .transpose()?,
        })
    }

    fn convert_text_group_only(self) -> Result<Property<SongProperty>> {
        Ok(Property {
            kind: match self.kind {
                PropertyKindFileOrText::Transform(_) => {
                    bail!("Transforms are not supported in the label")
                }
                PropertyKindFileOrText::Sticker(_) => {
                    bail!("Stickers are not supported in the label")
                }
                PropertyKindFileOrText::Property(_) => {
                    bail!("Properties are not supported in the label")
                }
                PropertyKindFileOrText::Text(value) => PropertyKindOrText::Text(value),
                PropertyKindFileOrText::Group(group) => {
                    let res: Vec<_> = group
                        .into_iter()
                        .map(|p| -> Result<Property<SongProperty>> { p.convert_text_group_only() })
                        .try_collect()?;
                    PropertyKindOrText::Group(res)
                }
            },
            style: Some(self.style.to_config_or(None, None)?),
            default: self
                .default
                .map(|v| -> Result<_> { Ok(Box::new((*v).convert_text_group_only()?)) })
                .transpose()?,
        })
    }
}
