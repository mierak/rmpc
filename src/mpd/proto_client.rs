use std::{
    io::{BufRead, Read},
    str::FromStr,
};

use anyhow::Result;
use log::trace;

use crate::mpd::errors::ErrorCode;

use super::{
    errors::{MpdError, MpdFailureResponse},
    split_line, FromMpd,
};
type MpdResult<T> = Result<T, MpdError>;

pub struct ProtoClient<'cmd, 'client, C: SocketClient> {
    command: &'cmd str,
    client: &'client mut C,
}

#[derive(Debug, Default, PartialEq)]
struct BinaryMpdResponse {
    pub bytes_read: u64,
    pub size_total: u32,
    pub mime_type: Option<String>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum MpdLine {
    Ok,
    Value(String),
}

impl<C: SocketClient> std::fmt::Debug for ProtoClient<'_, '_, C> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.command)
    }
}

pub trait SocketClient {
    fn reconnect(&mut self) -> MpdResult<&impl SocketClient>;
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<()>;
    fn read(&mut self) -> &mut impl BufRead;
    fn clear_read_buf(&mut self) -> Result<()>;
}

impl<'cmd, 'client, C: SocketClient> ProtoClient<'cmd, 'client, C> {
    pub fn new(input: &'cmd str, client: &'client mut C) -> Result<Self, MpdError> {
        let mut res = Self { command: input, client };
        res.execute(input)?;
        Ok(res)
    }

    fn execute(&mut self, command: &str) -> Result<&mut Self, MpdError> {
        trace!(command = self.command; "Executing command");
        if let Err(e) = self.client.write([command, "\n"].concat().as_bytes()) {
            if e.kind() == std::io::ErrorKind::BrokenPipe {
                log::error!(err:? = e; "Got broken pipe from mpd");
                self.client.reconnect()?;
                self.client.write([command, "\n"].concat().as_bytes())?;
                Ok(self)
            } else {
                Err(e.into())
            }
        } else {
            Ok(self)
        }
    }

    pub(super) fn read_ok(mut self) -> Result<(), MpdError> {
        trace!(command = self.command; "Reading command");
        match self.read_line() {
            Ok(MpdLine::Ok) => Ok(()),
            Ok(MpdLine::Value(val)) => Err(MpdError::Generic(format!("Expected 'OK' but got '{val}'"))),
            Err(MpdError::ClientClosed) => {
                self.client.reconnect()?;
                self.execute(self.command)?;
                self.read_ok()
            }
            Err(e) => {
                if !matches!(
                    e,
                    MpdError::Mpd(MpdFailureResponse {
                        code: ErrorCode::NoExist,
                        ..
                    })
                ) {
                    log::error!(e:?; "read buffer was reinitialized buffer was reinitialized");
                    self.client.clear_read_buf()?;
                }
                Err(e)
            }
        }
    }

    fn next<V: FromMpd>(&mut self, v: &mut V, val: String) -> Result<(), MpdError> {
        match v.next(val) {
            Ok(val) => Ok(val),
            Err(e) => {
                if !matches!(
                    e,
                    MpdError::Mpd(MpdFailureResponse {
                        code: ErrorCode::NoExist,
                        ..
                    })
                ) {
                    log::error!(e:?; "read buffer was reinitialized buffer was reinitialized");
                    self.client.clear_read_buf()?;
                }
                Err(e)
            }
        }
    }

    pub(crate) fn read_response<V>(mut self) -> Result<V, MpdError>
    where
        V: FromMpd + Default,
    {
        trace!(command = self.command; "Reading command");
        let mut result = V::default();
        loop {
            match self.read_line() {
                Ok(MpdLine::Ok) => return Ok(result),
                Ok(MpdLine::Value(val)) => self.next(&mut result, val)?,
                Err(MpdError::ClientClosed) => {
                    self.client.reconnect()?;
                    self.execute(self.command)?;
                    return self.read_response::<V>();
                }
                Err(e) => {
                    if !matches!(
                        e,
                        MpdError::Mpd(MpdFailureResponse {
                            code: ErrorCode::NoExist,
                            ..
                        })
                    ) {
                        log::error!(e:?; "read buffer was reinitialized buffer was reinitialized");
                        self.client.clear_read_buf()?;
                    }
                    return Err(e);
                }
            };
        }
    }

