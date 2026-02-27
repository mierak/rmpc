use crate::{
    errors::MpdError,
    from_mpd::{FromMpd, LineHandled},
};

#[derive(Default, Debug, Clone, Copy)]
pub struct Update {
    pub job_id: u32,
}

impl FromMpd for Update {
    fn next_internal(&mut self, key: &str, value: String) -> Result<LineHandled, MpdError> {
        match key {
            "updating_db" => self.job_id = value.parse()?,
            _ => return Ok(LineHandled::No { value }),
        }
        Ok(LineHandled::Yes)
    }
}
