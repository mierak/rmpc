use std::{collections::HashMap, path::PathBuf};

use rmpc_mpd::commands::Song;
use sha2::{Digest, Sha256};
use url::Url;
use zbus::zvariant::{ObjectPath, OwnedObjectPath, Value};

use crate::ctx::{ALBUM_ART_CACHE_DIR, Ctx};

pub trait SongExt {
    fn to_mpris_id(&self) -> OwnedObjectPath;
    fn id_from_object_path(path: ObjectPath<'_>) -> Option<u32>;
}

impl SongExt for Song {
    fn to_mpris_id(&self) -> OwnedObjectPath {
        OwnedObjectPath::try_from(format!("/org/musicpd/song/{}", self.id))
            .expect("song mpris id should always be a valid object path")
    }

    fn id_from_object_path(path: ObjectPath<'_>) -> Option<u32> {
        path.as_str().rsplit('/').next().and_then(|id_str| id_str.parse::<u32>().ok())
    }
}

impl Ctx {
    pub fn album_art_path(&self, song_uri: &str) -> PathBuf {
        let mut hasher = Sha256::new();
        hasher.update(song_uri.as_bytes());
        let file_name = format!("{:x}", hasher.finalize());
        self.cache_dir.join(ALBUM_ART_CACHE_DIR).join(file_name)
    }

    async fn clear_current_album_art_file(&self) {
        if let Some(uri) = &self.last_written_album_art_song_uri {
            let existing_path = self.album_art_path(uri);
            if let Err(err) = tokio::fs::remove_file(existing_path).await
                && err.kind() != std::io::ErrorKind::NotFound
            {
                tracing::warn!(err = ?err, "Failed to remove existing album art file");
            }
        }
    }

    pub(super) async fn current_song_metadata<'a>(
        &mut self,
    ) -> zbus::fdo::Result<HashMap<&'static str, Value<'a>>> {
        let song = if let Some(song) = &self.current_song {
            song
        } else {
            self.clear_current_album_art_file().await;
            self.last_written_album_art_song_uri = None;
            return Ok(HashMap::new());
        };

        let mut res = HashMap::new();
        Self::song_to_metadata(song, &mut res);

        let target_path = self.album_art_path(song.file.as_str());

        if let Some(buf) = &self.album_art {
            if self.last_written_album_art_song_uri.as_ref().is_none_or(|uri| uri != &song.file) {
                tracing::debug!(path = ?target_path, art = self.album_art.is_some(), "Writing cover art to file");

                self.clear_current_album_art_file().await;

                tokio::fs::write(&target_path, buf).await.map_err(|err| {
                    zbus::fdo::Error::Failed(format!("Failed to write cover art to file: {err}"))
                })?;
                self.last_written_album_art_song_uri = Some(song.file.clone());
            }
        } else {
            self.clear_current_album_art_file().await;
            self.last_written_album_art_song_uri = None;
        }

        if self.last_written_album_art_song_uri.as_ref().is_some_and(|uri| uri == &song.file) {
            match Url::from_file_path(&target_path) {
                Ok(url) => {
                    res.insert("mpris:artUrl", Value::new(url.to_string()));
                }
                Err(()) => {
                    tracing::warn!(path = ?target_path, "Failed to build file URL for album art");
                }
            }
        }

        Ok(res)
    }

    pub(super) fn tracks(&self) -> Vec<OwnedObjectPath> {
        self.queue.iter().map(|song| song.to_mpris_id()).collect()
    }

    pub(super) fn song_to_metadata(song: &Song, metadata: &mut HashMap<&'static str, Value<'_>>) {
        metadata.insert("mpris:trackid", Value::new(song.to_mpris_id()));

        if let Some(artist) = song.metadata.get("artist") {
            metadata.insert(
                "xesam:artist",
                Value::from(artist.iter().map(|a| a.to_owned()).collect::<Vec<_>>()),
            );
        }
        if let Some(aartist) = song.metadata.get("albumartist") {
            metadata.insert("xesam:albumArtist", Value::from(aartist.first().to_owned()));
        }
        if let Some(title) = song.metadata.get("title") {
            metadata.insert("xesam:title", Value::from(title.first().to_owned()));
        }
        if let Some(album) = song.metadata.get("album") {
            metadata.insert("xesam:album", Value::from(album.first().to_owned()));
        }
        if let Some(dur) = song.duration {
            metadata.insert("mpris:length", Value::from(dur.as_micros() as i64));
        }
    }
}
