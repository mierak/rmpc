pub fn default_column_widths() -> Vec<u16> {
    vec![20, 38, 42]
}

pub fn default_false() -> bool {
    false
}

pub fn default_volume_step() -> u8 {
    5
}

#[allow(clippy::unnecessary_wraps)]
pub fn default_progress_update_interval_ms() -> Option<u64> {
    Some(1000)
}
