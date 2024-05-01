use anyhow::Result;
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use serde_with::skip_serializing_none;

use super::properties::{
    Alignment, Property, PropertyFile, PropertyKindFileOrText, PropertyKindOrText, SongProperty, SongPropertyFile,
};
use super::style::ToConfigOr;
use super::StyleFile;

#[skip_serializing_none]
#[derive(Debug, Serialize, Deserialize)]
pub struct SongTableColumnFile {
    /// Property to display in the column
    /// Can be one of: Duration, Filename, Artist, AlbumArtist, Title, Album, Date, Genre or Comment    
    pub(super) prop: PropertyFile<SongPropertyFile>,
    /// Label to display in the column header
    /// If not set, the property name will be used
    pub(super) label: Option<String>,
    /// Width of the column in percent
    pub(super) width_percent: u16,
    /// Text alignment of the text in the column
    pub(super) alignment: Option<Alignment>,
}
impl std::fmt::Display for SongTableColumnFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?} {}% {:?}", self.prop, self.width_percent, self.alignment)
    }
}

#[derive(Debug, Clone)]
pub struct SongTableColumn {
    pub prop: &'static Property<'static, SongProperty>,
    pub label: &'static str,
    pub width_percent: u16,
    pub alignment: Alignment,
}

#[derive(Debug)]
pub(super) struct QueueTableColumns(pub Vec<SongTableColumn>);

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct QueueTableColumnsFile(pub Vec<SongTableColumnFile>);

impl Default for QueueTableColumnsFile {
    fn default() -> Self {
        QueueTableColumnsFile(vec![
            SongTableColumnFile {
                prop: PropertyFile {
                    kind: PropertyKindFileOrText::Property(SongPropertyFile::Artist),
                    default: Some(Box::new(PropertyFile {
                        kind: PropertyKindFileOrText::Text {
                            value: "Unknown".to_string(),
                        },
                        style: None,
                        default: None,
                    })),
                    style: None,
                },
                label: None,
                width_percent: 20,
                alignment: None,
            },
            SongTableColumnFile {
                prop: PropertyFile {
                    kind: PropertyKindFileOrText::Property(SongPropertyFile::Title),
                    default: Some(Box::new(PropertyFile {
                        kind: PropertyKindFileOrText::Text {
                            value: "Unknown".to_string(),
                        },
                        style: None,
                        default: None,
                    })),
                    style: None,
                },
                label: None,
                width_percent: 35,
                alignment: None,
            },
            SongTableColumnFile {
                prop: PropertyFile {
                    kind: PropertyKindFileOrText::Property(SongPropertyFile::Album),
                    default: Some(Box::new(PropertyFile {
                        kind: PropertyKindFileOrText::Text {
                            value: "Album".to_string(),
                        },
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
                width_percent: 30,
                alignment: None,
            },
            SongTableColumnFile {
                prop: PropertyFile {
                    kind: PropertyKindFileOrText::Property(SongPropertyFile::Duration),
                    default: Some(Box::new(PropertyFile {
                        kind: PropertyKindFileOrText::Text { value: "-".to_string() },
                        style: None,
                        default: None,
                    })),
                    style: None,
                },
                label: None,
                width_percent: 15,
                alignment: Some(Alignment::Right),
            },
        ])
    }
}

impl TryFrom<QueueTableColumnsFile> for QueueTableColumns {
    type Error = anyhow::Error;

    fn try_from(value: QueueTableColumnsFile) -> Result<Self, Self::Error> {
        if value.0.iter().map(|v| v.width_percent).sum::<u16>() > 100 {
            anyhow::bail!("Song table format width percent sum is greater than 100");
        }

        Ok(QueueTableColumns(
            value
                .0
                .into_iter()
                .map(|v| -> Result<_> {
                    let prop: Property<SongProperty> = v.prop.try_into()?;
                    let label = v.label.unwrap_or_else(|| match &prop.kind {
                        PropertyKindOrText::Text { .. } => String::new(),
                        PropertyKindOrText::Property(prop) => prop.to_string(),
                    });
                    Ok(SongTableColumn {
                        prop: Box::leak(Box::new(prop)),
                        label: Box::leak(Box::new(label)),
                        width_percent: v.width_percent,
                        alignment: v.alignment.unwrap_or(Alignment::Left),
                    })
                })
                .try_collect()?,
        ))
    }
}

impl TryFrom<PropertyFile<SongPropertyFile>> for &'static Property<'static, SongProperty> {
    type Error = anyhow::Error;

    fn try_from(value: PropertyFile<SongPropertyFile>) -> std::prelude::v1::Result<Self, Self::Error> {
        Property::<'static, SongProperty>::try_from(value)
            .map(|v| Box::leak(Box::new(v)))
            .map(|v| {
                let v: &'static Property<_> = v;
                v
            })
    }
}
impl TryFrom<PropertyFile<SongPropertyFile>> for Property<'static, SongProperty> {
    type Error = anyhow::Error;

    fn try_from(value: PropertyFile<SongPropertyFile>) -> std::result::Result<Self, Self::Error> {
        Ok(Self {
            kind: match value.kind {
                PropertyKindFileOrText::Text { value } => PropertyKindOrText::Text { value },
                PropertyKindFileOrText::Property(prop) => PropertyKindOrText::Property(prop.try_into()?),
            },
            style: Some(value.style.to_config_or(None, None)?),
            default: value
                .default
                .map(|v| TryFrom::<PropertyFile<SongPropertyFile>>::try_from(*v))
                .transpose()?,
        })
    }
}
