use std::str::FromStr;

use anyhow::Result;
use tokio::io::{AsyncBufReadExt, AsyncReadExt};
use tracing::trace;

use super::errors::{ErrorCode, MpdError, MpdFailureResponse};

#[derive(Debug, PartialEq)]
pub struct BinaryMpdResponse {
    pub bytes_read: u64,
    pub size_total: u32,
    pub mime_type: Option<String>,
}

impl BinaryMpdResponse {
    pub async fn from_read<R: std::fmt::Debug>(read: &mut R, binary_buf: &mut Vec<u8>) -> Result<Self, MpdError>
    where
        R: tokio::io::AsyncBufRead + Unpin,
    {
        let mut buf = String::new();
        let mut start_byte_read = false;
        let mut size = 0;
        let mut binary = 0;
        let mut mime_type = None;
        while !start_byte_read {
            let bytes_read = read.read_line(&mut buf).await?;
            if bytes_read == 0 {
                return Err(MpdError::ClientClosed);
            }
            if buf.starts_with("ACK") {
                return Err(MpdError::Mpd(MpdFailureResponse::from_str(&buf)?));
            } else if buf.starts_with("OK") && !start_byte_read {
                return Err(MpdError::Mpd(MpdFailureResponse {
                    code: ErrorCode::NoExist,
                    command_list_index: 0,
                    command: "".to_owned(),
                    message: "Empty binary response".to_owned(),
                }));
            }
            match buf.split_once(": ") {
                Some((key, mut val)) => {
                    val = val.trim();
                    match key {
                        "size" => {
                            size = if let Ok(val) = val.parse() {
                                val
                            } else {
                                return Err(MpdError::Parse(format!("Expected a digit for size, got: '{}'", val)));
                            }
                        }
                        "binary" => {
                            binary = val.parse().unwrap();
                            start_byte_read = true;
                        }
                        "type" => mime_type = Some(val.to_owned()),
                        key => {
                            return Err(MpdError::Generic(format!(
                                "Unexpected key when parsing binary response: '{key}'"
                            )))
                        }
                    }
                    buf.clear();
                }
                None => return Err(MpdError::Parse(format!("Expected split to succeed, got: '{}'", buf))),
            };
        }

        let mut handle = read.take(binary);
        let _ = handle.read_to_end(binary_buf).await?;
        let _ = read.read_line(&mut buf).await; // MPD prints an empty new line at the end of binary response
        buf.clear();
        let _ = read.read_line(&mut buf).await; // OK
        if buf.starts_with("OK") {
            Ok(Self {
                bytes_read: binary,
                size_total: size,
                mime_type,
            })
        } else {
            Err(MpdError::Generic(format!("Read ended with error: '{}'", buf)))
        }
    }
}

#[derive(Debug)]
pub struct EmptyMpdResponse;

impl EmptyMpdResponse {
    pub async fn is_ok<R: std::fmt::Debug>(read: &mut R) -> Result<(), MpdError>
    where
        R: tokio::io::AsyncBufRead + Unpin,
    {
        let mut buf = String::new();
        trace!(message = "Reading command");
        let bytes_read = read.read_line(&mut buf).await?;
        if bytes_read == 0 {
            return Err(MpdError::ClientClosed);
        }

        if buf.starts_with("OK") || buf.starts_with("list_OK") {
            return Ok(());
        }

        if buf.starts_with("ACK") {
            return Err(MpdError::Mpd(MpdFailureResponse::from_str(&buf)?));
        }

        Err(MpdError::Generic(format!("Too deep daddy: '{buf}'")))
    }
}

#[derive(Debug, PartialEq)]
pub struct MpdResponse<T: std::str::FromStr> {
    pub body: Option<T>,
}

