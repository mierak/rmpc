use std::{
    io::{BufRead, Read},
    str::FromStr,
};

use anyhow::Result;

use super::{
    FromMpd,
    errors::{MpdError, MpdFailureResponse},
    split_line,
    version::Version,
};
use crate::{mpd::errors::ErrorCode, shared::string_util::StringExt};
type MpdResult<T> = Result<T, MpdError>;

#[derive(Debug, Default, PartialEq)]
pub struct BinaryMpdResponse {
    pub bytes_read: u64,
    pub size_total: u32,
    pub mime_type: Option<String>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum MpdLine {
    Ok,
    Value(String),
}

pub trait SocketClient {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<()>;
    fn read(&mut self) -> &mut impl BufRead;
    fn version(&self) -> Version;
    fn clear_read_buf(&mut self) -> Result<()>;
}

pub trait ProtoClient {
    fn should_reinit_buffer(err: &MpdError) -> bool {
        !matches!(
            err,
            MpdError::Mpd(MpdFailureResponse { code: ErrorCode::NoExist, .. })
                | MpdError::TimedOut(_)
        )
    }

    fn reinit_buffer_if_needed(&mut self, err: &MpdError) -> Result<()>;

    fn execute(&mut self, command: &str) -> Result<(), MpdError>;

    fn read_ok(&mut self) -> Result<(), MpdError>;

    fn read_response<V>(&mut self) -> Result<V, MpdError>
    where
        V: FromMpd + Default;

    fn read_opt_response<V>(&mut self) -> Result<Option<V>, MpdError>
    where
        V: FromMpd + Default;

    fn read_bin(&mut self, command: &str) -> MpdResult<Option<Vec<u8>>>;

    fn read_bin_inner(
        &mut self,
        binary_buf: &mut Vec<u8>,
    ) -> Result<Option<BinaryMpdResponse>, MpdError>;

    fn read_line(read: &mut impl BufRead) -> Result<MpdLine, MpdError>;
}

impl<T: SocketClient> ProtoClient for T {
    fn reinit_buffer_if_needed(&mut self, err: &MpdError) -> Result<()> {
        if Self::should_reinit_buffer(err) {
            log::error!(err:?; "read buffer was reinitialized");
            self.clear_read_buf()?;
        }

        Ok(())
    }

    fn execute(&mut self, command: &str) -> Result<(), MpdError> {
        log::trace!(command; "Executing MPD command");
        Ok(self.write([command, "\n"].concat().as_bytes())?)
    }

    fn read_ok(&mut self) -> Result<(), MpdError> {
        let read = self.read();

        match Self::read_line(read) {
            Ok(MpdLine::Ok) => Ok(()),
            Ok(MpdLine::Value(val)) => {
                log::error!(val = val.as_str(); "read buffer was reinitialized because we got a value when expecting ok");
                self.clear_read_buf()?;
                Err(MpdError::Generic(format!("Expected 'OK' but got '{val}'")))
            }
            Err(e) => {
                self.reinit_buffer_if_needed(&e)?;
                Err(e)
            }
        }
    }

    fn read_response<V>(&mut self) -> Result<V, MpdError>
    where
        V: FromMpd + Default,
    {
        let mut result = V::default();
        let read = self.read();

        loop {
            match Self::read_line(read) {
                Ok(MpdLine::Ok) => return Ok(result),
                Ok(MpdLine::Value(val)) => {
                    if let Err(e) = result.next(val) {
                        self.reinit_buffer_if_needed(&e)?;
                        return Err(e);
                    }
                }
                Err(e) => {
                    self.reinit_buffer_if_needed(&e)?;
                    return Err(e);
                }
            }
        }
    }

    fn read_opt_response<V>(&mut self) -> Result<Option<V>, MpdError>
    where
        V: FromMpd + Default,
    {
        let mut result = V::default();
        let mut found_any = false;
        let read = self.read();
        loop {
            match Self::read_line(read) {
                Ok(MpdLine::Ok) => {
                    return if found_any { Ok(Some(result)) } else { Ok(None) };
                }
                Ok(MpdLine::Value(val)) => {
                    found_any = true;
                    if let Err(e) = result.next(val) {
                        self.reinit_buffer_if_needed(&e)?;
                        return Err(e);
                    }
                }
                Err(e) => {
                    self.reinit_buffer_if_needed(&e)?;
                    return Err(e);
                }
            }
        }
    }

