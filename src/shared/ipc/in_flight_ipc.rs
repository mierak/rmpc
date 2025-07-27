use std::{
    io::{BufRead, BufReader},
    os::unix::net::UnixStream,
};

use thiserror::Error;

use super::ipc_stream::{IPC_RESPONSE_ERROR, IPC_RESPONSE_SUCCESS};
use crate::shared::string_util::StringExt;

#[derive(Debug, Error)]
pub(crate) enum IpcCommandError {
    #[error("error: failed to serialize command, {0}")]
    CommandSerialization(#[from] serde_json::Error),
    #[error("error: failed to create command, {0}")]
    CommandCreate(anyhow::Error),
    #[error("error: socket error, {0}")]
    SocketError(#[from] std::io::Error),
    #[error("error: command failed, {0}")]
    CommandFailure(String),
}

pub(crate) struct InFlightIpcCommand {
    pub stream: UnixStream,
}

impl InFlightIpcCommand {
    pub(crate) fn read_response(self) -> Result<Option<String>, IpcCommandError> {
        let mut read = BufReader::new(&self.stream);
        let mut buf = String::new();

        read.read_line(&mut buf)?;
        if buf.trim().starts_with(IPC_RESPONSE_ERROR) {
            // trim "error: " from the start of the line end newline from the end
            buf.drain(..IPC_RESPONSE_ERROR.len() + ": ".len());
            buf.trim_end_in_place();
            return Err(IpcCommandError::CommandFailure(buf));
        }

        if buf.trim() == IPC_RESPONSE_SUCCESS {
            return Ok(None);
        }

        let mut line_buf = String::new();
        loop {
            line_buf.clear();
            let bytes = read.read_line(&mut line_buf)?;

            if bytes == 0 {
                return Err(IpcCommandError::SocketError(std::io::Error::new(
                    std::io::ErrorKind::UnexpectedEof,
                    format!("Unexpected end of IPC response, got '{}' so far", buf.trim()),
                )));
            }

            if line_buf.trim() == IPC_RESPONSE_SUCCESS {
                break;
            }

            buf.push_str(&line_buf);
        }

        buf.trim_end_in_place();
        Ok(Some(buf))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod test {
    use std::{io::Write, os::unix::net::UnixStream};

    use crate::shared::ipc::in_flight_ipc::InFlightIpcCommand;

    #[test]
    fn reads_ok() {
        let (stream, mut other) = UnixStream::pair().unwrap();
        let ipc_command = InFlightIpcCommand { stream };

        other.write_all(b"ok\n").unwrap();

        let response = ipc_command.read_response();
        assert_eq!(response.unwrap(), None);
    }

    #[test]
    fn reads_response() {
        let (stream, mut other) = UnixStream::pair().unwrap();
        let ipc_command = InFlightIpcCommand { stream };

        other.write_all(b"error: fluff you\n").unwrap();

        let response = ipc_command.read_response();
        assert_eq!(
            response.map_err(|err| err.to_string()),
            Err("error: command failed, fluff you".to_string())
        );
    }

    #[test]
    fn read_single_response() {
        let (stream, mut other) = UnixStream::pair().unwrap();
        let ipc_command = InFlightIpcCommand { stream };

        other.write_all(b"first line response\n").unwrap();
        other.write_all(b"ok\n").unwrap();

        let response = ipc_command.read_response();
        assert_eq!(response.unwrap(), Some("first line response".to_string()));
    }

    #[test]
    fn read_multi_response() {
        let (stream, mut other) = UnixStream::pair().unwrap();
        let ipc_command = InFlightIpcCommand { stream };

        other.write_all(b"first line response\n").unwrap();
        other.write_all(b"second line response\n").unwrap();
        other.write_all(b"third line response\n").unwrap();
        other.write_all(b"ok\n").unwrap();

        let response = ipc_command.read_response();
        assert_eq!(
            response.unwrap(),
            Some("first line response\nsecond line response\nthird line response".to_string())
        );
    }

    #[test]
    fn read_multi_response_with_no_ok_ack() {
        let (stream, mut other) = UnixStream::pair().unwrap();
        let ipc_command = InFlightIpcCommand { stream };

        other.write_all(b"first line response\n").unwrap();
        other.write_all(b"second line response\n").unwrap();
        other.write_all(b"third line response\n").unwrap();
        other.shutdown(std::net::Shutdown::Both).unwrap();

        let response = ipc_command.read_response();

        assert_eq!(
            response.map_err(|err| err.to_string()),
            Err("error: socket error, Unexpected end of IPC response, got 'first line response\nsecond line response\nthird line response' so far"
                .to_string())
        );
    }

    #[test]
    fn read_response_with_timeout() {
        let (stream, _other) = UnixStream::pair().unwrap();
        stream.set_read_timeout(Some(std::time::Duration::from_millis(10))).unwrap();
        let ipc_command = InFlightIpcCommand { stream };

        let response = ipc_command.read_response();
        assert!(response.map_err(|err| err.to_string()).is_err_and(|err| {
            err.starts_with("error: socket error, Resource temporarily unavailable")
        }));
    }
}
