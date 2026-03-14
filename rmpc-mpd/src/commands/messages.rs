use serde::Serialize;

use crate::{
    errors::MpdError,
    from_mpd::{FromMpd, LineHandled},
};

#[derive(Debug, Clone, Default, Serialize)]
pub struct Messages(pub Vec<(String, Vec<String>)>);

impl FromMpd for Messages {
    fn next_internal(&mut self, key: &str, value: String) -> Result<LineHandled, MpdError> {
        match key {
            "channel" => {
                self.0.push((value, Vec::new()));
            }
            "message" => {
                if let Some((_, messages)) = self.0.last_mut() {
                    messages.push(value);
                } else {
                    return Err(MpdError::Parse(format!("Message without channel: {value}")));
                }
            }
            _ => return Ok(LineHandled::No { value }),
        }
        Ok(LineHandled::Yes)
    }
}
