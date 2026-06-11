use std::collections::HashMap;

use derive_more::{AsMut, AsRef, Into, IntoIterator};

use crate::{
    errors::MpdError,
    from_mpd::{FromMpd, LineHandled},
};

#[derive(Debug, Default, IntoIterator, AsRef, AsMut, Into)]
pub struct MpdList(pub Vec<String>);

impl From<Vec<String>> for MpdList {
    fn from(value: Vec<String>) -> Self {
        MpdList(value)
    }
}

impl FromMpd for MpdList {
    fn next_internal(&mut self, _key: &str, value: String) -> Result<LineHandled, MpdError> {
        self.0.push(value);
        Ok(LineHandled::Yes)
    }
}

#[derive(Debug, Default, Clone)]
pub struct MpdGroupedList(pub Vec<HashMap<String, String>>);

#[derive(Default)]
pub(crate) struct RawGroupedPairs(pub Vec<(String, String)>);

impl FromMpd for RawGroupedPairs {
    fn next_internal(&mut self, key: &str, value: String) -> Result<LineHandled, MpdError> {
        self.0.push((key.to_string(), value));
        Ok(LineHandled::Yes)
    }
}

impl RawGroupedPairs {
    pub(crate) fn into_grouped_list(self, primary: &str) -> MpdGroupedList {
        let primary = primary.to_lowercase();
        let mut context = HashMap::new();
        let mut records = Vec::new();
        for (key, value) in self.0 {
            if key == primary {
                let mut record = context.clone();
                record.insert(key, value);
                records.push(record);
            } else {
                context.insert(key, value);
            }
        }
        MpdGroupedList(records)
    }
}
