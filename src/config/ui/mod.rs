use anyhow::Result;
use ratatui::style::{Color, Style};
use serde::{Deserialize, Serialize};

use self::{
    header::{HeaderConfig, HeaderConfigFile},
    progress_bar::{ProgressBarConfig, ProgressBarConfigFile},
    queue_table::{QueueTableColumns, QueueTableColumnsFile},
    scrollbar::{ScrollbarConfig, ScrollbarConfigFile},
    style::{Modifiers, StringColor, ToConfigOr},
};

mod header;
mod progress_bar;
pub mod properties;
mod queue_table;
mod scrollbar;
mod style;

pub use self::queue_table::SongTableColumn;
pub use style::{ConfigColor, StyleFile};

use super::defaults;

const DEFAULT_ART: &[u8; 58599] = include_bytes!("../../../assets/default.jpg");

#[derive(Debug)]
pub struct UiConfig {
    pub album_art_position: Position,
    pub album_art_width_percent: u16,
    pub draw_borders: bool,
    pub background_color: Option<Color>,
    pub header_background_color: Option<Color>,
    pub background_color_modal: Option<Color>,
    pub borders_style: Style,
    pub highlighted_item_style: Style,
    pub current_item_style: Style,
    pub highlight_border_style: Style,
    pub active_tab_style: Style,
    pub inactive_tab_style: Style,
    pub column_widths: [u16; 3],
    pub symbols: SymbolsConfig,
    pub progress_bar: ProgressBarConfig,
    pub scrollbar: ScrollbarConfig,
    pub show_song_table_header: bool,
    pub song_table_format: Vec<SongTableColumn>,
    pub header: HeaderConfig,
    pub default_album_art: Vec<u8>,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Position {
    Left,
    Right,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UiConfigFile {
    pub(super) album_art_position: Position,
    pub(super) album_art_width_percent: u16,
    #[serde(default = "defaults::default_true")]
    pub(super) draw_borders: bool,
    pub(super) symbols: SymbolsFile,
    pub(super) progress_bar: ProgressBarConfigFile,
    pub(super) scrollbar: ScrollbarConfigFile,
    #[serde(default = "defaults::default_column_widths")]
    pub(super) browser_column_widths: Vec<u16>,
    pub(super) background_color: Option<String>,
    pub(super) header_background_color: Option<String>,
    pub(super) background_color_modal: Option<String>,
    pub(super) active_tab_style: Option<StyleFile>,
    pub(super) inactive_tab_style: Option<StyleFile>,
    pub(super) borders_style: Option<StyleFile>,
    pub(super) highlighted_item_style: Option<StyleFile>,
    pub(super) current_item_style: Option<StyleFile>,
    pub(super) highlight_border_style: Option<StyleFile>,
    pub(super) show_song_table_header: bool,
    pub(super) song_table_format: QueueTableColumnsFile,
    pub(super) header: HeaderConfigFile,
    pub(super) default_album_art_path: Option<String>,
}

impl Default for UiConfigFile {
    fn default() -> Self {
        Self {
            album_art_position: Position::Left,
            album_art_width_percent: 40,
            default_album_art_path: None,
            draw_borders: true,
            background_color: None,
            header_background_color: None,
            show_song_table_header: true,
            header: HeaderConfigFile::default(),
            background_color_modal: None,
            borders_style: Some(StyleFile {
                fg: Some("blue".to_string()),
                bg: None,
                modifiers: None,
            }),
            highlighted_item_style: Some(StyleFile {
                fg: Some("blue".to_string()),
                bg: None,
                modifiers: Some(Modifiers::Bold),
            }),
            current_item_style: Some(StyleFile {
                fg: Some("black".to_string()),
                bg: Some("blue".to_string()),
                modifiers: Some(Modifiers::Bold),
            }),
            highlight_border_style: Some(StyleFile {
                fg: Some("blue".to_string()),
                bg: None,
                modifiers: None,
            }),
            active_tab_style: Some(StyleFile {
                fg: Some("black".to_string()),
                bg: Some("blue".to_string()),
                modifiers: Some(Modifiers::Bold),
            }),
            inactive_tab_style: Some(StyleFile {
                fg: None,
                bg: None,
                modifiers: None,
            }),
            browser_column_widths: vec![20, 38, 42],
            progress_bar: ProgressBarConfigFile::default(),
            scrollbar: ScrollbarConfigFile::default(),
            symbols: SymbolsFile {
                song: "🎵".to_owned(),
                dir: "📁".to_owned(),
                marker: "".to_owned(),
            },
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

impl TryFrom<UiConfigFile> for UiConfig {
    type Error = anyhow::Error;

    #[allow(clippy::similar_names)]
    fn try_from(value: UiConfigFile) -> Result<Self, Self::Error> {
        let bg_color = StringColor(value.background_color).to_color()?;
        let header_bg_color = StringColor(value.header_background_color).to_color()?.or(bg_color);
        let fallback_border_fg = Color::White;

        Ok(Self {
            background_color: bg_color,
            album_art_position: value.album_art_position,
            album_art_width_percent: value.album_art_width_percent,
            draw_borders: value.draw_borders,
            background_color_modal: StringColor(value.background_color_modal).to_color()?.or(bg_color),
            header_background_color: header_bg_color,
            borders_style: value.borders_style.to_config_or(Some(fallback_border_fg), None)?,
            highlighted_item_style: value.highlighted_item_style.to_config_or(Some(Color::Blue), None)?,
            highlight_border_style: value.highlight_border_style.to_config_or(Some(Color::Blue), None)?,
            inactive_tab_style: value.inactive_tab_style.to_config_or(None, header_bg_color)?,
            symbols: value.symbols.into(),
            show_song_table_header: value.show_song_table_header,
            scrollbar: value.scrollbar.into_config(fallback_border_fg)?,
            progress_bar: value.progress_bar.into_config()?,
            song_table_format: TryInto::<QueueTableColumns>::try_into(value.song_table_format)?.0,
            header: value.header.try_into()?,
            column_widths: [
                value.browser_column_widths[0],
                value.browser_column_widths[1],
                value.browser_column_widths[2],
            ],
            active_tab_style: value
                .active_tab_style
                .to_config_or(Some(Color::Black), Some(Color::Blue))?,
            current_item_style: value
                .current_item_style
                .to_config_or(Some(Color::Black), Some(Color::Blue))?,
            default_album_art: value
                .default_album_art_path
                .map_or_else(|| Ok(DEFAULT_ART.to_vec()), |path| std::fs::read(path))?,
        })
    }
}
