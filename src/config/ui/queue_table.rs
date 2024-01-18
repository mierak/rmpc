use anyhow::Result;
use itertools::Itertools;
use ratatui::style::Color;
use serde::{Deserialize, Serialize};

use super::{color::StringColor, Alignment, SongProperty};

#[derive(Debug, Serialize, Deserialize)]
pub struct SongTableColumnFile {
    /// Property to display in the column
    /// Can be one of: Duration, Filename, Artist, AlbumArtist, Title, Album, Date, Genre or Comment    
    pub(super) prop: SongProperty,
    /// Label to display in the column header
    /// If not set, the property name will be used
    pub(super) label: Option<String>,
    /// Width of the column in percent
    pub(super) width_percent: u16,
    /// Foreground color of the column
    pub(super) color: Option<String>,
    /// Text alignment of the text in the column
    pub(super) alignment: Option<Alignment>,
}

#[derive(Debug, Copy, Clone)]
pub struct SongTableColumn {
    pub prop: SongProperty,
    pub label: &'static str,
    pub width_percent: u16,
    pub color: Color,
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
                prop: SongProperty::Artist,
                label: None,
                width_percent: 20,
                color: None,
                alignment: None,
            },
            SongTableColumnFile {
                prop: SongProperty::Title,
                label: None,
                width_percent: 35,
                color: None,
                alignment: None,
            },
            SongTableColumnFile {
                prop: SongProperty::Album,
                label: None,
                width_percent: 30,
                color: Some("white".to_string()),
                alignment: None,
            },
            SongTableColumnFile {
                prop: SongProperty::Duration,
                label: None,
                width_percent: 15,
                color: None,
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
                    Ok(SongTableColumn {
                        prop: v.prop,
                        label: Box::leak(Box::new(v.label.unwrap_or_else(|| v.prop.to_string()))),
                        width_percent: v.width_percent,
                        alignment: v.alignment.unwrap_or(Alignment::Left),
                        color: StringColor(v.color).to_color()?.unwrap_or(Color::White),
                    })
                })
                .try_collect()?,
        ))
    }
}
