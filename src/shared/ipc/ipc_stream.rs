use std::{
    io::{Read, Write},
    os::unix::net::UnixStream,
    time::Duration,
};

pub const IPC_RESPONSE_FINISH: &str = "ok";

#[derive(Debug)]
pub(crate) struct IpcStream(UnixStream);

impl From<UnixStream> for IpcStream {
    fn from(stream: UnixStream) -> Self {
        IpcStream(stream)
    }
}

impl Write for IpcStream {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.0.flush()
    }
}

impl Read for IpcStream {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.0.read(buf)
    }
}

impl Drop for IpcStream {
    fn drop(&mut self) {
        if let Err(err) = self.0.set_write_timeout(Some(Duration::from_secs(1))) {
            log::error!(err:?; "Failed to set write timeout on IPC stream");
            return;
        }

        if let Err(err) = self.0.write_all(IPC_RESPONSE_FINISH.as_bytes()) {
            log::error!(err:?; "Failed to write response finisher to IPC stream on drop");
            return;
        }

        if let Err(err) = self.0.write_all(b"\n") {
            log::error!(err:?; "Failed to write newline to IPC stream on drop");
            return;
        }
    }
}
