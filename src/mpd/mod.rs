use self::errors::MpdError;

pub mod client;
pub mod commands;
pub mod errors;
pub mod mpd_client;
pub mod proto_client;
pub mod version;

pub(crate) trait FromMpd
where
    Self: std::marker::Sized,
{
    fn next_internal(&mut self, key: &str, value: String) -> Result<LineHandled, MpdError>;

    fn next(&mut self, line: String) -> Result<(), MpdError> {
        let (key, value) = split_line(line)?;
        match self.next_internal(key.to_lowercase().as_str(), value)? {
            LineHandled::Yes => {}
            LineHandled::No { value } => {
                log::warn!(key = key.as_str(), value = value.as_str(); "Encountered unknown key/value pair");
            }
        }
        Ok(())
    }
}

pub fn split_line(mut line: String) -> Result<(String, String), MpdError> {
    let delim_idx = match line.find(':') {
        Some(val) => val,
        None => return Err(MpdError::ValueExpected(line)),
    };
    let mut value = line.split_off(delim_idx);
    value.drain(..':'.len_utf8() + ' '.len_utf8());

    Ok((line, value))
}

pub(crate) enum LineHandled {
    Yes,
    No { value: String },
}

pub trait ParseErrorExt {
    fn logerr(self, key: &str, value: &str) -> Self;
}

impl<T: std::str::FromStr> ParseErrorExt for Result<T, anyhow::Error> {
    fn logerr(self, key: &str, value: &str) -> Self {
        if self.is_err() {
            log::error!(key, value; "Failed to parse value");
        }
        self
    }
}
impl<T: std::str::FromStr> ParseErrorExt for Result<T, std::num::ParseFloatError> {
    fn logerr(self, key: &str, value: &str) -> Self {
        if self.is_err() {
            log::error!(key, value; "Failed to parse value");
        }
        self
    }
}
impl<T: std::str::FromStr> ParseErrorExt for Result<T, std::num::ParseIntError> {
    fn logerr(self, key: &str, value: &str) -> Self {
        if self.is_err() {
            log::error!(key, value; "Failed to parse value");
        }
        self
    }
}
