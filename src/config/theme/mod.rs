use std::collections::HashMap;

use ::serde::{Deserialize, Serialize};
use anyhow::{Result, bail};
use cava::{CavaTheme, CavaThemeFile};
use itertools::Itertools;
use level_styles::{LevelStyles, LevelStylesFile};
use properties::{SongFormat, SongFormatFile};
use ratatui::style::{Color, Style};

use self::{
    header::{HeaderConfig, HeaderConfigFile},
    lyrics::{LyricsConfig, LyricsConfigFile},
    progress_bar::{ProgressBarConfig, ProgressBarConfigFile},
    queue_table::{QueueTableColumns, QueueTableColumnsFile},
    scrollbar::ScrollbarConfig,
    style::{StringColor, ToConfigOr},
    volume_slider::{VolumeSliderConfig, VolumeSliderConfigFile},
};
use crate::mpd::commands::metadata_tag::MetadataTag;

pub mod cava;
mod header;
pub mod level_styles;
mod lyrics;
mod progress_bar;
pub mod properties;
pub mod queue_table;
mod scrollbar;
mod style;
mod volume_slider;

pub use style::{ConfigColor, Modifiers, StyleFile};

pub use self::{
    queue_table::{PercentOrLength, SongTableColumn},
    scrollbar::ScrollbarConfigFile,
};
use super::{
    defaults,
    tabs::{PaneConversionError, PaneOrSplitFile, SizedPaneOrSplit},
    utils::tilde_expand,
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
    pub preview_label_style: Style,
    pub preview_metadata_group_style: Style,
    pub borders_style: Style,
    pub highlighted_item_style: Style,
    pub current_item_style: Style,
    pub highlight_border_style: Style,
    pub column_widths: [u16; 3],
    pub browser_song_format: SongFormat,
    pub symbols: SymbolsConfig,
    pub progress_bar: ProgressBarConfig,
    pub tab_bar: TabBar,
    pub scrollbar: Option<ScrollbarConfig>,
    pub show_song_table_header: bool,
    pub song_table_format: Vec<SongTableColumn>,
    pub header: HeaderConfig,
    #[debug("{}", default_album_art.len())]
    pub default_album_art: &'static [u8],
    pub layout: SizedPaneOrSplit,
    pub components: HashMap<String, SizedPaneOrSplit>,
    pub format_tag_separator: String,
    pub multiple_tag_resolution_strategy: TagResolutionStrategy,
    pub level_styles: LevelStyles,
    pub lyrics: LyricsConfig,
    pub cava: CavaTheme,
    pub volume_slider: VolumeSliderConfig,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct UiConfigFile {
    #[serde(default = "defaults::bool::<true>")]
    pub(super) draw_borders: bool,
    pub(super) symbols: SymbolsFile,
    pub(super) tab_bar: TabBarFile,
    pub(super) progress_bar: ProgressBarConfigFile,
    #[serde(default = "defaults::default_scrollbar")]
    pub(super) scrollbar: Option<ScrollbarConfigFile>,
    #[serde(default = "defaults::default_column_widths")]
    pub(super) browser_column_widths: Vec<u16>,
    #[serde(default)]
    pub(super) browser_song_format: SongFormatFile,
    pub(super) background_color: Option<String>,
    pub(super) text_color: Option<String>,
    #[serde(default = "defaults::default_preview_label_style")]
    pub(super) preview_label_style: StyleFile,
    #[serde(default = "defaults::default_preview_metaga_group_heading_style")]
    pub(super) preview_metadata_group_style: StyleFile,
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
    #[serde(default)]
    pub(super) components: HashMap<String, PaneOrSplitFile>,
    #[serde(default = "defaults::default_tag_separator")]
    pub(super) format_tag_separator: String,
    #[serde(default)]
    pub(super) multiple_tag_resolution_strategy: TagResolutionStrategy,
    #[serde(default)]
    pub(super) level_styles: LevelStylesFile,
    #[serde(default)]
    pub(super) lyrics: LyricsConfigFile,
    #[serde(default)]
    pub(super) cava: CavaThemeFile,
    #[serde(default)]
    pub(super) volume_slider: VolumeSliderConfigFile,
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
            scrollbar: Some(ScrollbarConfigFile::default()),
            symbols: SymbolsFile {
                song: "S".to_owned(),
                dir: "D".to_owned(),
                playlist: defaults::playlist_symbol(),
                marker: "M".to_owned(),
                ellipsis: Some("...".to_owned()),
                song_style: None,
                dir_style: None,
                playlist_style: None,
            },
            song_table_format: QueueTableColumnsFile::default(),
            browser_song_format: SongFormatFile::default(),
            format_tag_separator: " | ".to_owned(),
            multiple_tag_resolution_strategy: TagResolutionStrategy::default(),
            preview_label_style: StyleFile {
                fg: Some("yellow".to_string()),
                bg: None,
                modifiers: None,
            },
            preview_metadata_group_style: StyleFile {
                fg: Some("yellow".to_string()),
                bg: None,
                modifiers: Some(Modifiers::Bold),
            },
            level_styles: LevelStylesFile::default(),
            components: HashMap::default(),
            lyrics: LyricsConfigFile::default(),
            cava: CavaThemeFile::default(),
            volume_slider: VolumeSliderConfigFile::default(),
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
    #[serde(default = "defaults::playlist_symbol")]
    pub(super) playlist: String,
    pub(super) marker: String,
    pub(super) ellipsis: Option<String>,
    pub(super) song_style: Option<StyleFile>,
    pub(super) dir_style: Option<StyleFile>,
    pub(super) playlist_style: Option<StyleFile>,
}

#[derive(Debug, Default, Clone)]
pub struct SymbolsConfig {
    pub song: String,
    pub dir: String,
    pub playlist: String,
    pub marker: String,
    pub ellipsis: String,
    pub song_style: Option<Style>,
    pub dir_style: Option<Style>,
    pub playlist_style: Option<Style>,
}

impl From<SymbolsFile> for SymbolsConfig {
    fn from(value: SymbolsFile) -> Self {
        Self {
            song: value.song,
            dir: value.dir,
            playlist: value.playlist,
            marker: value.marker,
            ellipsis: value.ellipsis.unwrap_or_else(|| "...".to_string()),
            song_style: value
                .song_style
                .map(|s| s.to_config_or(None, None))
                .transpose()
                .unwrap_or_default(),
            dir_style: value
                .dir_style
                .map(|s| s.to_config_or(None, None))
                .transpose()
                .unwrap_or_default(),
            playlist_style: value
                .playlist_style
                .map(|s| s.to_config_or(None, None))
                .transpose()
                .unwrap_or_default(),
        }
    }
}
#[derive(Default, Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum TagResolutionStrategy {
    First,
    Last,
    #[default]
    All,
    Nth(usize),
}

