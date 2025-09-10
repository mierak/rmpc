use std::{self, io::Write};

use ratatui::{
    buffer::Cell,
    layout::Position,
    prelude::{Backend, CrosstermBackend},
};

use crate::shared::terminal::tty::TtyWriter;

pub struct CrosstermLockingBackend {
    writer: TtyWriter,
}

impl CrosstermLockingBackend {
    pub fn new(writer: TtyWriter) -> Self {
        Self { writer }
    }
}

impl Write for CrosstermLockingBackend {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.writer.lock().write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.writer.lock().flush()
    }
}

impl Backend for CrosstermLockingBackend {
    fn draw<'a, I>(&mut self, content: I) -> std::io::Result<()>
    where
        I: Iterator<Item = (u16, u16, &'a Cell)>,
    {
        CrosstermBackend::new(self.writer.lock().by_ref()).draw(content)
    }

    fn hide_cursor(&mut self) -> std::io::Result<()> {
        CrosstermBackend::new(self.writer.lock().by_ref()).hide_cursor()
    }

    fn show_cursor(&mut self) -> std::io::Result<()> {
        CrosstermBackend::new(self.writer.lock().by_ref()).show_cursor()
    }

    fn get_cursor_position(&mut self) -> std::io::Result<Position> {
        CrosstermBackend::new(self.writer.lock().by_ref()).get_cursor_position()
    }

    fn set_cursor_position<P: Into<ratatui::prelude::Position>>(
        &mut self,
        position: P,
    ) -> std::io::Result<()> {
        CrosstermBackend::new(self.writer.lock().by_ref()).set_cursor_position(position)
    }

    fn clear(&mut self) -> std::io::Result<()> {
        CrosstermBackend::new(self.writer.lock().by_ref()).clear()
    }

    fn size(&self) -> std::io::Result<ratatui::prelude::Size> {
        CrosstermBackend::new(self.writer.lock().by_ref()).size()
    }

    fn window_size(&mut self) -> std::io::Result<ratatui::backend::WindowSize> {
        CrosstermBackend::new(self.writer.lock().by_ref()).window_size()
    }

    fn flush(&mut self) -> std::io::Result<()> {
        ratatui::backend::Backend::flush(&mut CrosstermBackend::new(self.writer.lock().by_ref()))
    }

    fn append_lines(&mut self, n: u16) -> std::io::Result<()> {
        CrosstermBackend::new(self.writer.lock().by_ref()).append_lines(n)
    }
}
