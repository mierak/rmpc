use rmpc_mpd::commands::{Song, Status};

pub struct Ctx {
    pub current_song: Option<Song>,
    pub status: Status,
    pub queue: Vec<Song>,
    pub album_art: Option<Vec<u8>>,
    pub last_written_album_art_song_uri: Option<String>,
}
