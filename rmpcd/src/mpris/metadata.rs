use std::collections::HashMap;

use rmpc_mpd::commands::Song;
use zbus::zvariant::{ObjectPath, OwnedObjectPath, Value};

use crate::ctx::Ctx;

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
    pub(super) async fn current_song_metadata<'a>(
        &self,
    ) -> zbus::fdo::Result<HashMap<&'static str, Value<'a>>> {
        if let Some(song) = &self.current_song {
            let mut res = HashMap::new();
            Self::song_to_metadata(song, &mut res);
            tracing::debug!(art = self.album_art.is_some(), "Writing cover art to temp file");
            if let Some(buf) = &self.album_art {
                // TODO this cannot be here, this writes every time the metadata are queried and
                // it should probably utilize a cache to not write the same file all the time
                tokio::fs::write(std::env::temp_dir().join("rmpcd-cover-art"), buf).await.map_err(
                    |err| {
                        zbus::fdo::Error::Failed(format!(
                            "Failed to write cover art to temp file: {err}"
                        ))
                    },
                )?;
                res.insert(
                    "mpris:artUrl",
                    Value::new(format!(
                        "file://{}",
                        std::env::temp_dir().join("rmpcd-cover-art").to_string_lossy()
                    )),
                );
            }
            Ok(res)
        } else {
            Ok(HashMap::new())
        }
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
