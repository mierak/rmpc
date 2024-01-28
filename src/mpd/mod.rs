use tracing::instrument;

use self::errors::MpdError;

pub mod client;
pub mod commands;
pub mod errors;
pub mod mpd_client;
pub mod proto_client;
pub mod version;

trait FromMpd
where
    Self: std::marker::Sized,
{
    fn next_internal(&mut self, key: &str, value: String) -> Result<LineHandled, MpdError>;

    #[instrument(skip(self))]
    fn next(&mut self, line: String) -> Result<(), MpdError> {
        let (key, value) = split_line(line)?;
        match self.next_internal(key.to_lowercase().as_str(), value)? {
            LineHandled::Yes => {}
            LineHandled::No { value } => tracing::warn!(message = "Encountered unknow key/value pair", key, value),
        }
        Ok(())
    }
}

#[instrument]
pub(self) fn split_line(mut line: String) -> Result<(String, String), MpdError> {
    let delim_idx = match line.find(':') {
        Some(val) => val,
        None => return Err(MpdError::ValueExpected(line)),
    };
    let mut value = line.split_off(delim_idx);
    value.drain(..':'.len_utf8() + ' '.len_utf8());

    Ok((line, value))
}

enum LineHandled {
    Yes,
    No { value: String },
}
