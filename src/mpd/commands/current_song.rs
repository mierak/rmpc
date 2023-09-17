use std::time::Duration;

use crate::mpd::{errors::MpdError, FromMpd, LineHandled};

#[derive(Default, PartialEq, Eq, Clone)]
pub struct Song {
    pub id: u32,
    pub file: String,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub duration: Option<Duration>,

    // the other less relevant tags are pushed here
    pub others: Vec<(String, String)>,
    // pub name: Option<String>, // a name for this song. This is not the song title. The exact meaning of this tag is not well-defined. It is often used by badly configured internet radio stations with broken tags to squeeze both the artist name and the song title in one tag.
    // pub artistsort: Option<String>, // same as artist, but for sorting. This usually omits prefixes such as “The”.
    // pub albumsort: Option<String>,  // same as album, but for sorting.
    // pub albumartist: Option<String>, // on multi-artist albums, this is the artist name which shall be used for the whole album. The exact meaning of this tag is not well-defined.
    // pub albumartistsort: Option<String>, // same as albumartist, but for sorting.
    // pub titlesort: Option<String>,   // same as title, but for sorting.
    // pub track: Option<String>,       // the decimal track number within the album.
    // pub genre: Option<String>,       // the music genre.
    // pub mood: Option<String>,        // the mood of the audio with a few keywords.
    // pub date: Option<String>,        // the song’s release date. This is usually a 4-digit year.
    // pub originaldate: Option<String>, // the song’s original release date.
    // pub composer: Option<String>,    // the artist who composed the song.
    // pub composersort: Option<String>, // same as composer, but for sorting.
    // pub performer: Option<String>,   // the artist who performed the song.
    // pub conductor: Option<String>,   // the conductor who conducted the song.
    // pub work: Option<String>, // “a work is a distinct intellectual or artistic creation, which can be expressed in the form of one or more audio recordings”
    // pub ensemble: Option<String>, // the ensemble performing this song, e.g. “Wiener Philharmoniker”.
    // pub movement: Option<String>, // name of the movement, e.g. “Andante con moto”.
    // pub movementnumber: Option<String>, // movement number, e.g. “2” or “II”.
    // pub location: Option<String>, // location of the recording, e.g. “Royal Albert Hall”.
    // pub grouping: Option<String>, // “used if the sound belongs to a larger category of sounds/music” (from the IDv2.4.0 TIT1 description).
    // pub comment: Option<String>, // a human-readable comment about this song. The exact meaning of this tag is not well-defined.
    // pub disc: Option<String>,    // the decimal disc number in a multi-disc album.
    // pub label: Option<String>,   // the name of the label or publisher.
    // pub musicbrainz_artistid: Option<String>, // the artist id in the MusicBrainz database.
    // pub musicbrainz_albumid: Option<String>, // the album id in the MusicBrainz database.
    // pub musicbrainz_albumartistid: Option<String>, // the album artist id in the MusicBrainz database.
    // pub musicbrainz_trackid: Option<String>, // the track id in the MusicBrainz database.
    // pub musicbrainz_releasegroupid: Option<String>, // the release group id in the MusicBrainz database.
    // pub musicbrainz_releasetrackid: Option<String>, // the release track id in the MusicBrainz database.
    // pub musicbrainz_workid: Option<String>, // the work id in the MusicBrainz database.
    // pub last_modified: Option<String>, // ISO 8601
}
impl std::fmt::Debug for Song {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Song {{ file: {}, title: {:?}, artist: {:?}, id: {} }}",
            self.file, self.title, self.artist, self.id
        )
    }
}

impl FromMpd for Song {
    fn finish(self) -> Result<Self, crate::mpd::errors::MpdError> {
        Ok(self)
    }

    fn next_internal(&mut self, key: &str, value: String) -> Result<LineHandled, MpdError> {
        match key {
            "file" => self.file = value,
            "artist" => self.artist = Some(value),
            "album" => self.album = Some(value),
            "title" => self.title = Some(value),
            "id" => self.id = value.parse()?,
            "duration" => {
                self.duration = Some(Duration::from_secs_f64(value.parse()?));
            }
            "time" | "format" => {} // deprecated or ignored
            key => self.others.push((key.to_owned(), value)),
        }
        Ok(LineHandled::Yes)
    }
}
