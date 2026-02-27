use std::{
    io::{BufWriter, Read, Stdout, Write},
    sync::Arc,
};

use anyhow::Result;
use parking_lot::{Mutex, MutexGuard};

use crate::shared::tmux::IS_TMUX;

pub struct Tty {
    reader: TtyReader,
    writer: TtyWriter,
}

impl Tty {
    pub fn new() -> Self {
        Tty { reader: TtyReader::new(), writer: TtyWriter::new() }
    }

    pub fn reader(&self) -> TtyReader {
        self.reader.get()
    }

    pub fn writer(&self) -> TtyWriter {
        self.writer.get()
    }
}

pub struct TtyWriter {
    stdout: Arc<Mutex<BufWriter<std::io::Stdout>>>,
}

pub struct TtyReader {
    stdin: Arc<Mutex<std::io::Stdin>>,
}

impl TtyReader {
    pub fn new() -> Self {
        Self { stdin: Arc::new(Mutex::new(std::io::stdin())) }
    }

    pub(super) fn get(&self) -> Self {
        Self { stdin: Arc::clone(&self.stdin) }
    }
}

impl TtyWriter {
    pub fn new() -> Self {
        Self { stdout: Arc::new(Mutex::new(BufWriter::new(std::io::stdout()))) }
    }

    pub(super) fn get(&self) -> Self {
        Self { stdout: Arc::clone(&self.stdout) }
    }

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

impl Tty {
    pub fn query_term(query: &str, read_until: impl Fn((u8, &str)) -> bool) -> Result<String> {
        let query = if *IS_TMUX {
            format!("\x1bPtmux;{}\x1b\\", query.replace('\x1b', "\x1b\x1b"))
        } else {
            query.to_string()
        };

        let stdin = rustix::stdio::stdin();
        let termios_orig = rustix::termios::tcgetattr(stdin)?;
        let mut termios = termios_orig.clone();

        termios.local_modes &= !rustix::termios::LocalModes::ICANON;
        termios.local_modes &= !rustix::termios::LocalModes::ECHO;
        termios.special_codes[rustix::termios::SpecialCodeIndex::VTIME] = 1;
        termios.special_codes[rustix::termios::SpecialCodeIndex::VMIN] = 0;

        rustix::termios::tcsetattr(stdin, rustix::termios::OptionalActions::Drain, &termios)?;

        rustix::io::write(rustix::stdio::stdout(), query.as_bytes())?;

        let mut buf: String = String::new();
        loop {
            let mut charbuffer = [0; 1];
            rustix::io::read(stdin, &mut charbuffer)?;

            buf.push(charbuffer[0].into());

            if !read_until((charbuffer[0], &buf)) {
                break;
            }
        }

        rustix::termios::tcsetattr(stdin, rustix::termios::OptionalActions::Now, &termios_orig)?;

        Ok(buf)
    }

    pub fn query_device_attrs(query: &str) -> Result<String> {
        let result = Self::query_term(query, |(char, buf)| {
            !(char == b'c'
                && buf.contains('\x1b')
                && buf.rsplit('\x1b').next().is_some_and(|s| s.starts_with("[?"))
                || char == b'\0')
        });
        log::debug!(result:?; "devattr response");

        result
    }
}