    fn read_bin(&mut self, command: &str) -> MpdResult<Option<Vec<u8>>> {
        let mut buf = Vec::new();
        // trim the 0 offset from the initial command because we substitute
        // an actual value here
        let _ = match self.read_bin_inner(&mut buf) {
            Ok(Some(v)) => Ok(Some(v)),
            Ok(None) => return Ok(None),
            Err(e) => {
                self.reinit_buffer_if_needed(&e)?;
                Err(e)
            }
        };

        loop {
            let command = command.trim_end_matches(" 0");
            let command = format!("{} {}", command, buf.len());
            self.execute(command.as_ref())?;
            match self.read_bin_inner(&mut buf) {
                Ok(Some(response)) => {
                    if buf.len() >= response.size_total as usize || response.bytes_read == 0 {
                        log::trace!(len = buf.len(); "Finished reading binary response");
                        break;
                    }
                }
                Ok(None) => return Ok(None),
                Err(e) => {
                    self.reinit_buffer_if_needed(&e)?;
                    return Err(e);
                }
            }
        }
        Ok(Some(buf))
    }

    fn read_bin_inner(
        &mut self,
        binary_buf: &mut Vec<u8>,
    ) -> Result<Option<BinaryMpdResponse>, MpdError> {
        let mut result = BinaryMpdResponse::default();
        let read = self.read();
        {
            loop {
                match Self::read_line(read)? {
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
                                )));
                            }
                        }
                    }
                }
            }
        }

        let mut handle = read.take(result.bytes_read);
        let _ = handle.read_to_end(binary_buf)?;
        let _ = read.read_line(&mut String::new()); // MPD prints an empty new line at the end of binary response
        match Self::read_line(read)? {
            MpdLine::Ok => Ok(Some(result)),
            MpdLine::Value(val) => Err(MpdError::Generic(format!("Expected 'OK' but got '{val}'"))),
        }
    }

    fn read_line(read: &mut impl BufRead) -> Result<MpdLine, MpdError> {
        let mut buf = Vec::new();

        let bytes_read = match read.read_until(b'\n', &mut buf) {
            Ok(v) => Ok(v),
            Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => {
                log::error!(err:? = e; "Got broken pipe from mpd");
                Err(MpdError::ClientClosed)
            }
            Err(e)
                if e.kind() == std::io::ErrorKind::TimedOut
                    || e.kind() == std::io::ErrorKind::WouldBlock =>
            {
                log::trace!(err:? = e; "Reading line from MPD timed out");
                Err(e.into())
            }
            Err(e) => {
                log::error!(err:? = e; "Encountered unexpected error when reading a response line from MPD");
                Err(e.into())
            }
        }?;

        let mut line = String::from_utf8_lossy_as_owned(buf);

        if bytes_read == 0 {
            log::error!("Got an empty line in MPD's response");
            return Err(MpdError::ValueExpected(
                "Expected value when reading MPD's response but the stream reached EOF".to_string(),
            ));
        }

        if line.starts_with("OK") || line.starts_with("list_OK") {
            log::trace!(line = line.as_str().trim(); "Read MPD line OK");
            return Ok(MpdLine::Ok);
        }
        if line.starts_with("ACK") {
            log::error!(line = line.as_str().trim(); "Read MPD line with error");
            return Err(MpdError::Mpd(MpdFailureResponse::from_str(&line)?));
        }
        line.pop(); // pop the new line
        log::trace!(line = line.as_str().trim(); "Read MPD line");
        Ok(MpdLine::Value(line))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::io::{BufReader, Cursor};

    use super::SocketClient;
    use crate::mpd::{FromMpd, LineHandled, errors::MpdError, version::Version};

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
                _ => {
                    return Err(MpdError::Generic(String::from("unknown value")));
                }
            }
            Ok(LineHandled::Yes)
        }
    }

    struct TestClient {
        read: BufReader<Cursor<Vec<u8>>>,
    }
    impl TestClient {
        fn new(buf: &[u8]) -> Self {
            Self { read: BufReader::new(Cursor::new(buf.to_vec())) }
        }
    }
    impl SocketClient for TestClient {
        fn write(&mut self, _bytes: &[u8]) -> std::io::Result<()> {
            Ok(())
        }

        fn read(&mut self) -> &mut impl std::io::BufRead {
            &mut self.read
        }

        fn clear_read_buf(&mut self) -> anyhow::Result<()> {
            Ok(())
        }

        fn version(&self) -> Version {
            Version::new(0, 25, 0)
        }
    }

    mod read_mpd_line {

        use std::io::{BufReader, Cursor};

        use rstest::rstest;

        use crate::{
            mpd::{
                errors::{ErrorCode, MpdError, MpdFailureResponse},
                proto_client::{MpdLine, ProtoClient, SocketClient},
            },
            tests::fixtures::mpd_client::{TestMpdClient, client},
        };

        #[rstest]
        fn returns_ok(mut client: TestMpdClient) {
            client.set_read_content(Box::new(Cursor::new(b"OK enenene")));
            let result = TestMpdClient::read_line(client.read());

            assert_eq!(Ok(MpdLine::Ok), result);
        }

        #[rstest]
        fn returns_ok_for_list_ok(mut client: TestMpdClient) {
            client.set_read_content(Box::new(Cursor::new(b"list_OK enenene")));
            let result = TestMpdClient::read_line(client.read());

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

            client.set_read_content(Box::new(Cursor::new(
                b"ACK [55@2] {some_cmd} error message boi",
            )));
            let result = TestMpdClient::read_line(client.read());

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
            let result = TestMpdClient::read_line(client.read());

            assert_eq!(Err(MpdError::ClientClosed), result);
        }
    }

    mod response {

        use super::*;
        use crate::mpd::{
            errors::{ErrorCode, MpdError, MpdFailureResponse},
            proto_client::ProtoClient,
        };

        #[test]
        fn parses_correct_response() {
            let buf: &[u8] = b"val_b: a\nval_a: 5\nOK\n";

            let result = TestClient::new(buf).read_response::<TestMpdObject>();

            assert_eq!(result, Ok(TestMpdObject { val_a: "5".to_owned(), val_b: "a".to_owned() }));
        }

        #[test]
        fn returns_parse_error() {
            let buf: &[u8] = b"fail: lol\nOK\n";

            let result = TestClient::new(buf).read_response::<TestMpdObject>();

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

            let result = TestClient::new(buf).read_response::<TestMpdObject>();

            assert_eq!(result, Err(MpdError::Mpd(err)));
        }
    }
    mod response_opt {
        use super::*;
        use crate::mpd::{
            errors::{ErrorCode, MpdError, MpdFailureResponse},
            proto_client::ProtoClient,
        };

        #[test]
        fn parses_correct_response() {
            let buf: &[u8] = b"val_b: a\nval_a: 5\nOK\n";

            let result = TestClient::new(buf).read_opt_response::<TestMpdObject>();

            assert_eq!(
                result,
                Ok(Some(TestMpdObject { val_a: "5".to_owned(), val_b: "a".to_owned() }))
            );
        }

        #[test]
        fn returns_none() {
            let buf: &[u8] = b"OK\n";

            let result = TestClient::new(buf).read_opt_response::<TestMpdObject>();

            assert_eq!(result, Ok(None));
        }

        #[test]
        fn returns_parse_error() {
            let buf: &[u8] = b"fail: lol\nOK\n";

            let result = TestClient::new(buf).read_opt_response::<TestMpdObject>();

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

            let result = TestClient::new(buf).read_opt_response::<TestMpdObject>();

            assert_eq!(result, Err(MpdError::Mpd(err)));
        }
    }

    mod ok {
        use super::*;
        use crate::mpd::{
            errors::{ErrorCode, MpdFailureResponse},
            proto_client::ProtoClient,
        };

        #[test]
        fn parses_correct_response() {
            let buf: &[u8] = b"OK\n";

            let result = TestClient::new(buf).read_ok();

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

            let result = TestClient::new(buf).read_ok();

            assert_eq!(result, Err(MpdError::Mpd(err)));
        }

        #[test]
        fn returns_error_when_receiving_value() {
            let buf: &[u8] = b"idc\nOK\n";

            let result = TestClient::new(buf).read_ok();

            assert_eq!(result, Err(MpdError::Generic(String::from("Expected 'OK' but got 'idc'"))));
        }
    }

    mod binary {
        use crate::mpd::{
            errors::{ErrorCode, MpdError, MpdFailureResponse},
            proto_client::{BinaryMpdResponse, ProtoClient, tests::TestClient},
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

            let result = TestClient::new(buf).read_bin_inner(&mut Vec::new());

            assert_eq!(result, Err(MpdError::Mpd(err)));
        }

        #[test]
        fn returns_error_when_unknown_receiving_value() {
            let buf: &[u8] = b"idc: value\nOK\n";

            let result = TestClient::new(buf).read_bin_inner(&mut Vec::new());

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

            let result = TestClient::new(buf).read_bin_inner(&mut Vec::new());

            assert_eq!(result, Ok(None));
        }

        #[test]
        fn returns_success() {
            let bytes = &[0; 111];
            let buf: &[u8] = b"size: 222\ntype: image/png\nbinary: 111\n";
            let buf_end: &[u8] = b"\nOK\n";
            let c = [buf, bytes, buf_end].concat();
            let mut client = TestClient::new(&c);

            let mut buf = Vec::new();
            let result = client.read_bin_inner(&mut buf);

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
