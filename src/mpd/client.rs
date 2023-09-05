use std::str::FromStr;

use anyhow::Result;
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    net::{
        tcp::{OwnedReadHalf, OwnedWriteHalf},
        TcpStream,
    },
};
use tracing::{debug, trace};

use super::{
    errors::{MpdError, MpdFailureResponse},
    split_line, FromMpd, FromMpdBuilder,
};

type MpdResult<T> = Result<T, MpdError>;

pub struct Client<'a> {
    name: Option<&'a str>,
    rx: BufReader<OwnedReadHalf>,
    tx: OwnedWriteHalf,
    reconnect: bool,
    addr: String,
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
    pub async fn init(addr: String, name: Option<&'a str>, reconnect: bool) -> MpdResult<Client<'a>> {
        let stream = TcpStream::connect(&addr).await?;
        let (rx, tx) = stream.into_split();
        let mut rx = BufReader::new(rx);

        let mut buf = String::new();
        rx.read_line(&mut buf).await?;
        if !buf.starts_with("OK") {
            return Err(MpdError::Generic(format!("Handshake validation failed. '{buf}'")));
        };

        debug!(message = "MPD client initiazed", handshake = buf.trim());
        Ok(Self {
            name,
            rx,
            tx,
            reconnect,
            addr,
        })
    }

    #[tracing::instrument]
    async fn reconnect(&mut self) -> MpdResult<&Client> {
        let stream = TcpStream::connect(&self.addr).await?;
        let (rx, tx) = stream.into_split();
        let mut rx = BufReader::new(rx);

        let mut buf = String::new();
        rx.read_line(&mut buf).await?;
        if !buf.starts_with("OK") {
            return Err(MpdError::Generic(format!("Handshake validation failed. '{buf}'")));
        };
        self.rx = rx;
        self.tx = tx;

        debug!(message = "MPD client reconnected", handshake = buf.trim(),);
        Ok(self)
    }

    #[tracing::instrument(skip(self), fields(command = ?command))]
    pub(super) async fn execute_binary(&mut self, command: &str) -> MpdResult<Vec<u8>> {
        let mut buf = Vec::new();

        self.tx
            .write_all(format!("{command} {} \n", buf.len()).as_bytes())
            .await?;
        let _ = match Self::read_binary(&mut self.rx, &mut buf).await {
            Ok(v) => Ok(v),
            Err(MpdError::ClientClosed) if self.reconnect => {
                self.reconnect().await?;
                self.tx
                    .write_all(format!("{command} {} \n", buf.len()).as_bytes())
                    .await?;
                Self::read_binary(&mut self.rx, &mut buf).await
            }
            Err(e) => Err(e),
        };
        loop {
            self.tx
                .write_all(format!("{command} {} \n", buf.len()).as_bytes())
                .await?;
            let response = Self::read_binary(&mut self.rx, &mut buf).await?;

            if buf.len() >= response.size_total as usize || response.bytes_read == 0 {
                trace!(message = "Finshed reading binary response", len = buf.len());
                break;
            }
        }
        Ok(buf)
    }

    #[tracing::instrument(skip(self), fields(command = ?command))]
    pub(super) async fn execute<T>(&mut self, command: &str) -> MpdResult<T>
    where
        T: FromMpd + FromMpdBuilder<T>,
    {
        self.tx.write_all([command, "\n"].concat().as_bytes()).await?;
        match Self::read::<BufReader<OwnedReadHalf>, T, T>(&mut self.rx).await {
            Ok(v) => Ok(v),
            Err(MpdError::ClientClosed) => {
                self.reconnect().await?;
                self.tx.write_all([command, "\n"].concat().as_bytes()).await?;
                Self::read::<BufReader<OwnedReadHalf>, T, T>(&mut self.rx).await
            }
            Err(e) => Err(e),
        }
    }

    #[tracing::instrument(skip(self), fields(command = ?command))]
    pub(super) async fn execute_option<T>(&mut self, command: &str) -> MpdResult<Option<T>>
    where
        T: FromMpd + FromMpdBuilder<T>,
    {
        self.tx.write_all([command, "\n"].concat().as_bytes()).await?;
        match Self::read_option::<BufReader<OwnedReadHalf>, T, T>(&mut self.rx).await {
            Ok(v) => Ok(v),
            Err(MpdError::ClientClosed) => {
                self.reconnect().await?;
                self.tx.write_all([command, "\n"].concat().as_bytes()).await?;
                Self::read_option::<BufReader<OwnedReadHalf>, T, T>(&mut self.rx).await
            }
            Err(e) => Err(e),
        }
    }

    #[tracing::instrument(skip(self), fields(command = ?command))]
    pub(super) async fn execute_ok(&mut self, command: &str) -> MpdResult<()> {
        self.tx.write_all([command, "\n"].concat().as_bytes()).await?;
        match Self::read_ok(&mut self.rx).await {
            Ok(v) => Ok(v),
            Err(MpdError::ClientClosed) => {
                self.reconnect().await?;
                self.tx.write_all([command, "\n"].concat().as_bytes()).await?;
                Self::read_ok(&mut self.rx).await
            }
            Err(e) => Err(e),
        }
    }

    #[tracing::instrument(skip(read, binary_buf), fields(buf_len = binary_buf.len()))]
    async fn read_binary<R: std::fmt::Debug>(
        read: &mut R,
        binary_buf: &mut Vec<u8>,
    ) -> Result<BinaryMpdResponse, MpdError>
    where
        R: tokio::io::AsyncBufRead + Unpin,
    {
        let mut result = BinaryMpdResponse::default();
        {
            let mut lines = read.lines();
            loop {
                match Self::read_mpd_line(lines.next_line().await?)? {
                    MpdLine::Ok => return Err(MpdError::Generic("Expected binary data but got 'OK'".to_owned())),
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
        let _ = handle.read_to_end(binary_buf).await?;
        let _ = read.read_line(&mut String::new()).await; // MPD prints an empty new line at the end of binary response
        match Self::read_mpd_line(read.lines().next_line().await?)? {
            MpdLine::Ok => Ok(result),
            MpdLine::Value(val) => Err(MpdError::Generic(format!("Expected 'OK' but got '{val}'"))),
        }
    }

    #[tracing::instrument(skip(read))]
    async fn read<R, A, V>(read: &mut R) -> Result<V, MpdError>
    where
        R: tokio::io::AsyncBufRead + Unpin,
        V: FromMpd,
        A: FromMpdBuilder<V>,
    {
        trace!(message = "Reading command");
        let mut result = A::create();
        let mut lines = read.lines();
        loop {
            match Self::read_mpd_line(lines.next_line().await?)? {
                MpdLine::Ok => break,
                MpdLine::Value(val) => result.next(val)?,
            };
        }

        result.finish()
    }

    #[tracing::instrument(skip(read))]
    async fn read_ok<R>(read: &mut R) -> Result<(), MpdError>
    where
        R: tokio::io::AsyncBufRead + Unpin,
    {
        trace!(message = "Reading command");
        let mut lines = read.lines();
        match Self::read_mpd_line(lines.next_line().await?)? {
            MpdLine::Ok => Ok(()),
            MpdLine::Value(val) => Err(MpdError::Generic(format!("Expected 'OK' but got '{val}'"))),
        }
    }

    #[tracing::instrument(skip(read))]
    async fn read_option<R, A, V>(read: &mut R) -> Result<Option<V>, MpdError>
    where
        R: tokio::io::AsyncBufRead + Unpin,
        V: FromMpd,
        A: FromMpdBuilder<V>,
    {
        trace!(message = "Reading command");
        let mut result = A::create();
        let mut lines = read.lines();
        let mut found_any = false;
        loop {
            match Self::read_mpd_line(lines.next_line().await?)? {
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

    fn read_mpd_line(line: Option<String>) -> Result<MpdLine, MpdError> {
        if let Some(line) = line {
            if line.starts_with("OK") || line.starts_with("list_OK") {
                return Ok(MpdLine::Ok);
            }
            if line.starts_with("ACK") {
                return Err(MpdError::Mpd(MpdFailureResponse::from_str(&line)?));
            }
            Ok(MpdLine::Value(line))
        } else {
            Err(MpdError::ClientClosed)
        }
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
        use crate::mpd::{
            client::{Client, MpdLine},
            errors::{ErrorCode, MpdError, MpdFailureResponse},
        };

        #[tokio::test]
        async fn returns_ok() {
            let result = Client::read_mpd_line(Some("OK enenene".to_owned()));

            assert_eq!(Ok(MpdLine::Ok), result);
        }

        #[tokio::test]
        async fn returns_ok_for_list_ok() {
            let result = Client::read_mpd_line(Some("list_OK enenene".to_owned()));

            assert_eq!(Ok(MpdLine::Ok), result);
        }

        #[tokio::test]
        async fn returns_mpd_err() {
            let err = MpdFailureResponse {
                code: ErrorCode::PlayerSync,
                command_list_index: 2,
                command: "some_cmd".to_string(),
                message: "error message boi".to_string(),
            };

            let result = Client::read_mpd_line(Some("ACK [55@2] {some_cmd} error message boi".to_owned()));

            assert_eq!(Err(MpdError::Mpd(err)), result);
        }

        #[tokio::test]
        async fn returns_client_closed() {
            let result = Client::read_mpd_line(None);

            assert_eq!(Err(MpdError::ClientClosed), result);
        }
    }

    mod response {
        use crate::mpd::{
            client::Client,
            errors::{ErrorCode, MpdError, MpdFailureResponse},
        };

        use super::*;

        #[tokio::test]
        async fn parses_correct_response() {
            let buf: &[u8] = b"val_b: a\nval_a: 5\nOK\n";
            let mut c = Cursor::new(buf);

            let result = Client::read::<Cursor<&[u8]>, TestMpdObject, TestMpdObject>(&mut c).await;

            assert_eq!(
                result,
                Ok(TestMpdObject {
                    val_a: "5".to_owned(),
                    val_b: "a".to_owned()
                })
            );
        }

        #[tokio::test]
        async fn returns_parse_error() {
            let buf: &[u8] = b"fail: lol\nOK\n";
            let mut c = Cursor::new(buf);

            let result = Client::read::<Cursor<&[u8]>, TestMpdObject, TestMpdObject>(&mut c).await;

            assert_eq!(result, Err(MpdError::Generic(String::from("intentional fail"))));
        }

        #[tokio::test]
        async fn returns_mpd_error() {
            let buf: &[u8] = b"ACK [55@2] {some_cmd} error message boi\n";
            let err = MpdFailureResponse {
                code: ErrorCode::PlayerSync,
                command_list_index: 2,
                command: "some_cmd".to_string(),
                message: "error message boi".to_string(),
            };
            let mut c = Cursor::new(buf);

            let result = Client::read::<Cursor<&[u8]>, TestMpdObject, TestMpdObject>(&mut c).await;

            assert_eq!(result, Err(MpdError::Mpd(err)));
        }
    }
    mod response_opt {
        use crate::mpd::{
            client::Client,
            errors::{ErrorCode, MpdError, MpdFailureResponse},
        };

        use super::*;

        #[tokio::test]
        async fn parses_correct_response() {
            let buf: &[u8] = b"val_b: a\nval_a: 5\nOK\n";
            let mut c = Cursor::new(buf);

            let result = Client::read_option::<Cursor<&[u8]>, TestMpdObject, TestMpdObject>(&mut c).await;

            assert_eq!(
                result,
                Ok(Some(TestMpdObject {
                    val_a: "5".to_owned(),
                    val_b: "a".to_owned()
                }))
            );
        }

        #[tokio::test]
        async fn returns_none() {
            let buf: &[u8] = b"OK\n";
            let mut c = Cursor::new(buf);

            let result = Client::read_option::<Cursor<&[u8]>, TestMpdObject, TestMpdObject>(&mut c).await;

            assert_eq!(result, Ok(None));
        }

        #[tokio::test]
        async fn returns_parse_error() {
            let buf: &[u8] = b"fail: lol\nOK\n";
            let mut c = Cursor::new(buf);

            let result = Client::read_option::<Cursor<&[u8]>, TestMpdObject, TestMpdObject>(&mut c).await;

            assert_eq!(result, Err(MpdError::Generic(String::from("intentional fail"))));
        }

        #[tokio::test]
        async fn returns_mpd_error() {
            let buf: &[u8] = b"ACK [55@2] {some_cmd} error message boi\n";
            let err = MpdFailureResponse {
                code: ErrorCode::PlayerSync,
                command_list_index: 2,
                command: "some_cmd".to_string(),
                message: "error message boi".to_string(),
            };
            let mut c = Cursor::new(buf);

            let result = Client::read_option::<Cursor<&[u8]>, TestMpdObject, TestMpdObject>(&mut c).await;

            assert_eq!(result, Err(MpdError::Mpd(err)));
        }
    }

    mod ok {
        use crate::mpd::{
            client::Client,
            errors::{ErrorCode, MpdFailureResponse},
        };

        use super::*;

        #[tokio::test]
        async fn parses_correct_response() {
            let buf: &[u8] = b"OK\n";
            let mut c = Cursor::new(buf);

            let result = Client::read_ok(&mut c).await;

            assert_eq!(result, Ok(()));
        }

        #[tokio::test]
        async fn returns_mpd_error() {
            let buf: &[u8] = b"ACK [55@2] {some_cmd} error message boi\n";
            let err = MpdFailureResponse {
                code: ErrorCode::PlayerSync,
                command_list_index: 2,
                command: "some_cmd".to_string(),
                message: "error message boi".to_string(),
            };
            let mut c = Cursor::new(buf);

            let result = Client::read_ok(&mut c).await;

            assert_eq!(result, Err(MpdError::Mpd(err)));
        }

        #[tokio::test]
        async fn returns_error_when_receiving_value() {
            let buf: &[u8] = b"idc\nOK\n";
            let mut c = Cursor::new(buf);

            let result = Client::read_ok(&mut c).await;

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

        #[tokio::test]
        async fn returns_mpd_error() {
            let buf: &[u8] = b"ACK [55@2] {some_cmd} error message boi\n";
            let err = MpdFailureResponse {
                code: ErrorCode::PlayerSync,
                command_list_index: 2,
                command: "some_cmd".to_string(),
                message: "error message boi".to_string(),
            };
            let mut c = Cursor::new(buf);

            let result = Client::read_binary(&mut c, &mut Vec::new()).await;

            assert_eq!(result, Err(MpdError::Mpd(err)));
        }

        #[tokio::test]
        async fn returns_error_when_unknown_receiving_value() {
            let buf: &[u8] = b"idc: value\nOK\n";
            let mut c = Cursor::new(buf);

            let result = Client::read_binary(&mut c, &mut Vec::new()).await;

            assert_eq!(
                result,
                Err(MpdError::Generic(String::from(
                    "Unexpected key when parsing binary response: 'idc'"
                )))
            );
        }

        #[tokio::test]
        async fn returns_error_when_unknown_receiving_unexpected_ok() {
            let buf: &[u8] = b"OK\n";
            let mut c = Cursor::new(buf);

            let result = Client::read_binary(&mut c, &mut Vec::new()).await;

            assert_eq!(
                result,
                Err(MpdError::Generic(String::from("Expected binary data but got 'OK'")))
            );
        }

        #[tokio::test]
        async fn returns_success() {
            let bytes = &[0; 111];
            let buf: &[u8] = b"size: 222\ntype: image/png\nbinary: 111\n";
            let buf_end: &[u8] = b"\nOK\n";
            let mut c = Cursor::new([buf, bytes, buf_end].concat());

            let mut buf = Vec::new();
            let result = Client::read_binary(&mut c, &mut buf).await;

            assert_eq!(buf, bytes);
            assert_eq!(
                result,
                Ok(BinaryMpdResponse {
                    bytes_read: 111,
                    size_total: 222,
                    mime_type: Some("image/png".to_owned())
                })
            );
        }
    }
}
