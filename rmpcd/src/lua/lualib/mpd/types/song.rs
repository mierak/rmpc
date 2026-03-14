use mlua::UserData;
use rmpc_mpd::commands::Song as MpdSong;

use crate::lua::lualib::mpd::types::{MetadataTag, MetadataTagExt};

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Song {
    pub file: String,
    pub duration: u128,
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
            duration: value.duration.map(|d| d.as_millis()).unwrap_or_default(),
            artist: value.metadata.get("artist").to_metadata_tag(),
            artistsort: value.metadata.get("artistsort").to_metadata_tag(),
            album: value.metadata.get("album").to_metadata_tag(),
            albumsort: value.metadata.get("albumsort").to_metadata_tag(),
            albumartist: value.metadata.get("albumartist").to_metadata_tag(),
            albumartistsort: value.metadata.get("albumartistsort").to_metadata_tag(),
            title: value.metadata.get("title").to_metadata_tag(),
            titlesort: value.metadata.get("titlesort").to_metadata_tag(),
            track: value.metadata.get("track").to_metadata_tag(),
            name: value.metadata.get("name").to_metadata_tag(),
            genre: value.metadata.get("genre").to_metadata_tag(),
            mood: value.metadata.get("mood").to_metadata_tag(),
            date: value.metadata.get("date").to_metadata_tag(),
            originaldate: value.metadata.get("originaldate").to_metadata_tag(),
            composer: value.metadata.get("composer").to_metadata_tag(),
            composersort: value.metadata.get("composersort").to_metadata_tag(),
            performer: value.metadata.get("performer").to_metadata_tag(),
            conductor: value.metadata.get("conductor").to_metadata_tag(),
            work: value.metadata.get("work").to_metadata_tag(),
            ensemble: value.metadata.get("ensemble").to_metadata_tag(),
            movement: value.metadata.get("movement").to_metadata_tag(),
            movementnumber: value.metadata.get("movementnumber").to_metadata_tag(),
            showmovement: value
                .metadata
                .get("showmovement")
                .and_then(|v| v.first().parse::<bool>().ok()),
            location: value.metadata.get("location").to_metadata_tag(),
            grouping: value.metadata.get("grouping").to_metadata_tag(),
            comment: value.metadata.get("comment").to_metadata_tag(),
            disc: value.metadata.get("disc").to_metadata_tag(),
            label: value.metadata.get("label").to_metadata_tag(),
            musicbrainz_artistid: value.metadata.get("musicbrainz_artistid").to_metadata_tag(),
            musicbrainz_albumid: value.metadata.get("musicbrainz_albumid").to_metadata_tag(),
            musicbrainz_albumartistid: value
                .metadata
                .get("musicbrainz_albumartistid")
                .to_metadata_tag(),
            musicbrainz_trackid: value.metadata.get("musicbrainz_trackid").to_metadata_tag(),
            musicbrainz_releasegroupid: value
                .metadata
                .get("musicbrainz_releasegroupid")
                .to_metadata_tag(),
            musicbrainz_releasetrackid: value
                .metadata
                .get("musicbrainz_releasetrackid")
                .to_metadata_tag(),
            musicbrainz_workid: value.metadata.get("musicbrainz_workid").to_metadata_tag(),
        }
    }
}

impl UserData for Song {
    fn add_fields<F: mlua::UserDataFields<Self>>(fields: &mut F) {
        fields.add_field_method_get("file", |_, this| Ok(this.file.clone()));
        fields.add_field_method_get("duration", |_, this| Ok(this.duration));
        fields.add_field_method_get("artist", |_, this| Ok(this.artist.clone()));
        fields.add_field_method_get("artist_sort", |_, this| Ok(this.artistsort.clone()));
        fields.add_field_method_get("album", |_, this| Ok(this.album.clone()));
        fields.add_field_method_get("album_sort", |_, this| Ok(this.albumsort.clone()));
        fields.add_field_method_get("album_artist", |_, this| Ok(this.albumartist.clone()));
        fields
            .add_field_method_get("album_artist_sort", |_, this| Ok(this.albumartistsort.clone()));
        fields.add_field_method_get("title", |_, this| Ok(this.title.clone()));
        fields.add_field_method_get("title_sort", |_, this| Ok(this.titlesort.clone()));
        fields.add_field_method_get("track", |_, this| Ok(this.track.clone()));
        fields.add_field_method_get("name", |_, this| Ok(this.name.clone()));
        fields.add_field_method_get("genre", |_, this| Ok(this.genre.clone()));
        fields.add_field_method_get("mood", |_, this| Ok(this.mood.clone()));
        fields.add_field_method_get("date", |_, this| Ok(this.date.clone()));
        fields.add_field_method_get("original_date", |_, this| Ok(this.originaldate.clone()));
        fields.add_field_method_get("composer", |_, this| Ok(this.composer.clone()));
        fields.add_field_method_get("composer_sort", |_, this| Ok(this.composersort.clone()));
        fields.add_field_method_get("performer", |_, this| Ok(this.performer.clone()));
        fields.add_field_method_get("conductor", |_, this| Ok(this.conductor.clone()));
        fields.add_field_method_get("work", |_, this| Ok(this.work.clone()));
        fields.add_field_method_get("ensemble", |_, this| Ok(this.ensemble.clone()));
        fields.add_field_method_get("movement", |_, this| Ok(this.movement.clone()));
        fields.add_field_method_get("movement_number", |_, this| Ok(this.movementnumber.clone()));
        fields.add_field_method_get("show_movement", |_, this| Ok(this.showmovement));
        fields.add_field_method_get("location", |_, this| Ok(this.location.clone()));
        fields.add_field_method_get("grouping", |_, this| Ok(this.grouping.clone()));
        fields.add_field_method_get("comment", |_, this| Ok(this.comment.clone()));
        fields.add_field_method_get("disc", |_, this| Ok(this.disc.clone()));
        fields.add_field_method_get("label", |_, this| Ok(this.label.clone()));
        fields.add_field_method_get("musicbrainz_artist_id", |_, this| {
            Ok(this.musicbrainz_artistid.clone())
        });
        fields.add_field_method_get("musicbrainz_album_id", |_, this| {
            Ok(this.musicbrainz_albumid.clone())
        });
        fields.add_field_method_get("musicbrainz_album_artist_id", |_, this| {
            Ok(this.musicbrainz_albumartistid.clone())
        });
        fields.add_field_method_get("musicbrainz_track_id", |_, this| {
            Ok(this.musicbrainz_trackid.clone())
        });
        fields.add_field_method_get("musicbrainz_release_group_id", |_, this| {
            Ok(this.musicbrainz_releasegroupid.clone())
        });
        fields.add_field_method_get("musicbrainz_release_track_id", |_, this| {
            Ok(this.musicbrainz_releasetrackid.clone())
        });
        fields.add_field_method_get("musicbrainz_work_id", |_, this| {
            Ok(this.musicbrainz_workid.clone())
        });
    }
}
