#![allow(clippy::used_underscore_binding)]
use std::{collections::HashMap, sync::Arc};

use rmpc_mpd::commands::Song;
use tokio::sync::RwLock;
use zbus::{
    interface,
    object_server::SignalEmitter,
    zvariant::{ObjectPath, OwnedObjectPath, Value},
};

use crate::{ctx::Ctx, mpris::metadata::SongExt};

pub struct Tracklist {
    ctx: Arc<RwLock<Ctx>>,
}

impl Tracklist {
    pub fn new(ctx: Arc<RwLock<Ctx>>) -> Self {
        Self { ctx }
    }
}

#[interface(name = "org.mpris.MedPlayer2.TrackList")]
impl Tracklist {
    #[zbus()]
    async fn get_tracks_metadata(
        &self,
        tracks: Vec<ObjectPath<'_>>,
    ) -> Vec<HashMap<&'static str, Value<'_>>> {
        let state = self.ctx.read().await;
        let mut res = Vec::new();
        for track in tracks {
            let id = Song::id_from_object_path(track);
            if let Some(song) = state.queue.iter().find(|s| Some(s.id) == id) {
                let mut metadata = HashMap::new();
                Ctx::song_to_metadata(song, &mut metadata);
                res.push(metadata);
            }
        }

        res
    }

    #[zbus(property)]
    async fn tracks(&self) -> Vec<OwnedObjectPath> {
        self.ctx.read().await.tracks()
    }

    #[zbus()]
    fn add_track(&self, _uri: &str, _after_track: ObjectPath<'_>, _set_as_current: bool) {}

    #[zbus()]
    fn remove_track(&self, _track: ObjectPath<'_>) {}

    #[zbus()]
    fn go_to(&self, _track: ObjectPath<'_>) {}

    #[zbus(signal)]
    pub async fn track_list_replaced(
        ctx: &SignalEmitter<'_>,
        tracks: Vec<ObjectPath<'_>>,
        current_track: ObjectPath<'_>,
    ) -> zbus::Result<()> {
    }
}