    pub(super) fn read_opt_response<V>(mut self) -> Result<Option<V>, MpdError>
    where
        V: FromMpd + Default,
    {
        trace!(command = self.command; "Reading command");
        let mut result = V::default();
        let mut found_any = false;
        loop {
            match self.read_line() {
                Ok(MpdLine::Ok) => return if found_any { Ok(Some(result)) } else { Ok(None) },
                Ok(MpdLine::Value(val)) => {
                    found_any = true;
                    self.next(&mut result, val)?;
                }
                Err(MpdError::ClientClosed) => {
                    self.client.reconnect()?;
                    self.execute(self.command)?;
                    return self.read_opt_response::<V>();
                }
                Err(e) => {
                    if !matches!(
                        e,
                        MpdError::Mpd(MpdFailureResponse {
                            code: ErrorCode::NoExist,
                            ..
                        })
                    ) {
                        log::error!(e:?; "read buffer was reinitialized buffer was reinitialized");
                        self.client.clear_read_buf()?;
                    }
                    return Err(e);
                }
            }
        }
    }

    pub(super) fn read_bin(mut self) -> MpdResult<Option<Vec<u8>>> {
        let mut buf = Vec::new();
        // trim the 0 offset from the initial command because we substitute
        // an actual value here
        let command = self.command.trim_end_matches(" 0");
        let _ = match self.read_bin_inner(&mut buf) {
            Ok(Some(v)) => Ok(Some(v)),
            Ok(None) => return Ok(None),
            Err(MpdError::ClientClosed) => {
                self.client.reconnect()?;
                self.execute(&format!("{} {}", command, buf.len()))?;
                self.read_bin_inner(&mut buf)
            }
            Err(e) => {
                if !matches!(
                    e,
                    MpdError::Mpd(MpdFailureResponse {
                        code: ErrorCode::NoExist,
                        ..
                    })
                ) {
                    log::error!(e:?; "read buffer was reinitialized buffer was reinitialized");
                    self.client.clear_read_buf()?;
                }
                Err(e)
            }
        };
        loop {
            let command = format!("{} {}", command, buf.len());
            log::trace!(len = buf.len(), command = command.as_str(); "Requesting more binary data");
            self.execute(&command)?;
            match self.read_bin_inner(&mut buf) {
                Ok(Some(response)) => {
                    if buf.len() >= response.size_total as usize || response.bytes_read == 0 {
                        trace!( len = buf.len();"Finshed reading binary response");
                        break;
                    }
                }
                Ok(None) => return Ok(None),
                Err(e) => {
                    if !matches!(
                        e,
                        MpdError::Mpd(MpdFailureResponse {
                            code: ErrorCode::NoExist,
                            ..
                        })
                    ) {
                        log::error!(e:?; "read buffer was reinitialized buffer was reinitialized");
                        self.client.clear_read_buf()?;
                    }
                    return Err(e);
                }
            }
        }
        Ok(Some(buf))
    }