impl TagResolutionStrategy {
    pub fn resolve<'a>(self, tag: &'a MetadataTag, separator: &str) -> std::borrow::Cow<'a, str> {
        match self {
            TagResolutionStrategy::First => tag.first().into(),
            TagResolutionStrategy::Last => tag.last().into(),
            TagResolutionStrategy::All => tag.join(separator),
            TagResolutionStrategy::Nth(idx) => tag.nth(idx).into(),
        }
    }
}

// Converts all components while also resolving dependencies between them. If a
// component is missing but is present in the source map it will be skipped and
// the resolution will be retried in the next loop over. Only when component is
// truly missing or a different kind of error occurs will the conversion fail.
fn convert_components(
    value: HashMap<String, PaneOrSplitFile>,
) -> Result<HashMap<String, SizedPaneOrSplit>> {
    let mut result = HashMap::new();
    let mut components = value.into_iter().collect_vec();

    let mut i = 0usize;
    let mut last_size = components.len();
    let mut same_size_count = 0;
    loop {
        let current = components.get(i.checked_rem(components.len()).unwrap_or(0));
        match current {
            Some((name, pane)) => match pane.convert(&result) {
                Ok(v) => {
                    result.insert(name.to_owned(), v);
                    components.remove(i % components.len());
                }
                Err(PaneConversionError::MissingComponent(missing_name))
                    if components.iter().any(|(n, _)| n == &missing_name) =>
                {
                    i += 1;
                }
                err @ Err(_) => {
                    err?;
                }
            },
            None => break,
        }

        let remaining = components.len();
        if last_size == remaining {
            same_size_count += 1;
        } else {
            same_size_count = 0;
        }

        if same_size_count > remaining {
            bail!(
                "Failed to resolve components. Circular dependency detected. Components: {:?}",
                components.iter().map(|(name, _)| name).collect::<Vec<_>>()
            );
        }

        last_size = remaining;
    }

    log::debug!(result:?; "Converted components");

    Ok(result)
}

impl TryFrom<UiConfigFile> for UiConfig {
    type Error = anyhow::Error;

    #[allow(clippy::similar_names)]
    fn try_from(value: UiConfigFile) -> Result<Self, Self::Error> {
        let bg_color = StringColor(value.background_color).to_color()?;
        let header_bg_color = StringColor(value.header_background_color).to_color()?.or(bg_color);
        let fallback_border_fg = Color::White;
        let components = convert_components(value.components)?;

        Ok(Self {
            layout: value.layout.convert(&components)?,
            components,
            cava: value.cava.into_config(bg_color)?,
            background_color: bg_color,
            draw_borders: value.draw_borders,
            format_tag_separator: value.format_tag_separator,
            multiple_tag_resolution_strategy: value.multiple_tag_resolution_strategy,
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
            scrollbar: value.scrollbar.map(|sc| sc.into_config(fallback_border_fg)).transpose()?,
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
            default_album_art: value.default_album_art_path.map_or(
                Ok(DEFAULT_ART as &'static [u8]),
                |path| -> Result<_> {
                    let path = tilde_expand(&path);
                    Ok(std::fs::read(path.as_ref())?.leak())
                },
            )?,
            browser_song_format: TryInto::<SongFormat>::try_into(value.browser_song_format)?,
            preview_label_style: value.preview_label_style.to_config_or(None, None)?,
            preview_metadata_group_style: value
                .preview_metadata_group_style
                .to_config_or(None, None)?,
            level_styles: value.level_styles.try_into()?,
            lyrics: value.lyrics.into(),
            volume_slider: value.volume_slider.into_config()?,
        })
    }
}
