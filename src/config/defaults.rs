#![allow(dead_code)]
pub fn default_column_widths() -> Vec<u16> {
    vec![20, 38, 42]
}

pub fn default_false() -> bool {
    false
}

pub fn default_true() -> bool {
    true
}

pub fn default_volume_step() -> u8 {
    5
}

pub fn default_scrolloff() -> usize {
    0
}

pub fn default_read_timeout() -> u64 {
    10_000
}

pub fn default_write_timeout() -> u64 {
    5000
}

#[allow(clippy::unnecessary_wraps)]
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
