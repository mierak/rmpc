use std::{
    io::{BufWriter, Read, Stdout, Write},
    sync::{Arc, LazyLock},
};

use parking_lot::{Mutex, MutexGuard};

pub struct Terminal {
    reader: TtyReader,
    writer: TtyWriter,
}

pub static TERMINAL: LazyLock<Terminal> = LazyLock::new(|| Terminal {
    reader: TtyReader { stdin: Arc::new(Mutex::new(std::io::stdin())) },
    writer: TtyWriter { stdout: Arc::new(Mutex::new(BufWriter::new(std::io::stdout()))) },
});

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
