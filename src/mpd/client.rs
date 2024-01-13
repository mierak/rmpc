use std::{
    io::{BufRead, BufReader, Read, Write},
    net::TcpStream,
    str::FromStr,
};

use anyhow::Result;
use tracing::{debug, trace};

use super::{
    errors::{MpdError, MpdFailureResponse},
    split_line,
    version::Version,
    FromMpd, FromMpdBuilder,
};

type MpdResult<T> = Result<T, MpdError>;

pub struct Client<'a> {
    name: Option<&'a str>,
    rx: BufReader<TcpStream>,
    stream: TcpStream,
    reconnect: bool,
    addr: &'static str,
    pub version: Version,
}

impl std::fmt::Debug for Client<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Client {{ name: {:?}, recconect: {}, addr: {} }}",
            self.name, self.reconnect, self.addr
        )
    }
}

#[allow(dead_code)]
impl<'a> Client<'a> {
    #[tracing::instrument]
    pub fn init(addr: &'static str, name: Option<&'a str>, reconnect: bool) -> MpdResult<Client<'a>> {
        let stream = TcpStream::connect(addr)?;
        stream.set_write_timeout(Some(std::time::Duration::from_secs(1)))?;
        stream.set_read_timeout(Some(std::time::Duration::from_secs(10)))?;
        let mut rx = BufReader::new(stream.try_clone()?);

        let mut buf = String::new();
        rx.read_line(&mut buf)?;
        if !buf.starts_with("OK") {
            return Err(MpdError::Generic(format!("Handshake validation failed. '{buf}'")));
        };
        let Some(version): Option<Version> = buf.strip_prefix("OK MPD ").and_then(|v| v.parse().ok()) else {
            return Err(MpdError::Generic(format!(
                "Handshake validation failed. Cannot parse version from '{buf}'"
            )));
        };

        debug!(
            message = "MPD client initiazed",
            handshake = buf.trim(),
            version = version.to_string()
        );

