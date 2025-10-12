use std::fmt::Display;

use anyhow::Result;

#[derive(Debug, PartialEq)]
pub enum MpdError {
    Parse(String),
    UnknownCode(u8),
    Generic(String),
    ClientClosed,
    Mpd(MpdFailureResponse),
    ValueExpected(String),
    UnsupportedMpdVersion(&'static str),
    TimedOut(String),
}

impl std::error::Error for MpdError {}
impl From<std::io::Error> for MpdError {
    fn from(err: std::io::Error) -> Self {
        match err.kind() {
            std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut => {
                MpdError::TimedOut(err.to_string())
            }
            _ => MpdError::Generic(format!("{err}")),
        }
    }
}

impl MpdError {
    pub fn detail_or_display(&self) -> String {
        match self {
            MpdError::Mpd(failure) => failure.message.to_string(),
            _ => self.to_string(),
        }
    }
}

impl Display for MpdError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MpdError::Parse(msg) => write!(f, "ParseError: '{msg}'"),
            MpdError::UnknownCode(code) => {
                write!(f, "UnknownCodeError: '{code}'")
            }
            MpdError::Generic(msg) => write!(f, "GenericError: '{msg}'"),
            MpdError::ClientClosed => {
                write!(f, "Client has been already closed.")
            }
            MpdError::Mpd(err) => write!(f, "MpdError: '{err}'"),
            MpdError::ValueExpected(val) => {
                write!(f, "Expected value from MPD but got '{val}'")
            }
            MpdError::UnsupportedMpdVersion(val) => {
                write!(f, "Unsupported MPD version: '{val}'")
            }
            MpdError::TimedOut(msg) => write!(f, "Reading response from MPD timed out, '{msg}'"),
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum ErrorCode {
    /// not a list
    NotList = 1,
    /// bad command arguments
    Argument = 2,
    /// invalid password
    Password = 3,
    /// insufficient permissions
    Permission = 4,
    /// unknown command
    UnknownCmd = 5,
    /// object doesn't exist
    NoExist = 50,
    /// maximum playlist size exceeded
    PlaylistMax = 51,
    /// general system error
    System = 52,
    /// error loading playlist
    PlaylistLoad = 53,
    /// update database is already in progress
    UpdateAlready = 54,
    /// player synchronization error
    PlayerSync = 55,
    /// object already exists
    Exist = 56,
}

impl Display for ErrorCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", match self {
            ErrorCode::NotList => "not a list",
            ErrorCode::Argument => "bad argument",
            ErrorCode::Password => "invalid password",
            ErrorCode::Permission => "no permission",
            ErrorCode::UnknownCmd => "unknown command",
            ErrorCode::NoExist => "resource does not exist",
            ErrorCode::PlaylistMax => "maximum playlist size",
            ErrorCode::System => "system error",
            ErrorCode::PlaylistLoad => "unable to load playlist",
            ErrorCode::UpdateAlready => "database update already in progress",
            ErrorCode::PlayerSync => "player is in an inconsistent state",
            ErrorCode::Exist => "resource already exists",
        })
    }
}

impl std::str::FromStr for ErrorCode {
    type Err = MpdError;

    fn from_str(s: &str) -> Result<ErrorCode, MpdError> {
        if let Ok(s) = s.parse() {
            match s {
                1 => Ok(Self::NotList),
                2 => Ok(Self::Argument),
                3 => Ok(Self::Password),
                4 => Ok(Self::Permission),
                5 => Ok(Self::UnknownCmd),

                50 => Ok(Self::NoExist),
                51 => Ok(Self::PlaylistMax),
                52 => Ok(Self::System),
                53 => Ok(Self::PlaylistLoad),
                54 => Ok(Self::UpdateAlready),
                55 => Ok(Self::PlayerSync),
                56 => Ok(Self::Exist),

                _ => Err(MpdError::UnknownCode(s)),
            }
        } else {
            Err(MpdError::Parse(s.to_owned()))
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct MpdFailureResponse {
    pub code: ErrorCode,
    pub command_list_index: u8,
    pub command: String,
    pub message: String,
}

impl Display for MpdFailureResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Cannot execute command: '{}'. Detail: '{}'. Reason: '{}'. Cmd idx: '{}'",
            self.command, self.message, self.code, self.command_list_index
        )
    }
}

enum ParseError {
    NoAck,
    NoCode,
    NoCommandIndex,
    InvalidCommandIndex,
    NoCurrentCommand,
}
impl From<anyhow::Error> for MpdError {
    fn from(value: anyhow::Error) -> Self {
        Self::Generic(value.to_string())
    }
}
impl From<std::num::ParseIntError> for MpdError {
    fn from(value: std::num::ParseIntError) -> Self {
        Self::Parse(value.to_string())
    }
}
impl From<std::num::ParseFloatError> for MpdError {
    fn from(value: std::num::ParseFloatError) -> Self {
        Self::Parse(value.to_string())
    }
}

impl From<ParseError> for MpdError {
    fn from(value: ParseError) -> Self {
        let text = match value {
            ParseError::NoAck => "No Ack",
            ParseError::NoCode => "No error code",
            ParseError::NoCommandIndex => "No command index",
            ParseError::InvalidCommandIndex => "Invalid command index",
            ParseError::NoCurrentCommand => "No current command",
        };
        Self::Parse(format!("Invalid error format. {text}."))
    }
}

// ACK [error@command_listNum] {current_command} message_text
impl std::str::FromStr for MpdFailureResponse {
    type Err = MpdError;

    fn from_str(s: &str) -> Result<Self, MpdError> {
        if let Some(rest) = s.strip_prefix("ACK [") {
            if let Some((error_code, rest)) = rest.split_once('@') {
                let error_code: ErrorCode = error_code.parse()?;

                if let Some((command_idx, rest)) = rest.split_once(']') {
                    if let Ok(command_idx) = command_idx.parse() {
                        if let Some(rest) = rest.strip_prefix(" {") {
                            if let Some((command, rest)) = rest.split_once("} ") {
                                let message = rest.trim();

                                Ok(Self {
                                    code: error_code,
                                    command_list_index: command_idx,
                                    command: command.to_owned(),
                                    message: message.to_owned(),
                                })
                            } else {
                                Err(ParseError::NoCurrentCommand.into())
                            }
                        } else {
                            Err(ParseError::NoCurrentCommand.into())
                        }
                    } else {
                        Err(ParseError::InvalidCommandIndex.into())
                    }
                } else {
                    Err(ParseError::NoCommandIndex.into())
                }
            } else {
                Err(ParseError::NoCode.into())
            }
        } else {
            Err(ParseError::NoAck.into())
        }
    }
}
