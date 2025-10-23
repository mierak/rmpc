use std::num::{ParseFloatError, ParseIntError};

use anyhow::{Context, Result, bail};
use chumsky::Parser;
use itertools::Itertools;
use ratatui::layout::Constraint;
use serde::{Deserialize, Serialize};
use serde_with::skip_serializing_none;
use thiserror::Error;

use super::{
    SongFormatOrProps,
    StyleFile,
    parser::{self, make_error_report},
    properties::{
        Alignment,
        Property,
        PropertyFile,
        PropertyKindFile,
        PropertyKindFileOrText,
        PropertyKindOrText,
        SongProperty,
        SongPropertyFile,
        Transform,
        TransformFile,
    },
    style::ToConfigOr,
};
use crate::config::tabs::PaneConversionError;

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
    pub(super) prop: SongFormatOrProps<PropertyFile<SongPropertyFile>>,
    /// Label to display in the column header
    /// If not set, the property name will be used
    pub(super) label: Option<String>,
    /// Width of the column in percent
    pub(super) width_percent: Option<u16>,
    pub(super) width: Option<String>,
    /// Text alignment of the text in the column
    pub(super) alignment: Option<Alignment>,
}

#[derive(Debug, Clone)]
pub struct SongTableColumn {
    pub prop: Property<SongProperty>,
    pub label: String,
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
                prop: SongFormatOrProps::Props(PropertyFile {
                    kind: PropertyKindFileOrText::Property(SongPropertyFile::Artist),
                    default: Some(Box::new(PropertyFile {
                        kind: PropertyKindFileOrText::Text("Unknown".to_string()),
                        style: None,
                        default: None,
                    })),
                    style: None,
                }),
                label: None,
                width_percent: None,
                width: Some("20%".to_string()),
                alignment: None,
            },
            SongTableColumnFile {
                prop: SongFormatOrProps::Props(PropertyFile {
                    kind: PropertyKindFileOrText::Property(SongPropertyFile::Title),
                    default: Some(Box::new(PropertyFile {
                        kind: PropertyKindFileOrText::Text("Unknown".to_string()),
                        style: None,
                        default: None,
                    })),
                    style: None,
                }),
                label: None,
                width_percent: None,
                width: Some("35%".to_string()),
                alignment: None,
            },
            SongTableColumnFile {
                prop: SongFormatOrProps::Props(PropertyFile {
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
                }),
                label: None,
                width_percent: None,
                width: Some("30%".to_string()),
                alignment: None,
            },
            SongTableColumnFile {
                prop: SongFormatOrProps::Props(PropertyFile {
                    kind: PropertyKindFileOrText::Property(SongPropertyFile::Duration),
                    default: Some(Box::new(PropertyFile {
                        kind: PropertyKindFileOrText::Text("-".to_string()),
                        style: None,
                        default: None,
                    })),
                    style: None,
                }),
                label: None,
                width_percent: None,
                width: Some("15%".to_string()),
                alignment: Some(Alignment::Right),
            },
        ])
    }
}

impl TryFrom<QueueTableColumnsFile> for QueueTableColumns {
    type Error = PaneConversionError;

    fn try_from(value: QueueTableColumnsFile) -> Result<Self, Self::Error> {
        Ok(QueueTableColumns(
            value
                .0
                .into_iter()
                .map(|v| -> Result<_, PaneConversionError> {
                    let prop: Property<_> = match v.prop {
                        SongFormatOrProps::Format(s) => parser::parser()
                            .parse(&s)
                            .into_result()
                            .map_err(|errs| {
                                PaneConversionError::FormatError(make_error_report(errs, &s))
                            })?
                            .into_iter()
                            .next()
                            .context("A song property must be specified")?
                            .try_into()?,
                        SongFormatOrProps::Props(p) => p.try_into()?,
                    };
                    let label = v.label.unwrap_or_else(|| match &prop.kind {
                        PropertyKindOrText::Text { .. } => String::new(),
                        PropertyKindOrText::Sticker { .. } => String::new(),
                        PropertyKindOrText::Transform { .. } => String::new(),
                        PropertyKindOrText::Property(prop) => prop.to_string(),
                        PropertyKindOrText::Group(_) => String::new(),
                    });

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
                                            bail!(
                                                "Invalid width format: '{}'. Error: {}",
                                                width,
                                                err
                                            )
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

impl TryFrom<PropertyFile<PropertyKindFile>> for Property<SongProperty> {
    type Error = anyhow::Error;

    fn try_from(value: PropertyFile<PropertyKindFile>) -> Result<Self, Self::Error> {
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
                PropertyKindFileOrText::Property(PropertyKindFile::Song(prop)) => {
                    PropertyKindOrText::Property(prop.into())
                }
                PropertyKindFileOrText::Property(_) => {
                    bail!("Only song properties are allowed in the song table")
                }
                PropertyKindFileOrText::Group(group) => {
                    let res: Vec<_> = group
                        .into_iter()
                        .map(|p| -> Result<Property<SongProperty>> { p.try_into() })
                        .try_collect()?;
                    PropertyKindOrText::Group(res)
                }
            },
            style: Some(value.style.to_config_or(None, None)?),
            default: value
                .default
                .map(|v| -> Result<_> {
                    let s: Property<SongProperty> = (*v).try_into()?;
                    Ok(Box::new(s))
                })
                .transpose()?,
        })
    }
}

impl TryFrom<PropertyFile<SongPropertyFile>> for Property<SongProperty> {
    type Error = anyhow::Error;

    fn try_from(value: PropertyFile<SongPropertyFile>) -> std::result::Result<Self, Self::Error> {
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
                PropertyKindFileOrText::Property(prop) => PropertyKindOrText::Property(prop.into()),
                PropertyKindFileOrText::Group(group) => {
                    let res: Vec<_> = group
                        .into_iter()
                        .map(|p| -> Result<Property<SongProperty>> { p.try_into() })
                        .try_collect()?;
                    PropertyKindOrText::Group(res)
                }
            },
            style: Some(value.style.to_config_or(None, None)?),
            default: value
                .default
                .map(|v| -> Result<_> {
                    Ok(Box::new(TryFrom::<PropertyFile<SongPropertyFile>>::try_from(*v)?))
                })
                .transpose()?,
        })
    }
}