        Ok(Self {
            name,
            rx,
            stream,
            reconnect,
            addr,
            version,
        })
    }

    #[tracing::instrument]
    fn reconnect(&mut self) -> MpdResult<&Client> {
        let stream = TcpStream::connect(self.addr)?;
        stream.set_write_timeout(Some(std::time::Duration::from_secs(1)))?;
        stream.set_read_timeout(Some(std::time::Duration::from_secs(10)))?;
        let mut rx = BufReader::new(stream.try_clone()?);

        let mut buf = String::new();
        rx.read_line(&mut buf)?;
        if !buf.starts_with("OK") {
            return Err(MpdError::Generic(format!("Handshake validation failed. '{buf}'")));
        };

        let Some(version): Option<Version> = buf.strip_prefix("OK MPD ").and_then(|v| v.parse().ok()) else {
            return Err(MpdError::Generic(format!(
                "Handshake validation failed. Cannot parse version from '{buf}'"
            )));
        };

        self.rx = rx;
        self.stream = stream;
        self.version = version;

        debug!(
            message = "MPD client initiazed",
            handshake = buf.trim(),
            version = version.to_string()
        );

        Ok(self)
    }

    #[tracing::instrument(skip(self))]
    pub fn set_read_timeout(&mut self, timeout: Option<std::time::Duration>) -> std::io::Result<()> {
        self.stream.set_read_timeout(timeout)
    }

    #[tracing::instrument(skip(self))]
    pub fn set_write_timeout(&mut self, timeout: Option<std::time::Duration>) -> std::io::Result<()> {
        self.stream.set_write_timeout(timeout)
    }

    #[tracing::instrument(skip(self))]
    pub(super) fn execute_binary(&mut self, command: &str) -> MpdResult<Option<Vec<u8>>> {
        let mut buf = Vec::new();

        self.write_command(&format!("{command} {}", buf.len()))?;
        let _ = match Self::read_binary(&mut self.rx, &mut buf) {
            Ok(Some(v)) => Ok(Some(v)),
            Ok(None) => return Ok(None),
            Err(MpdError::ClientClosed) if self.reconnect => {
                self.reconnect()?;
                self.write_command(&format!("{command} {}", buf.len()))?;
                Self::read_binary(&mut self.rx, &mut buf)
            }
            Err(e) => Err(e),
        };
        loop {
            self.write_command(&format!("{command} {}", buf.len()))?;
            if let Some(response) = Self::read_binary(&mut self.rx, &mut buf)? {
                if buf.len() >= response.size_total as usize || response.bytes_read == 0 {
                    trace!(message = "Finshed reading binary response", len = buf.len());
                    break;
                }
            } else {
                return Err(MpdError::ValueExpected("Expected binary data but got none".to_owned()));
            }
        }
        Ok(Some(buf))
    }

    #[tracing::instrument(skip(self))]
    pub(super) fn execute<T>(&mut self, command: &str) -> MpdResult<T>
    where
        T: FromMpd + FromMpdBuilder<T>,
    {
        self.write_command(command)?;
        match Self::read::<BufReader<TcpStream>, T, T>(&mut self.rx) {
            Ok(v) => Ok(v),
            Err(MpdError::ClientClosed) => {
                self.reconnect()?;
                self.write_command(command)?;
                Self::read::<BufReader<TcpStream>, T, T>(&mut self.rx)
            }
            Err(e) => Err(e),
        }
    }

    #[tracing::instrument(skip(self), fields(command = ?command))]
    pub(super) fn execute_option<T>(&mut self, command: &str) -> MpdResult<Option<T>>
    where
        T: FromMpd + FromMpdBuilder<T>,
    {
        self.write_command(command)?;
        match Self::read_option::<BufReader<TcpStream>, T, T>(&mut self.rx) {
            Ok(v) => Ok(v),
            Err(MpdError::ClientClosed) => {
                self.reconnect()?;
                self.write_command(command)?;
                Self::read_option::<BufReader<TcpStream>, T, T>(&mut self.rx)
            }
            Err(e) => Err(e),
        }
    }

    #[tracing::instrument(skip(self), fields(command = ?command))]
    pub(super) fn execute_ok(&mut self, command: &str) -> MpdResult<()> {
        self.write_command(command)?;
        match Self::read_ok(&mut self.rx) {
            Ok(v) => Ok(v),
            Err(MpdError::ClientClosed) => {
                self.reconnect()?;
                self.write_command(command)?;
                Self::read_ok(&mut self.rx)
            }
            Err(e) => Err(e),
        }
    }

    #[tracing::instrument(skip(read, binary_buf), fields(buf_len = binary_buf.len()))]
    fn read_binary<R: std::fmt::Debug>(
        read: &mut R,
        binary_buf: &mut Vec<u8>,
    ) -> Result<Option<BinaryMpdResponse>, MpdError>
    where
        R: std::io::BufRead,
    {
        let mut result = BinaryMpdResponse::default();
        {
            loop {
                match Self::read_mpd_line(read)? {
                    MpdLine::Ok => {
                        tracing::warn!("Expected binary data but got 'OK'");
                        return Ok(None);
                    }
                    MpdLine::Value(val) => {
                        let (key, value) = split_line(val)?;
                        match key.to_lowercase().as_ref() {
                            "size" => result.size_total = value.parse()?,
                            "type" => result.mime_type = Some(value),
                            "binary" => {
                                result.bytes_read = value.parse()?;
                                break;
                            }
                            key => {
                                return Err(MpdError::Generic(format!(
                                    "Unexpected key when parsing binary response: '{key}'"
                                )))
                            }
                        }
                    }
                };
            }
        }
        let mut handle = read.take(result.bytes_read);
        let _ = handle.read_to_end(binary_buf)?;
        let _ = read.read_line(&mut String::new()); // MPD prints an empty new line at the end of binary response
        match Self::read_mpd_line(read)? {
            MpdLine::Ok => Ok(Some(result)),
            MpdLine::Value(val) => Err(MpdError::Generic(format!("Expected 'OK' but got '{val}'"))),
        }
    }

    #[tracing::instrument(skip(read))]
    fn read<R, A, V>(read: &mut R) -> Result<V, MpdError>
    where
        R: std::io::BufRead,
        V: FromMpd,
        A: FromMpdBuilder<V>,
    {
        trace!(message = "Reading command");
        let mut result = A::create();
        loop {
            match Self::read_mpd_line(read)? {
                MpdLine::Ok => break,
                MpdLine::Value(val) => result.next(val)?,
            };
        }

        result.finish()
    }

    #[tracing::instrument(skip(read))]
    fn read_ok<R>(read: &mut R) -> Result<(), MpdError>
    where
        R: std::io::BufRead,
    {
        trace!(message = "Reading command");
        match Self::read_mpd_line(read)? {
            MpdLine::Ok => Ok(()),
            MpdLine::Value(val) => Err(MpdError::Generic(format!("Expected 'OK' but got '{val}'"))),
        }
    }

    #[tracing::instrument(skip(read))]
    fn read_option<R, A, V>(read: &mut R) -> Result<Option<V>, MpdError>
    where
        R: std::io::BufRead,
        V: FromMpd,
        A: FromMpdBuilder<V>,
    {
        trace!(message = "Reading command");
        let mut result = A::create();
        let mut found_any = false;
        loop {
            match Self::read_mpd_line(read)? {
                MpdLine::Ok => break,
                MpdLine::Value(val) => {
                    found_any = true;
                    result.next(val)?;
                }
            }
        }

        if found_any {
            Ok(Some(result.finish()?))
        } else {
            Ok(None)
        }
    }

    fn write_command(&mut self, command: &str) -> Result<(), MpdError> {
        if let Err(e) = self.stream.write_all([command, "\n"].concat().as_bytes()) {
            if e.kind() == std::io::ErrorKind::BrokenPipe {
                self.reconnect()?;
                self.stream.write_all([command, "\n"].concat().as_bytes())?;
            }
        }
        Ok(())
    }

    fn read_mpd_line<R: std::io::BufRead>(read: &mut R) -> Result<MpdLine, MpdError> {
        let mut line = String::new();

        let bytes_read = match read.read_line(&mut line) {
            Ok(v) => Ok(v),
            Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => Err(MpdError::ClientClosed),
            _ => Err(MpdError::ClientClosed),
        }?;

        if bytes_read == 0 {
            return Err(MpdError::ClientClosed);
        }

        if line.starts_with("OK") || line.starts_with("list_OK") {
            return Ok(MpdLine::Ok);
        }
        if line.starts_with("ACK") {
            return Err(MpdError::Mpd(MpdFailureResponse::from_str(&line)?));
        }
        line.pop(); // pop the new line
        Ok(MpdLine::Value(line))
    }
}

