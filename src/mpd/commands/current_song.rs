use anyhow::anyhow;
use anyhow::Context;

pub const COMMAND: &[u8; 11] = b"currentsong";

#[derive(Default)]
pub struct Song {
    pub file: String,
    pub artist: Option<String>, // the artist name. Its meaning is not well-defined; see “composer” and “performer” for more specific tags.
    pub artistsort: Option<String>, // same as artist, but for sorting. This usually omits prefixes such as “The”.
    pub album: Option<String>,  // the album name.
    pub albumsort: Option<String>, // same as album, but for sorting.
    pub albumartist: Option<String>, // on multi-artist albums, this is the artist name which shall be used for the whole album. The exact meaning of this tag is not well-defined.
    pub albumartistsort: Option<String>, // same as albumartist, but for sorting.
    pub title: Option<String>,       // the song title.
    pub titlesort: Option<String>,   // same as title, but for sorting.
    pub track: Option<String>,       // the decimal track number within the album.
    pub name: Option<String>, // a name for this song. This is not the song title. The exact meaning of this tag is not well-defined. It is often used by badly configured internet radio stations with broken tags to squeeze both the artist name and the song title in one tag.
    pub genre: Option<String>, // the music genre.
    pub mood: Option<String>, // the mood of the audio with a few keywords.
    pub date: Option<String>, // the song’s release date. This is usually a 4-digit year.
    pub originaldate: Option<String>, // the song’s original release date.
    pub composer: Option<String>, // the artist who composed the song.
    pub composersort: Option<String>, // same as composer, but for sorting.
    pub performer: Option<String>, // the artist who performed the song.
    pub conductor: Option<String>, // the conductor who conducted the song.
    pub work: Option<String>, // “a work is a distinct intellectual or artistic creation, which can be expressed in the form of one or more audio recordings”
    pub ensemble: Option<String>, // the ensemble performing this song, e.g. “Wiener Philharmoniker”.
    pub movement: Option<String>, // name of the movement, e.g. “Andante con moto”.
    pub movementnumber: Option<String>, // movement number, e.g. “2” or “II”.
    pub location: Option<String>, // location of the recording, e.g. “Royal Albert Hall”.
    pub grouping: Option<String>, // “used if the sound belongs to a larger category of sounds/music” (from the IDv2.4.0 TIT1 description).
    pub comment: Option<String>, // a human-readable comment about this song. The exact meaning of this tag is not well-defined.
    pub disc: Option<String>,    // the decimal disc number in a multi-disc album.
    pub label: Option<String>,   // the name of the label or publisher.
    pub musicbrainz_artistid: Option<String>, // the artist id in the MusicBrainz database.
    pub musicbrainz_albumid: Option<String>, // the album id in the MusicBrainz database.
    pub musicbrainz_albumartistid: Option<String>, // the album artist id in the MusicBrainz database.
    pub musicbrainz_trackid: Option<String>, // the track id in the MusicBrainz database.
    pub musicbrainz_releasegroupid: Option<String>, // the release group id in the MusicBrainz database.
    pub musicbrainz_releasetrackid: Option<String>, // the release track id in the MusicBrainz database.
    pub musicbrainz_workid: Option<String>, // the work id in the MusicBrainz database.
    pub last_modified: Option<String>, // ISO 8601
    pub pos: u32,                // Duration of the current song in seconds.

    // TODO remove, only playlistinfo
    pub id: u32,
    pub duration: Option<f32>,           // Duration of the current song in seconds.
    pub duration_string: Option<String>, // Duration of the current song in seconds.

    // Info state added by app, not MPD
    pub selected: bool, // the work id in the MusicBrainz database.
}
impl std::fmt::Debug for Song {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Song {{ file: {}, id: {}, selected: {}, pos: {} }}",
            self.file, self.id, self.selected, self.pos
        )
    }
}

impl std::str::FromStr for Song {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut resunt = Song::default();

        for line in s.lines() {
            let (key, value) = line
                .split_once(": ")
                .context(anyhow!("Invalid value '{}' whe parsing Song", line))?;
            match key.to_lowercase().as_str() {
                "file" => resunt.file = value.to_owned(),
                "artist" => resunt.artist = Some(value.to_owned()),
                "artistsort" => resunt.artistsort = Some(value.to_owned()),
                "album" => resunt.album = Some(value.to_owned()),
                "albumsort" => resunt.albumsort = Some(value.to_owned()),
                "albumartist" => resunt.albumartist = Some(value.to_owned()),
                "albumartistsort" => resunt.albumartistsort = Some(value.to_owned()),
                "title" => resunt.title = Some(value.to_owned()),
                "titlesort" => resunt.titlesort = Some(value.to_owned()),
                "track" => resunt.track = Some(value.to_owned()),
                "name" => resunt.name = Some(value.to_owned()),
                "genre" => resunt.genre = Some(value.to_owned()),
                "mood" => resunt.mood = Some(value.to_owned()),
                "date" => resunt.date = Some(value.to_owned()),
                "originaldate" => resunt.originaldate = Some(value.to_owned()),
                "composer" => resunt.composer = Some(value.to_owned()),
                "composersort" => resunt.composersort = Some(value.to_owned()),
                "performer" => resunt.performer = Some(value.to_owned()),
                "conductor" => resunt.conductor = Some(value.to_owned()),
                "work" => resunt.work = Some(value.to_owned()),
                "ensemble" => resunt.ensemble = Some(value.to_owned()),
                "movement" => resunt.movement = Some(value.to_owned()),
                "movementnumber" => resunt.movementnumber = Some(value.to_owned()),
                "location" => resunt.location = Some(value.to_owned()),
                "grouping" => resunt.grouping = Some(value.to_owned()),
                "comment" => resunt.comment = Some(value.to_owned()),
                "disc" => resunt.disc = Some(value.to_owned()),
                "label" => resunt.label = Some(value.to_owned()),
                "pos" => resunt.pos = value.parse()?,
                "last-modified" => resunt.label = Some(value.to_owned()),
                "musicbrainz_artistid" => resunt.musicbrainz_artistid = Some(value.to_owned()),
                "musicbrainz_albumid" => resunt.musicbrainz_albumid = Some(value.to_owned()),
                "musicbrainz_albumartistid" => resunt.musicbrainz_albumartistid = Some(value.to_owned()),
                "musicbrainz_trackid" => resunt.musicbrainz_trackid = Some(value.to_owned()),
                "musicbrainz_releasegroupid" => resunt.musicbrainz_releasegroupid = Some(value.to_owned()),
                "musicbrainz_releasetrackid" => resunt.musicbrainz_releasetrackid = Some(value.to_owned()),
                "musicbrainz_workid" => resunt.musicbrainz_workid = Some(value.to_owned()),
                "id" => resunt.id = value.parse()?,
                "duration" => {
                    resunt.duration = Some(value.parse()?);
                    resunt.duration_string = Some(value.to_owned());
                }
                "time" => {}   // deprecated
                "format" => {} // ignored
                key => tracing::warn!(
                    message = "Encountered unknow key/value pair while parsing 'listfiles' command",
                    key,
                    value
                ),
            }
        }

        Ok(resunt)
    }
}
