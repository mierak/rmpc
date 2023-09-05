use crate::mpd::{errors::MpdError, FromMpd, LineHandled};

#[derive(Debug, Default)]
pub struct MpdList(pub Vec<String>);

impl FromMpd for MpdList {
    fn finish(self) -> Result<Self, MpdError> {
        Ok(self)
    }

    fn next_internal(&mut self, _key: &str, value: String) -> Result<LineHandled, MpdError> {
        self.0.push(value);
        Ok(LineHandled::Yes)
    }
}