    fn read_bin_inner(&mut self, binary_buf: &mut Vec<u8>) -> Result<Option<BinaryMpdResponse>, MpdError> {
        let mut result = BinaryMpdResponse::default();
        {
            loop {
                match self.read_line()? {
                    MpdLine::Ok => {
                        log::warn!("Expected binary data but got 'OK'");
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

        let read = self.client.read();
        let mut handle = read.take(result.bytes_read);
        let _ = handle.read_to_end(binary_buf)?;
        let _ = read.read_line(&mut String::new()); // MPD prints an empty new line at the end of binary response
        match self.read_line()? {
            MpdLine::Ok => Ok(Some(result)),
            MpdLine::Value(val) => Err(MpdError::Generic(format!("Expected 'OK' but got '{val}'"))),
        }
    }

    fn read_line(&mut self) -> Result<MpdLine, MpdError> {
        let read = self.client.read();
        let mut line = String::new();
        std::thread::sleep(std::time::Duration::from_millis(1));

        let bytes_read = match read.read_line(&mut line) {
            Ok(v) => Ok(v),
            Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => {
                log::error!(err:? = e; "Got broken pipe from mpd");
                Err(MpdError::ClientClosed)
            }
            Err(e) => {
                log::error!(err:? = e; "Encountered unexpected error whe reading a response  line from MPD");
                Err(e.into())
            }
        }?;

        if bytes_read == 0 {
            log::error!("Got an empty line in MPD's response");
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

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::io::{BufReader, Cursor};

    use crate::mpd::{errors::MpdError, FromMpd, LineHandled};

    use super::SocketClient;

    #[derive(Default, Debug, PartialEq, Eq)]
    struct TestMpdObject {
        val_a: String,
        val_b: String,
    }
    impl FromMpd for TestMpdObject {
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

    struct TestClient {
        read: BufReader<Cursor<Vec<u8>>>,
    }
    impl TestClient {
        fn new(buf: &[u8]) -> Self {
            Self {
                read: BufReader::new(Cursor::new(buf.to_vec())),
            }
        }
    }
    impl SocketClient for TestClient {
        fn reconnect(&mut self) -> super::MpdResult<&impl SocketClient> {
            Ok(self)
        }
        fn write(&mut self, _bytes: &[u8]) -> std::io::Result<()> {
            Ok(())
        }
        fn read(&mut self) -> &mut impl std::io::BufRead {
            &mut self.read
        }

        fn clear_read_buf(&mut self) -> anyhow::Result<()> {
            Ok(())
        }
    }

    mod read_mpd_line {

        use std::io::{BufReader, Cursor};

        use crate::tests::fixtures::mpd_client::{client, TestMpdClient};
        use rstest::rstest;

        use crate::mpd::{
            errors::{ErrorCode, MpdError, MpdFailureResponse},
            proto_client::{MpdLine, ProtoClient},
        };

        #[rstest]
        fn returns_ok(mut client: TestMpdClient) {
            client.set_read_content(Box::new(Cursor::new(b"OK enenene")));
            let mut client = ProtoClient::new("", &mut client).unwrap();
            let result = client.read_line();

            assert_eq!(Ok(MpdLine::Ok), result);
        }

        #[rstest]
        fn returns_ok_for_list_ok(mut client: TestMpdClient) {
            client.set_read_content(Box::new(Cursor::new(b"list_OK enenene")));
            let mut client = ProtoClient::new("", &mut client).unwrap();
            let result = client.read_line();

            assert_eq!(Ok(MpdLine::Ok), result);
        }

        #[rstest]
        fn returns_mpd_err(mut client: TestMpdClient) {
            let err = MpdFailureResponse {
                code: ErrorCode::PlayerSync,
                command_list_index: 2,
                command: "some_cmd".to_string(),
                message: "error message boi".to_string(),
            };

            client.set_read_content(Box::new(Cursor::new(b"ACK [55@2] {some_cmd} error message boi")));
            let mut client = ProtoClient::new("", &mut client).unwrap();
            let result = client.read_line();

            assert_eq!(Err(MpdError::Mpd(err)), result);
        }

        #[rstest]
        fn returns_client_closed_on_broken_pipe(mut client: TestMpdClient) {
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

            client.set_read(BufReader::new(Box::new(Mock)));
            let mut client = ProtoClient::new("", &mut client).unwrap();
            let result = client.read_line();

            assert_eq!(Err(MpdError::ClientClosed), result);
        }
    }

    mod response {

        use crate::mpd::{
            errors::{ErrorCode, MpdError, MpdFailureResponse},
            proto_client::ProtoClient,
        };

        use super::*;

        #[test]
        fn parses_correct_response() {
            let buf: &[u8] = b"val_b: a\nval_a: 5\nOK\n";

            let result = ProtoClient::new("", &mut TestClient::new(buf))
                .unwrap()
                .read_response::<TestMpdObject>();

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

            let result = ProtoClient::new("", &mut TestClient::new(buf))
                .unwrap()
                .read_response::<TestMpdObject>();

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

            let result = ProtoClient::new("", &mut TestClient::new(buf))
                .unwrap()
                .read_response::<TestMpdObject>();

            assert_eq!(result, Err(MpdError::Mpd(err)));
        }
    }
    mod response_opt {
        use crate::mpd::{
            errors::{ErrorCode, MpdError, MpdFailureResponse},
            proto_client::ProtoClient,
        };

        use super::*;

        #[test]
        fn parses_correct_response() {
            let buf: &[u8] = b"val_b: a\nval_a: 5\nOK\n";

            let result = ProtoClient::new("", &mut TestClient::new(buf))
                .unwrap()
                .read_opt_response::<TestMpdObject>();

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

            let result = ProtoClient::new("", &mut TestClient::new(buf))
                .unwrap()
                .read_opt_response::<TestMpdObject>();

            assert_eq!(result, Ok(None));
        }

        #[test]
        fn returns_parse_error() {
            let buf: &[u8] = b"fail: lol\nOK\n";

            let result = ProtoClient::new("", &mut TestClient::new(buf))
                .unwrap()
                .read_opt_response::<TestMpdObject>();

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

            let result = ProtoClient::new("", &mut TestClient::new(buf))
                .unwrap()
                .read_opt_response::<TestMpdObject>();

            assert_eq!(result, Err(MpdError::Mpd(err)));
        }
    }

    mod ok {
        use crate::mpd::{
            errors::{ErrorCode, MpdFailureResponse},
            proto_client::ProtoClient,
        };

        use super::*;

        #[test]
        fn parses_correct_response() {
            let buf: &[u8] = b"OK\n";

            let result = ProtoClient::new("", &mut TestClient::new(buf)).unwrap().read_ok();

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

            let result = ProtoClient::new("", &mut TestClient::new(buf)).unwrap().read_ok();

            assert_eq!(result, Err(MpdError::Mpd(err)));
        }

        #[test]
        fn returns_error_when_receiving_value() {
            let buf: &[u8] = b"idc\nOK\n";

            let result = ProtoClient::new("", &mut TestClient::new(buf)).unwrap().read_ok();

            assert_eq!(
                result,
                Err(MpdError::Generic(String::from("Expected 'OK' but got 'idc'")))
            );
        }
    }

    mod binary {
        use crate::mpd::{
            errors::{ErrorCode, MpdError, MpdFailureResponse},
            proto_client::{tests::TestClient, BinaryMpdResponse, ProtoClient},
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

            let result = ProtoClient::new("", &mut TestClient::new(buf))
                .unwrap()
                .read_bin_inner(&mut Vec::new());

            assert_eq!(result, Err(MpdError::Mpd(err)));
        }

        #[test]
        fn returns_error_when_unknown_receiving_value() {
            let buf: &[u8] = b"idc: value\nOK\n";

            let result = ProtoClient::new("", &mut TestClient::new(buf))
                .unwrap()
                .read_bin_inner(&mut Vec::new());

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

            let result = ProtoClient::new("", &mut TestClient::new(buf))
                .unwrap()
                .read_bin_inner(&mut Vec::new());

            assert_eq!(result, Ok(None));
        }

        #[test]
        fn returns_success() {
            let bytes = &[0; 111];
            let buf: &[u8] = b"size: 222\ntype: image/png\nbinary: 111\n";
            let buf_end: &[u8] = b"\nOK\n";
            let c = [buf, bytes, buf_end].concat();
            let mut client = TestClient::new(&c);
            let mut command = ProtoClient::new("", &mut client).unwrap();

            let mut buf = Vec::new();
            let result = command.read_bin_inner(&mut buf);

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
