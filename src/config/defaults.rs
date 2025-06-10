#![allow(dead_code)]
#![allow(clippy::unnecessary_wraps)]

use super::theme::{Modifiers, ScrollbarConfigFile, StyleFile, properties::SongPropertyFile};
pub fn default_column_widths() -> Vec<u16> {
    vec![20, 38, 42]
}

pub fn bool<const V: bool>() -> bool {
    V
}

pub fn u8<const V: u8>() -> u8 {
    V
}

pub fn u16<const V: u16>() -> u16 {
    V
}

pub fn u32<const V: u32>() -> u32 {
    V
}

pub fn u64<const V: u64>() -> u64 {
    V
}

pub fn usize<const V: usize>() -> usize {
    V
}

pub fn default_bar_symbols() -> Vec<char> {
    "▁▂▃▄▅▆▇█".chars().collect()
}

pub fn default_progress_update_interval_ms() -> Option<u64> {
    Some(1000)
}

pub fn mpd_address() -> String {
    "127.0.0.1:6600".to_string()
}

pub fn mpd_host() -> String {
    "127.0.0.1".to_string()
}

pub fn mpd_port() -> String {
    "6600".to_string()
}

pub fn disabled_album_art_protos() -> Vec<String> {
    ["http://", "https://"].into_iter().map(|p| p.to_owned()).collect()
}

pub fn default_playing_label() -> String {
    "Playing".to_string()
}

pub fn default_paused_label() -> String {
    "Paused".to_string()
}

pub fn default_stopped_label() -> String {
    "Stopped".to_string()
}

pub fn default_on_label() -> String {
    "On".to_string()
}

pub fn default_off_label() -> String {
    "Off".to_string()
}

pub fn default_oneshot_label() -> String {
    "OS".to_string()
}

pub fn default_song_sort() -> Vec<SongPropertyFile> {
    vec![
        SongPropertyFile::Disc,
        SongPropertyFile::Track,
        SongPropertyFile::Artist,
        SongPropertyFile::Title,
    ]
}

pub fn default_tag_separator() -> String {
    " | ".to_string()
}

pub fn default_preview_label_style() -> StyleFile {
    StyleFile { fg: Some("yellow".to_string()), bg: None, modifiers: None }
}

pub fn default_preview_metaga_group_heading_style() -> StyleFile {
    StyleFile { fg: Some("yellow".to_string()), bg: None, modifiers: Some(Modifiers::Bold) }
}

pub fn default_thousands_separator() -> String {
    ",".to_string()
}

pub fn default_time_unit_separator() -> String {
    ", ".to_string()
}

pub fn default_scrollbar() -> Option<ScrollbarConfigFile> {
    Some(ScrollbarConfigFile::default())
}

pub fn default_trace_color() -> StyleFile {
    StyleFile { fg: Some("magenta".to_string()), bg: Some("black".to_string()), modifiers: None }
}

pub fn default_debug_color() -> StyleFile {
    StyleFile {
        fg: Some("light_green".to_string()),
        bg: Some("black".to_string()),
        modifiers: None,
    }
}

pub fn default_info_color() -> StyleFile {
    StyleFile { fg: Some("blue".to_string()), bg: Some("black".to_string()), modifiers: None }
}

pub fn default_warn_color() -> StyleFile {
    StyleFile { fg: Some("yellow".to_string()), bg: Some("black".to_string()), modifiers: None }
}

pub fn default_error_color() -> StyleFile {
    StyleFile { fg: Some("red".to_string()), bg: Some("black".to_string()), modifiers: None }
}

pub fn default_status_bar_background_color() -> StyleFile {
    StyleFile { fg: Some("black".to_string()), bg: Some("black".to_string()), modifiers: None }
}
