use anyhow::Result;
use itertools::Itertools;
use serde::{Deserialize, Serialize};

use super::properties::{Alignment, SongProperty, SongPropertyFile};
use super::StyleFile;

#[derive(Debug, Serialize, Deserialize)]
pub struct SongTableColumnFile {
    /// Property to display in the column
    /// Can be one of: Duration, Filename, Artist, AlbumArtist, Title, Album, Date, Genre or Comment    
    pub(super) prop: SongPropertyFile,
    /// Label to display in the column header
    /// If not set, the property name will be used
    pub(super) label: Option<String>,
    /// Width of the column in percent
    pub(super) width_percent: u16,
    /// Text alignment of the text in the column
    pub(super) alignment: Option<Alignment>,
}

#[derive(Debug, Copy, Clone)]
pub struct SongTableColumn {
    pub prop: SongProperty,
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
                prop: SongPropertyFile::Artist {
                    style: None,
                    default: "Unknown".to_string(),
                },
                label: None,
                width_percent: 20,
                alignment: None,
            },
            SongTableColumnFile {
                prop: SongPropertyFile::Title {
                    style: None,
                    default: "Unknown".to_string(),
                },
                label: None,
                width_percent: 35,
                alignment: None,
            },
            SongTableColumnFile {
                prop: SongPropertyFile::Album {
                    style: Some(StyleFile {
                        fg: Some("white".to_string()),
                        bg: None,
                        modifiers: None,
                    }),
                    default: "Unknown Album".to_string(),
                },
                label: None,
                width_percent: 30,
                alignment: None,
            },
            SongTableColumnFile {
                prop: SongPropertyFile::Duration {
                    style: None,
                    default: "-".to_string(),
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
                    let prop: SongProperty = v.prop.try_into()?;
                    Ok(SongTableColumn {
                        prop,
                        label: Box::leak(Box::new(v.label.unwrap_or_else(|| prop.to_string()))),
                        width_percent: v.width_percent,
                        alignment: v.alignment.unwrap_or(Alignment::Left),
                    })
                })
                .try_collect()?,
        ))
    }
}
