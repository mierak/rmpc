use rmpc_mpd::commands::{Song as MpdSong, metadata_tag::MetadataTag};
use serde::Serialize;

#[serde_with::skip_serializing_none]
#[derive(Serialize, PartialEq, Eq)]
pub struct Song {
    pub file: String,
    pub artist: Option<MetadataTag>,
    pub artistsort: Option<MetadataTag>,
    pub album: Option<MetadataTag>,
    pub albumsort: Option<MetadataTag>,
    pub albumartist: Option<MetadataTag>,
    pub albumartistsort: Option<MetadataTag>,
    pub title: Option<MetadataTag>,
    pub titlesort: Option<MetadataTag>,
    pub track: Option<MetadataTag>,
    pub name: Option<MetadataTag>,
    pub genre: Option<MetadataTag>,
    pub mood: Option<MetadataTag>,
    pub date: Option<MetadataTag>,
    pub originaldate: Option<MetadataTag>,
    pub composer: Option<MetadataTag>,
    pub composersort: Option<MetadataTag>,
    pub performer: Option<MetadataTag>,
    pub conductor: Option<MetadataTag>,
    pub work: Option<MetadataTag>,
    pub ensemble: Option<MetadataTag>,
    pub movement: Option<MetadataTag>,
    pub movementnumber: Option<MetadataTag>,
    pub showmovement: Option<bool>,
    pub location: Option<MetadataTag>,
    pub grouping: Option<MetadataTag>,
    pub comment: Option<MetadataTag>,
    pub disc: Option<MetadataTag>,
    pub label: Option<MetadataTag>,
    pub musicbrainz_artistid: Option<MetadataTag>,
    pub musicbrainz_albumid: Option<MetadataTag>,
    pub musicbrainz_albumartistid: Option<MetadataTag>,
    pub musicbrainz_trackid: Option<MetadataTag>,
    pub musicbrainz_releasegroupid: Option<MetadataTag>,
    pub musicbrainz_releasetrackid: Option<MetadataTag>,
    pub musicbrainz_workid: Option<MetadataTag>,
}

impl From<&MpdSong> for Song {
    fn from(value: &MpdSong) -> Self {
        Self {
            file: value.file.clone(),
            artist: value.metadata.get("artist").cloned(),
            artistsort: value.metadata.get("artistsort").cloned(),
            album: value.metadata.get("album").cloned(),
            albumsort: value.metadata.get("albumsort").cloned(),
            albumartist: value.metadata.get("albumartist").cloned(),
            albumartistsort: value.metadata.get("albumartistsort").cloned(),
            title: value.metadata.get("title").cloned(),
            titlesort: value.metadata.get("titlesort").cloned(),
            track: value.metadata.get("track").cloned(),
            name: value.metadata.get("name").cloned(),
            genre: value.metadata.get("genre").cloned(),
            mood: value.metadata.get("mood").cloned(),
            date: value.metadata.get("date").cloned(),
            originaldate: value.metadata.get("originaldate").cloned(),
            composer: value.metadata.get("composer").cloned(),
            composersort: value.metadata.get("composersort").cloned(),
            performer: value.metadata.get("performer").cloned(),
            conductor: value.metadata.get("conductor").cloned(),
            work: value.metadata.get("work").cloned(),
            ensemble: value.metadata.get("ensemble").cloned(),
            movement: value.metadata.get("movement").cloned(),
            movementnumber: value.metadata.get("movementnumber").cloned(),
            showmovement: value
                .metadata
                .get("showmovement")
                .and_then(|v| v.first().parse::<bool>().ok()),
            location: value.metadata.get("location").cloned(),
            grouping: value.metadata.get("grouping").cloned(),
            comment: value.metadata.get("comment").cloned(),
            disc: value.metadata.get("disc").cloned(),
            label: value.metadata.get("label").cloned(),
            musicbrainz_artistid: value.metadata.get("musicbrainz_artistid").cloned(),
            musicbrainz_albumid: value.metadata.get("musicbrainz_albumid").cloned(),
            musicbrainz_albumartistid: value.metadata.get("musicbrainz_albumartistid").cloned(),
            musicbrainz_trackid: value.metadata.get("musicbrainz_trackid").cloned(),
            musicbrainz_releasegroupid: value.metadata.get("musicbrainz_releasegroupid").cloned(),
            musicbrainz_releasetrackid: value.metadata.get("musicbrainz_releasetrackid").cloned(),
            musicbrainz_workid: value.metadata.get("musicbrainz_workid").cloned(),
        }
    }
}
