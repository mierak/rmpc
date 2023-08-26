use ratatui::prelude::Alignment;

pub mod button;
pub mod frame_counter;
pub mod kitty_image;
pub mod line;
pub mod progress_bar;
pub mod tabs;
pub mod volume;

pub(self) fn get_line_offset(line_width: u16, text_area_width: u16, alignment: Alignment) -> u16 {
    match alignment {
        Alignment::Center => (text_area_width / 2).saturating_sub(line_width / 2),
        Alignment::Right => text_area_width.saturating_sub(line_width),
        Alignment::Left => 0,
    }
}
