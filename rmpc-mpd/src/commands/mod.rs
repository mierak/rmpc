pub mod current_song;
pub mod decoders;
pub mod idle;
pub mod list;
pub mod list_all;
pub mod list_files;
pub mod list_mounts;
pub mod list_playlist;
pub mod list_playlists;
pub mod lsinfo;
pub mod messages;
pub mod metadata_tag;
pub mod mpd_config;
pub mod outputs;
pub mod playlist_info;
pub mod status;
pub mod stickers;
pub mod update;
pub mod volume;

pub use self::{
    current_song::Song,
    decoders::Decoder,
    idle::IdleEvent,
    list_files::ListFiles,
    list_mounts::Mounts,
    list_playlists::Playlist,
    lsinfo::LsInfo,
    status::{State, Status},
    update::Update,
    volume::Volume,
};

/// Parses MPD's audio format string into (samplerate, bits, channels).
///
/// MPD emits "samplerate:bits:channels" where bits can also be "f" (float),
/// any field can be "*" (unknown), and DSD is special-cased: "dsdNN:channels"
/// where NN * 44100 is the DSD sample rate, or "samplerate:dsd:channels" for
/// rates not divisible by 44100, where samplerate is MPD's internal byte rate
/// (one eighth of the DSD sample rate). DSD is one bit per sample.
pub(crate) fn parse_audio_format(audio: &str) -> (Option<u32>, Option<u32>, Option<u32>) {
    let mut fields = audio.split(':');
    let first = fields.next();
    let second = fields.next();
    let third = fields.next();

    if let Some(multiple) = first.and_then(|v| v.strip_prefix("dsd")) {
        let samplerate = multiple.parse::<u32>().ok().and_then(|v| v.checked_mul(44100));
        return (samplerate, samplerate.map(|_| 1), second.and_then(|v| v.parse().ok()));
    }

    let samplerate = first.and_then(|v| v.parse::<u32>().ok());
    let channels = third.and_then(|v| v.parse().ok());
    if second == Some("dsd") {
        // The dsd marker alone proves 1 bit per sample, even when the rate
        // is unknown ("*:dsd:2").
        return (samplerate.and_then(|v| v.checked_mul(8)), Some(1), channels);
    }

    (samplerate, second.and_then(|v| v.parse().ok()), channels)
}