impl<T> MpdResponse<T>
where
    T: std::str::FromStr + std::fmt::Debug,
    <T as FromStr>::Err: std::fmt::Debug,
{
    pub async fn from_read<R>(read: &mut R) -> Result<Self, MpdError>
    where
        R: tokio::io::AsyncBufRead + Unpin,
    {
        trace!(message = "Reading command");
        let mut buf = String::new();
        let mut result = String::new();
        loop {
            match read.read_line(&mut buf).await {
                Ok(0) => return Err(MpdError::ClientClosed),
                Ok(_) if buf.starts_with("ACK") => return Err(MpdError::Mpd(MpdFailureResponse::from_str(&buf)?)),
                Ok(_) if buf == "OK\n" || buf == "list_OK\n" => break,
                Ok(_) => {
                    result.push_str(&std::mem::take(&mut buf));
                }
                Err(e) => return Err(e.into()),
            };
        }

        if result.trim().is_empty() {
            Ok(Self { body: None })
        } else {
            match result.parse::<T>() {
                Ok(body) => Ok(Self { body: Some(body) }),
                Err(err) => Err(MpdError::Parse(format!(
                    "cannot parse '{result}', nested error: '{err:?}'"
                ))),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use crate::mpd::{
        errors::{ErrorCode, MpdError, MpdFailureResponse},
        response::{EmptyMpdResponse, MpdResponse},
    };

    mod response {
        use crate::mpd::commands::Volume;

        use super::*;

        #[tokio::test]
        async fn parses_correct_response() {
            let buf: &[u8] = b"volume 50\nOK\n";
            let mut c = Cursor::new(buf);

            let result = MpdResponse::<Volume>::from_read(&mut c).await;

            assert_eq!(
                result,
                Ok(MpdResponse {
                    body: Some(Volume::new(50))
                })
            );
        }

        #[tokio::test]
        async fn returns_parse_error() {
            let buf: &[u8] = b"lol\nOK\n";
            let mut c = Cursor::new(buf);

            let result = MpdResponse::<Volume>::from_read(&mut c).await;

            assert_eq!(result, Err(MpdError::Parse("cannot parse 'lol\n', nested error: 'Invalid value 'lol\n' when parsing Volume - split', command: '".to_string())));
        }

        #[tokio::test]
        async fn parses_empty_response() {
            let buf: &[u8] = b"OK\n";
            let mut c = Cursor::new(buf);

            let result = MpdResponse::<Volume>::from_read(&mut c).await;

            assert_eq!(result, Ok(MpdResponse { body: None }));
        }
        #[tokio::test]
        async fn detects_closed_client() {
            let buf: &[u8] = b"";
            let mut c = Cursor::new(buf);

            let result = MpdResponse::<Volume>::from_read(&mut c).await;

            assert_eq!(result, Err(MpdError::ClientClosed));
        }

        #[tokio::test]
        async fn parses_mpd_error() {
            let buf: &[u8] = b"ACK [55@2] {some_cmd} error message boi";
            let mut c = Cursor::new(buf);

            let result = MpdResponse::<Volume>::from_read(&mut c).await;

            assert_eq!(
                result,
                Err(MpdError::Mpd(MpdFailureResponse {
                    code: ErrorCode::PlayerSync,
                    command_list_index: 2,
                    command: "some_cmd".to_string(),
                    message: "error message boi".to_string(),
                }))
            );
        }
    }

    mod empty {
        use super::*;

        #[tokio::test]
        async fn parses_ok() {
            let buf: &[u8] = b"OK\n";
            let mut c = Cursor::new(buf);

            let result = EmptyMpdResponse::is_ok(&mut c).await;

            assert_eq!(result, Ok(()));
        }

        #[tokio::test]
        async fn parses_list_ok() {
            let buf: &[u8] = b"list_OK\n";
            let mut c = Cursor::new(buf);

            let result = EmptyMpdResponse::is_ok(&mut c).await;

            assert_eq!(result, Ok(()));
        }

        #[tokio::test]
        async fn detects_closed_client() {
            let buf: &[u8] = b"";
            let mut c = Cursor::new(buf);

            let result = EmptyMpdResponse::is_ok(&mut c).await;

            assert_eq!(result, Err(MpdError::ClientClosed));
        }

        #[tokio::test]
        async fn parses_mpd_error() {
            let buf: &[u8] = b"ACK [55@2] {some_cmd} error message boi";
            let mut c = Cursor::new(buf);

            let result = EmptyMpdResponse::is_ok(&mut c).await;

            assert_eq!(
                result,
                Err(MpdError::Mpd(MpdFailureResponse {
                    code: ErrorCode::PlayerSync,
                    command_list_index: 2,
                    command: "some_cmd".to_string(),
                    message: "error message boi".to_string(),
                }))
            );
        }

        #[tokio::test]
        async fn returns_generic_error_as_fallback() {
            let buf: &[u8] = b"lolno";
            let mut c = Cursor::new(buf);

            let result = EmptyMpdResponse::is_ok(&mut c).await;

            assert_eq!(result, Err(MpdError::Generic("Too deep daddy: 'lolno'".to_string())));
        }
    }

    mod binary {
        use std::io::Cursor;

        use crate::mpd::{
            errors::{ErrorCode, MpdError, MpdFailureResponse},
            response::BinaryMpdResponse,
        };

        #[tokio::test]
        async fn empty_successful_response_returns_not_exists() {
            let buf: &[u8] = b"OK\n";
            let mut c = Cursor::new(buf);

            let mut buf = Vec::new();
            let result = BinaryMpdResponse::from_read(&mut c, &mut buf).await;

            assert_eq!(
                result,
                Err(MpdError::Mpd(MpdFailureResponse {
                    code: ErrorCode::NoExist,
                    command_list_index: 0,
                    command: "".to_owned(),
                    message: "Empty binary response".to_owned()
                }))
            );
        }

        #[tokio::test]
        async fn returns_empty_error() {
            let buf: &[u8] = b"ACK [55@2] {some_cmd} error message boi";
            let mut c = Cursor::new(buf);

            let mut buf = Vec::new();
            let result = BinaryMpdResponse::from_read(&mut c, &mut buf).await;

            assert_eq!(
                result,
                Err(MpdError::Mpd(MpdFailureResponse {
                    code: ErrorCode::PlayerSync,
                    command_list_index: 2,
                    command: "some_cmd".to_owned(),
                    message: "error message boi".to_owned()
                }))
            );
        }

        #[tokio::test]
        async fn returns_client_closed_when_unexpectedly_read_zero_bytes() {
            let buf: &[u8] = b"size: 111\n";
            let mut c = Cursor::new(buf);

            let mut buf = Vec::new();
            let result = BinaryMpdResponse::from_read(&mut c, &mut buf).await;

            assert_eq!(result, Err(MpdError::ClientClosed));
        }

        #[tokio::test]
        async fn returns_success() {
            let bytes = &[0; 111];
            let buf: &[u8] = b"size: 222\ntype: image/png\nbinary: 111\n";
            let buf_end: &[u8] = b"\nOK\n";
            let mut c = Cursor::new([buf, bytes, buf_end].concat());

            let mut buf = Vec::new();
            let result = BinaryMpdResponse::from_read(&mut c, &mut buf).await;

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
