use std::path::PathBuf;

use rmpc_mpd::commands::{Song, Status};

pub const ALBUM_ART_CACHE_DIR: &str = "albumart";

pub struct Ctx {
    pub current_song: Option<Song>,
    pub status: Status,
    pub queue: Vec<Song>,
    pub album_art: Option<Vec<u8>>,
    pub last_written_album_art_song_uri: Option<String>,
    pub cache_dir: PathBuf,
}
