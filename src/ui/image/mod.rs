use anyhow::Result;
use ratatui::{buffer::Buffer, layout::Rect, style::Color};

pub mod album_art_facade;
pub mod iterm2;
pub mod kitty_image;
pub mod ueberzug;

pub trait ImageProto {
    fn render(&mut self, rect: Rect) -> Result<()>;
    fn post_render(&mut self, buf: &mut Buffer, bg_color: Option<Color>, rect: Rect) -> Result<()>;
    fn hide(&mut self, bg_color: Option<Color>, size: Rect) -> Result<()>;
    fn show(&mut self);
    fn resize(&mut self);
    fn set_data(&mut self, data: Option<Vec<u8>>);
}
