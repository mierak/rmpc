use crossterm::{clipboard::CopyToClipboard, execute};

use crate::shared::{
    macros::{status_error, status_info},
    terminal::TERMINAL,
};

pub struct Clipboard<T> {
    content: T,
}

impl<T: AsRef<[u8]>> From<T> for Clipboard<T> {
    fn from(content: T) -> Self {
        Self { content }
    }
}

impl<T: AsRef<[u8]>> Clipboard<T> {
    pub fn write(self) -> std::io::Result<()> {
        execute!(TERMINAL.writer(), CopyToClipboard::to_clipboard_from(self.content))
    }

    pub fn write_with_status(self) {
        match self.write() {
            Ok(()) => status_info!("Copied to clipboard"),
            Err(err) => status_error!("Failed to copy to clipboard: {err}"),
        }
    }
}
