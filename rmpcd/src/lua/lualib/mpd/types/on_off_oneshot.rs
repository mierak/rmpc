use rmpc_mpd::commands::status::OnOffOneshot as MpdOnOffOneShot;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Copy, Clone)]
#[serde(rename_all = "lowercase")]
pub enum OnOffOneshot {
    On,
    Off,
    Oneshot,
}

impl From<OnOffOneshot> for MpdOnOffOneShot {
    fn from(value: OnOffOneshot) -> Self {
        match value {
            OnOffOneshot::On => MpdOnOffOneShot::On,
            OnOffOneshot::Off => MpdOnOffOneShot::Off,
            OnOffOneshot::Oneshot => MpdOnOffOneShot::Oneshot,
        }
    }
}

impl From<MpdOnOffOneShot> for OnOffOneshot {
    fn from(value: MpdOnOffOneShot) -> Self {
        match value {
            MpdOnOffOneShot::On => OnOffOneshot::On,
            MpdOnOffOneShot::Off => OnOffOneshot::Off,
            MpdOnOffOneShot::Oneshot => OnOffOneshot::Oneshot,
        }
    }
}