#[derive(Debug, Default, PartialEq)]
struct BinaryMpdResponse {
    pub bytes_read: u64,
    pub size_total: u32,
    pub mime_type: Option<String>,
}
#[derive(Debug, PartialEq, Eq)]
enum MpdLine {
    Ok,
    Value(String),
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use crate::mpd::{errors::MpdError, FromMpd, LineHandled};

    #[derive(Default, Debug, PartialEq, Eq)]
    struct TestMpdObject {
        val_a: String,
        val_b: String,
    }
    impl FromMpd for TestMpdObject {
        fn finish(self) -> Result<Self, crate::mpd::errors::MpdError> {
            Ok(self)
        }

        fn next_internal(&mut self, key: &str, value: String) -> Result<LineHandled, MpdError> {
            if key == "fail" {
                return Err(MpdError::Generic(String::from("intentional fail")));
            }
            match key {
                "val_a" => self.val_a = value,
                "val_b" => self.val_b = value,
                _ => return Err(MpdError::Generic(String::from("unknown value"))),
            }
            Ok(LineHandled::Yes)
        }
    }

    mod read_mpd_line {
        use std::io::Cursor;

        use crate::mpd::{
            client::{Client, MpdLine},
            errors::{ErrorCode, MpdError, MpdFailureResponse},
        };

        #[test]
        fn returns_ok() {
            let result = Client::read_mpd_line(&mut Cursor::new("OK enenene".to_owned()));

            assert_eq!(Ok(MpdLine::Ok), result);
        }

