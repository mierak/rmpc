use crate::mpd::{FromMpd, LineHandled};

#[derive(Debug, Default)]
pub struct FileList(pub Vec<String>);

impl FromMpd for FileList {
    fn finish(self) -> Result<Self, crate::mpd::errors::MpdError> {
        Ok(self)
    }

    fn next_internal(
        &mut self,
        key: &str,
        value: String,
    ) -> Result<crate::mpd::LineHandled, crate::mpd::errors::MpdError> {
        match key {
            "file" => {
                self.0.push(value);
                Ok(LineHandled::Yes)
            }
            _ => return Ok(LineHandled::No { value }),
        }
    }
}
