use std::{
    io::{Read, Write},
    os::unix::net::UnixStream,
    time::Duration,
};

pub const IPC_RESPONSE_SUCCESS: &str = "ok";
pub const IPC_RESPONSE_ERROR: &str = "error";

/// Wrapper around a [`UnixStream`] that handles IPC communication on the
/// "server" side. Automatically writes a well formed IPC response when dropped.
#[derive(Debug)]
pub(crate) struct IpcStream {
    inner: UnixStream,
    response: Vec<String>,
    error: Option<String>,
}

impl IpcStream {
    /// Consumes the stream as an error, meaning a [`IPC_RESPONSE_ERROR`]
    /// followed by an error messarge will be sent. If no error is reported, a
    /// [`Self::response`] followed by [`IPC_RESPONSE_SUCCESS`] will be sent
    /// instead.
    pub fn error(mut self, error: String) {
        self.error = Some(error);
    }

    pub fn append_response_line(&mut self, response: String) {
        self.response.push(response);
    }
}

impl From<UnixStream> for IpcStream {
    fn from(stream: UnixStream) -> Self {
        IpcStream { inner: stream, response: Vec::new(), error: None }
    }
}

impl Write for IpcStream {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.inner.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

impl Read for IpcStream {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.inner.read(buf)
    }
}

impl Drop for IpcStream {
    fn drop(&mut self) {
        if let Err(err) = self.inner.set_write_timeout(Some(Duration::from_secs(1))) {
            log::error!(err:?; "Failed to set write timeout on IPC stream");
            return;
        }

        if let Some(err) = &self.error {
            if let Err(err) = self.inner.write_all(b"error: ") {
                log::error!(err:?; "Failed to error response start to IPC stream on drop");
                return;
            }
            if let Err(err) = self.inner.write_all(err.as_bytes()) {
                log::error!(err:?; "Failed to error response to IPC stream on drop");
                return;
            }
        } else {
            for response in &self.response {
                if let Err(err) = self.inner.write_all(response.as_bytes()) {
                    log::error!(err:?; "Failed to write response to IPC stream on drop");
                    return;
                }
                if let Err(err) = self.inner.write_all(b"\n") {
                    log::error!(err:?; "Failed to write newline to IPC stream on drop");
                    return;
                }
            }
            if let Err(err) = self.inner.write_all(IPC_RESPONSE_SUCCESS.as_bytes()) {
                log::error!(err:?; "Failed to write response finisher to IPC stream on drop");
                return;
            }
        }

        if let Err(err) = self.inner.write_all(b"\n") {
            log::error!(err:?; "Failed to write newline to IPC stream on drop");
            return;
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod test {
    use std::{
        io::{BufRead, BufReader, Read},
        os::unix::net::UnixStream,
    };

    use crate::shared::ipc::ipc_stream::{IPC_RESPONSE_SUCCESS, IpcStream};

    #[test]
    fn prints_ok_on_drop() {
        let mut stream = UnixStream::pair().expect("Failed to create UnixStream pair");
        let ipc_stream = IpcStream::from(stream.0);

        drop(ipc_stream);
        let mut buf = String::new();
        stream.1.read_to_string(&mut buf).unwrap();

        assert_eq!(buf.trim(), IPC_RESPONSE_SUCCESS);
    }

    #[test]
    fn prints_success_responses_on_drop() {
        let stream = UnixStream::pair().expect("Failed to create UnixStream pair");
        let mut ipc_stream = IpcStream::from(stream.0);

        ipc_stream.append_response_line("Hello".to_string());
        ipc_stream.append_response_line("World".to_string());

        drop(ipc_stream);

        let mut buf = String::new();
        let mut reader = BufReader::new(stream.1);

        reader.read_line(&mut buf).unwrap();
        assert_eq!(buf.trim(), "Hello");

        buf.clear();
        reader.read_line(&mut buf).unwrap();
        assert_eq!(buf.trim(), "World");

        buf.clear();
        reader.read_line(&mut buf).unwrap();
        assert_eq!(buf.trim(), IPC_RESPONSE_SUCCESS);
    }

    #[test]
    fn prints_error_on_drop() {
        let stream = UnixStream::pair().expect("Failed to create UnixStream pair");
        let mut ipc_stream = IpcStream::from(stream.0);

        ipc_stream.append_response_line("Hello".to_string());
        ipc_stream.append_response_line("World".to_string());
        ipc_stream.error("An error occurred".to_string());

        let mut buf = String::new();
        let mut reader = BufReader::new(stream.1);

        reader.read_line(&mut buf).unwrap();
        assert_eq!(buf.trim(), "error: An error occurred");
    }

    #[test]
    fn prints_error_on_drop_when_no_success_messages() {
        let stream = UnixStream::pair().expect("Failed to create UnixStream pair");
        let ipc_stream = IpcStream::from(stream.0);

        ipc_stream.error("An error occurred".to_string());

        let mut buf = String::new();
        let mut reader = BufReader::new(stream.1);

        reader.read_line(&mut buf).unwrap();
        assert_eq!(buf.trim(), "error: An error occurred");
    }
}
