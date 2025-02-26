use ::serde::{Deserialize, Serialize};
use anyhow::Result;
use properties::{SongFormat, SongFormatFile};
use ratatui::style::{Color, Style};

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

pub use style::{ConfigColor, StyleFile};

pub use self::queue_table::{PercentOrLength, SongTableColumn};
use super::{
    defaults,
    tabs::{PaneOrSplitFile, SizedPaneOrSplit},
};

const DEFAULT_ART: &[u8; 58599] = include_bytes!("../../../assets/default.jpg");

#[derive(derive_more::Debug, Default, Clone)]
pub struct UiConfig {
    pub draw_borders: bool,
    pub background_color: Option<Color>,
    pub header_background_color: Option<Color>,
    pub modal_background_color: Option<Color>,
    pub modal_backdrop: bool,
    pub text_color: Option<Color>,
    pub borders_style: Style,
    pub highlighted_item_style: Style,
    pub current_item_style: Style,
    pub highlight_border_style: Style,
    pub column_widths: [u16; 3],
    pub browser_song_format: SongFormat,
    pub symbols: SymbolsConfig,
    pub progress_bar: ProgressBarConfig,
    pub tab_bar: TabBar,
    pub scrollbar: ScrollbarConfig,
    pub show_song_table_header: bool,
    pub song_table_format: Vec<SongTableColumn>,
    pub header: HeaderConfig,
    #[debug("{}", default_album_art.len())]
    pub default_album_art: &'static [u8],
    pub layout: SizedPaneOrSplit,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct UiConfigFile {
    #[serde(default = "defaults::default_true")]
    pub(super) draw_borders: bool,
    pub(super) symbols: SymbolsFile,
    pub(super) tab_bar: TabBarFile,
    pub(super) progress_bar: ProgressBarConfigFile,
    pub(super) scrollbar: ScrollbarConfigFile,
    #[serde(default = "defaults::default_column_widths")]
    pub(super) browser_column_widths: Vec<u16>,
    #[serde(default)]
    pub(super) browser_song_format: SongFormatFile,
    pub(super) background_color: Option<String>,
    pub(super) text_color: Option<String>,
    pub(super) header_background_color: Option<String>,
    pub(super) modal_background_color: Option<String>,
    #[serde(default)]
    pub(super) modal_backdrop: bool,
    pub(super) borders_style: Option<StyleFile>,
    pub(super) highlighted_item_style: Option<StyleFile>,
    pub(super) current_item_style: Option<StyleFile>,
    pub(super) highlight_border_style: Option<StyleFile>,
    pub(super) show_song_table_header: bool,
    pub(super) song_table_format: QueueTableColumnsFile,
    pub(super) header: HeaderConfigFile,
    pub(super) default_album_art_path: Option<String>,
    #[serde(default)]
    pub(super) layout: PaneOrSplitFile,
}

impl Default for UiConfigFile {
    fn default() -> Self {
        Self {
            layout: PaneOrSplitFile::default(),
            default_album_art_path: None,
            draw_borders: true,
            background_color: None,
            text_color: None,
            header_background_color: None,
            show_song_table_header: true,
            header: HeaderConfigFile::default(),
            modal_background_color: None,
            modal_backdrop: false,
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
            tab_bar: TabBarFile {
                enabled: Some(true),
                active_style: Some(StyleFile {
                    fg: Some("black".to_string()),
                    bg: Some("blue".to_string()),
                    modifiers: Some(Modifiers::Bold),
                }),
                inactive_style: Some(StyleFile { fg: None, bg: None, modifiers: None }),
            },
            browser_column_widths: vec![20, 38, 42],
            progress_bar: ProgressBarConfigFile::default(),
            scrollbar: ScrollbarConfigFile::default(),
            symbols: SymbolsFile {
                song: "S".to_owned(),
                dir: "D".to_owned(),
                marker: "M".to_owned(),
                ellipsis: Some("...".to_owned()),
            },
            song_table_format: QueueTableColumnsFile::default(),
            browser_song_format: SongFormatFile::default(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct TabBarFile {
    // deprecated
    pub(super) enabled: Option<bool>,
    pub(super) active_style: Option<StyleFile>,
    pub(super) inactive_style: Option<StyleFile>,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct TabBar {
    pub active_style: Style,
    pub inactive_style: Style,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SymbolsFile {
    pub(super) song: String,
    pub(super) dir: String,
    pub(super) marker: String,
    pub(super) ellipsis: Option<String>,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct SymbolsConfig {
    pub song: &'static str,
    pub dir: &'static str,
    pub marker: &'static str,
    pub ellipsis: &'static str,
}

impl From<SymbolsFile> for SymbolsConfig {
    fn from(value: SymbolsFile) -> Self {
        Self {
            song: value.song.leak(),
            dir: value.dir.leak(),
            marker: value.marker.leak(),
            ellipsis: value.ellipsis.unwrap_or_else(|| "...".to_string()).leak(),
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
            layout: value.layout.convert()?,
            background_color: bg_color,
            draw_borders: value.draw_borders,
            modal_background_color: StringColor(value.modal_background_color)
                .to_color()?
                .or(bg_color),
            modal_backdrop: value.modal_backdrop,
            text_color: StringColor(value.text_color).to_color()?,
            header_background_color: header_bg_color,
            borders_style: value.borders_style.to_config_or(Some(fallback_border_fg), None)?,
            highlighted_item_style: value
                .highlighted_item_style
                .to_config_or(Some(Color::Blue), None)?,
            highlight_border_style: value
                .highlight_border_style
                .to_config_or(Some(Color::Blue), None)?,
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
            tab_bar: TabBar {
                active_style: value
                    .tab_bar
                    .active_style
                    .to_config_or(Some(Color::Black), Some(Color::Blue))?,
                inactive_style: value.tab_bar.inactive_style.to_config_or(None, header_bg_color)?,
            },
            current_item_style: value
                .current_item_style
                .to_config_or(Some(Color::Black), Some(Color::Blue))?,
            default_album_art: value
                .default_album_art_path
                .map_or(Ok(DEFAULT_ART as &'static [u8]), |path| -> Result<_> {
                    Ok(std::fs::read(path)?.leak())
                })?,
            browser_song_format: TryInto::<SongFormat>::try_into(value.browser_song_format)?,
        })
    }
}
