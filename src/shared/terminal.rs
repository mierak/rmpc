use std::{
    io::{BufWriter, Read, Stdout, Write},
    sync::{Arc, LazyLock},
};

use anyhow::Result;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use parking_lot::{Mutex, MutexGuard};
use ratatui::{
    buffer::Cell,
    layout::Position,
    prelude::{Backend, CrosstermBackend},
};

#[allow(dead_code)]
pub struct Terminal {
    reader: TtyReader,
    writer: TtyWriter,
}

pub static TERMINAL: LazyLock<Terminal> = LazyLock::new(|| Terminal {
    reader: TtyReader { stdin: Arc::new(Mutex::new(std::io::stdin())) },
    writer: TtyWriter { stdout: Arc::new(Mutex::new(BufWriter::new(std::io::stdout()))) },
});

#[allow(dead_code)]
impl Terminal {
    pub fn reader(&self) -> TtyReader {
        TtyReader { stdin: Arc::clone(&self.reader.stdin) }
    }

    pub fn writer(&self) -> TtyWriter {
        TtyWriter { stdout: Arc::clone(&self.writer.stdout) }
    }
}

pub struct TtyWriter {
    stdout: Arc<Mutex<BufWriter<std::io::Stdout>>>,
}

pub struct TtyReader {
    stdin: Arc<Mutex<std::io::Stdin>>,
}

impl TtyWriter {
    pub fn lock(&self) -> MutexGuard<'_, BufWriter<Stdout>> {
        self.stdout.lock()
    }
}

impl Write for TtyWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.stdout.lock().write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.stdout.lock().flush()
    }
}

impl Read for TtyReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.stdin.lock().read(buf)
    }
}

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

pub fn restore<B: Backend + std::io::Write>(
    terminal: &mut ratatui::Terminal<B>,
    enable_mouse: bool,
) -> Result<()> {
    if enable_mouse {
        execute!(std::io::stdout(), DisableMouseCapture)?;
    }
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    Ok(terminal.show_cursor()?)
}

pub fn setup(enable_mouse: bool) -> Result<ratatui::Terminal<CrosstermLockingBackend>> {
    enable_raw_mode()?;
    let mut writer = TERMINAL.writer();
    execute!(writer, EnterAlternateScreen)?;
    if enable_mouse {
        execute!(writer, EnableMouseCapture)?;
    }
    let mut terminal = ratatui::Terminal::new(CrosstermLockingBackend::new(writer))?;
    terminal.clear()?;
    Ok(terminal)
}
