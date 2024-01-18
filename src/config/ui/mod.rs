use anyhow::Result;
use ratatui::style::{Color, Style};
use serde::{Deserialize, Serialize};
use strum::Display;

use self::{
    color::{FgBgColorsExt, Modifiers, StringColor},
    progress_bar::{ProgressBarConfig, ProgressBarConfigFile},
    queue_table::{QueueTableColumns, QueueTableColumnsFile, SongTableColumn},
    scrollbar::{ScrollbarConfig, ScrollbarConfigFile},
};

use super::defaults;

mod color;
mod progress_bar;
mod queue_table;
mod scrollbar;

pub use color::{ConfigColor, StyleFile};

#[derive(Debug)]
pub struct UiConfig {
    pub disable_images: bool,
    pub background_color: Option<Color>,
    pub background_color_modal: Option<Color>,
    pub borders_color: Color,
    pub current_song_color: Color,
    pub highlight_style: Style,
    pub highlight_border_style: Style,
    pub active_tab_style: Style,
    pub inactive_tab_style: Style,
    pub column_widths: [u16; 3],
    pub symbols: SymbolsConfig,
    pub volume_color: Color,
    pub status_color: Color,
    pub progress_bar: ProgressBarConfig,
    pub scrollbar: ScrollbarConfig,
    pub show_song_table_header: bool,
    pub song_table_format: Vec<SongTableColumn>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UiConfigFile {
    #[serde(default = "defaults::default_false")]
    pub(super) disable_images: bool,
    pub(super) symbols: SymbolsFile,
    pub(super) progress_bar: ProgressBarConfigFile,
    pub(super) scrollbar: ScrollbarConfigFile,
    #[serde(default = "defaults::default_column_widths")]
    pub(super) browser_column_widths: Vec<u16>,
    pub(super) background_color: Option<String>,
    pub(super) background_color_modal: Option<String>,
    pub(super) active_tab_style: Option<StyleFile>,
    pub(super) inactive_tab_style: Option<StyleFile>,
    pub(super) borders_color: Option<String>,
    pub(super) current_song_color: Option<String>,
    pub(super) highlight_style: Option<StyleFile>,
    pub(super) highlight_border_style: Option<StyleFile>,
    pub(super) volume_color: Option<String>,
    pub(super) status_color: Option<String>,
    pub(super) show_song_table_header: bool,
    pub(super) song_table_format: QueueTableColumnsFile,
}

impl Default for UiConfigFile {
    fn default() -> Self {
        Self {
            disable_images: false,
            background_color: None,
            background_color_modal: None,
            borders_color: Some("blue".to_string()),
            current_song_color: Some("blue".to_string()),
            highlight_style: Some(StyleFile {
                fg_color: Some("black".to_string()),
                bg_color: Some("blue".to_string()),
                modifiers: None,
            }),
            highlight_border_style: Some(StyleFile {
                fg_color: Some("blue".to_string()),
                bg_color: None,
                modifiers: None,
            }),
            active_tab_style: Some(StyleFile {
                fg_color: Some("black".to_string()),
                bg_color: Some("blue".to_string()),
                modifiers: Some(Modifiers::Bold),
            }),
            inactive_tab_style: Some(StyleFile {
                fg_color: None,
                bg_color: None,
                modifiers: None,
            }),
            browser_column_widths: vec![20, 38, 42],
            volume_color: Some("blue".to_string()),
            status_color: Some("yellow".to_string()),
            progress_bar: ProgressBarConfigFile::default(),
            scrollbar: ScrollbarConfigFile::default(),
            symbols: SymbolsFile {
                song: "🎵".to_owned(),
                dir: "📁".to_owned(),
                marker: "".to_owned(),
            },
            show_song_table_header: true,
            song_table_format: QueueTableColumnsFile::default(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SymbolsFile {
    pub(super) song: String,
    pub(super) dir: String,
    pub(super) marker: String,
}

#[derive(Debug, Default)]
pub struct SymbolsConfig {
    pub song: &'static str,
    pub dir: &'static str,
    pub marker: &'static str,
}

impl From<SymbolsFile> for SymbolsConfig {
    fn from(value: SymbolsFile) -> Self {
        Self {
            song: Box::leak(Box::new(value.song)),
            dir: Box::leak(Box::new(value.dir)),
            marker: Box::leak(Box::new(value.marker)),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Copy, Clone, Display)]
pub enum SongProperty {
    Duration,
    Filename,
    Artist,
    AlbumArtist,
    Title,
    Album,
    Date,
    Genre,
    Comment,
}

#[derive(Debug, Serialize, Deserialize, Copy, Clone)]
pub enum Alignment {
    Left,
    Right,
    Center,
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

impl TryFrom<UiConfigFile> for UiConfig {
    type Error = anyhow::Error;

    #[allow(clippy::similar_names)]
    fn try_from(value: UiConfigFile) -> Result<Self, Self::Error> {
        let bg_color = StringColor(value.background_color).to_color()?;
        let modal_bg_color = StringColor(value.background_color_modal).to_color()?.or(bg_color);
        let borders_color = StringColor(value.borders_color).to_color()?.unwrap_or(Color::White);

        Ok(Self {
            background_color: bg_color,
            background_color_modal: modal_bg_color,
            borders_color,
            current_song_color: StringColor(value.current_song_color).to_color()?.unwrap_or(Color::Red),
            volume_color: StringColor(value.volume_color).to_color()?.unwrap_or(Color::Blue),
            status_color: StringColor(value.status_color).to_color()?.unwrap_or(Color::Yellow),
            highlight_style: value.highlight_style.to_config_or(Color::Black, Color::Blue)?,
            highlight_border_style: value.highlight_border_style.to_config_or(Color::Red, Color::Reset)?,
            active_tab_style: value.active_tab_style.to_config_or(Color::Black, Color::Blue)?,
            inactive_tab_style: value.inactive_tab_style.to_config_or(Color::Reset, Color::Reset)?,
            disable_images: value.disable_images,
            symbols: value.symbols.into(),
            show_song_table_header: value.show_song_table_header,
            scrollbar: value.scrollbar.into_config(borders_color)?,
            progress_bar: value.progress_bar.into_config()?,
            column_widths: [
                value.browser_column_widths[0],
                value.browser_column_widths[1],
                value.browser_column_widths[2],
            ],
            song_table_format: TryInto::<QueueTableColumns>::try_into(value.song_table_format)?.0,
        })
    }
}