        #[test]
        fn returns_ok_for_list_ok() {
            let result = Client::read_mpd_line(&mut Cursor::new("list_OK enenene".to_owned()));

            assert_eq!(Ok(MpdLine::Ok), result);
        }

        #[test]
        fn returns_mpd_err() {
            let err = MpdFailureResponse {
                code: ErrorCode::PlayerSync,
                command_list_index: 2,
                command: "some_cmd".to_string(),
                message: "error message boi".to_string(),
            };

            let result = Client::read_mpd_line(&mut Cursor::new("ACK [55@2] {some_cmd} error message boi".to_owned()));

            assert_eq!(Err(MpdError::Mpd(err)), result);
        }

        #[test]
        fn returns_client_closed_on_broken_pipe() {
            struct Mock;
            impl std::io::BufRead for Mock {
                fn consume(&mut self, _amt: usize) {}
                fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
                    Err(std::io::Error::from(std::io::ErrorKind::BrokenPipe))
                }
            }
            impl std::io::Read for Mock {
                fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
                    Err(std::io::Error::from(std::io::ErrorKind::BrokenPipe))
                }
            }

            let result = Client::read_mpd_line(&mut Mock);

            assert_eq!(Err(MpdError::ClientClosed), result);
        }
    }

    mod response {
        use crate::mpd::{
            client::Client,
            errors::{ErrorCode, MpdError, MpdFailureResponse},
        };

        use super::*;

        #[test]
        fn parses_correct_response() {
            let buf: &[u8] = b"val_b: a\nval_a: 5\nOK\n";
            let mut c = Cursor::new(buf);

            let result = Client::read::<Cursor<&[u8]>, TestMpdObject, TestMpdObject>(&mut c);

            assert_eq!(
                result,
                Ok(TestMpdObject {
                    val_a: "5".to_owned(),
                    val_b: "a".to_owned()
                })
            );
        }

        #[test]
        fn returns_parse_error() {
            let buf: &[u8] = b"fail: lol\nOK\n";
            let mut c = Cursor::new(buf);

            let result = Client::read::<Cursor<&[u8]>, TestMpdObject, TestMpdObject>(&mut c);

            assert_eq!(result, Err(MpdError::Generic(String::from("intentional fail"))));
        }

        #[test]
        fn returns_mpd_error() {
            let buf: &[u8] = b"ACK [55@2] {some_cmd} error message boi\n";
            let err = MpdFailureResponse {
                code: ErrorCode::PlayerSync,
                command_list_index: 2,
                command: "some_cmd".to_string(),
                message: "error message boi".to_string(),
            };
            let mut c = Cursor::new(buf);

            let result = Client::read::<Cursor<&[u8]>, TestMpdObject, TestMpdObject>(&mut c);

            assert_eq!(result, Err(MpdError::Mpd(err)));
        }
    }
    mod response_opt {
        use crate::mpd::{
            client::Client,
            errors::{ErrorCode, MpdError, MpdFailureResponse},
        };

        use super::*;

        #[test]
        fn parses_correct_response() {
            let buf: &[u8] = b"val_b: a\nval_a: 5\nOK\n";
            let mut c = Cursor::new(buf);

            let result = Client::read_option::<Cursor<&[u8]>, TestMpdObject, TestMpdObject>(&mut c);

            assert_eq!(
                result,
                Ok(Some(TestMpdObject {
                    val_a: "5".to_owned(),
                    val_b: "a".to_owned()
                }))
            );
        }

        #[test]
        fn returns_none() {
            let buf: &[u8] = b"OK\n";
            let mut c = Cursor::new(buf);

            let result = Client::read_option::<Cursor<&[u8]>, TestMpdObject, TestMpdObject>(&mut c);

            assert_eq!(result, Ok(None));
        }

        #[test]
        fn returns_parse_error() {
            let buf: &[u8] = b"fail: lol\nOK\n";
            let mut c = Cursor::new(buf);

            let result = Client::read_option::<Cursor<&[u8]>, TestMpdObject, TestMpdObject>(&mut c);

            assert_eq!(result, Err(MpdError::Generic(String::from("intentional fail"))));
        }

        #[test]
        fn returns_mpd_error() {
            let buf: &[u8] = b"ACK [55@2] {some_cmd} error message boi\n";
            let err = MpdFailureResponse {
                code: ErrorCode::PlayerSync,
                command_list_index: 2,
                command: "some_cmd".to_string(),
                message: "error message boi".to_string(),
            };
            let mut c = Cursor::new(buf);

            let result = Client::read_option::<Cursor<&[u8]>, TestMpdObject, TestMpdObject>(&mut c);

            assert_eq!(result, Err(MpdError::Mpd(err)));
        }
    }

    mod ok {
        use crate::mpd::{
            client::Client,
            errors::{ErrorCode, MpdFailureResponse},
        };

        use super::*;

        #[test]
        fn parses_correct_response() {
            let buf: &[u8] = b"OK\n";
            let mut c = Cursor::new(buf);

            let result = Client::read_ok(&mut c);

            assert_eq!(result, Ok(()));
        }

        #[test]
        fn returns_mpd_error() {
            let buf: &[u8] = b"ACK [55@2] {some_cmd} error message boi\n";
            let err = MpdFailureResponse {
                code: ErrorCode::PlayerSync,
                command_list_index: 2,
                command: "some_cmd".to_string(),
                message: "error message boi".to_string(),
            };
            let mut c = Cursor::new(buf);

            let result = Client::read_ok(&mut c);

            assert_eq!(result, Err(MpdError::Mpd(err)));
        }

        #[test]
        fn returns_error_when_receiving_value() {
            let buf: &[u8] = b"idc\nOK\n";
            let mut c = Cursor::new(buf);

            let result = Client::read_ok(&mut c);

            assert_eq!(
                result,
                Err(MpdError::Generic(String::from("Expected 'OK' but got 'idc'")))
            );
        }
    }

    mod binary {
        use std::io::Cursor;

        use crate::mpd::{
            client::{BinaryMpdResponse, Client},
            errors::{ErrorCode, MpdError, MpdFailureResponse},
        };

        #[test]
        fn returns_mpd_error() {
            let buf: &[u8] = b"ACK [55@2] {some_cmd} error message boi\n";
            let err = MpdFailureResponse {
                code: ErrorCode::PlayerSync,
                command_list_index: 2,
                command: "some_cmd".to_string(),
                message: "error message boi".to_string(),
            };
            let mut c = Cursor::new(buf);

            let result = Client::read_binary(&mut c, &mut Vec::new());

            assert_eq!(result, Err(MpdError::Mpd(err)));
        }

        #[test]
        fn returns_error_when_unknown_receiving_value() {
            let buf: &[u8] = b"idc: value\nOK\n";
            let mut c = Cursor::new(buf);

            let result = Client::read_binary(&mut c, &mut Vec::new());

            assert_eq!(
                result,
                Err(MpdError::Generic(String::from(
                    "Unexpected key when parsing binary response: 'idc'"
                )))
            );
        }

        #[test]
        fn returns_none_when_unknown_receiving_unexpected_ok() {
            let buf: &[u8] = b"OK\n";
            let mut c = Cursor::new(buf);

            let result = Client::read_binary(&mut c, &mut Vec::new());

            assert_eq!(result, Ok(None));
        }

        #[test]
        fn returns_success() {
            let bytes = &[0; 111];
            let buf: &[u8] = b"size: 222\ntype: image/png\nbinary: 111\n";
            let buf_end: &[u8] = b"\nOK\n";
            let mut c = Cursor::new([buf, bytes, buf_end].concat());

            let mut buf = Vec::new();
            let result = Client::read_binary(&mut c, &mut buf);

            assert_eq!(buf, bytes);
            assert_eq!(
                result,
                Ok(Some(BinaryMpdResponse {
                    bytes_read: 111,
                    size_total: 222,
                    mime_type: Some("image/png".to_owned())
                }))
            );
        }
    }
}
