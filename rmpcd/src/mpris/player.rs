#![allow(clippy::used_underscore_binding)]
use std::{collections::HashMap, sync::Arc};

use rmpc_mpd::{
    commands::{Song, State, status::OnOffOneshot, volume::Bound},
    mpd_client::{MpdClient, MpdCommand, ValueChange},
    proto_client::ProtoClient,
};
use tokio::sync::RwLock;
use zbus::{
    fdo,
    interface,
    object_server::SignalEmitter,
    zvariant::{ObjectPath, Value},
};

use crate::{
    async_client::AsyncClient,
    ctx::Ctx,
    mpris::{
        metadata::SongExt,
        seek::{self, SeekPlan},
    },
};

pub struct Player {
    ctx: Arc<RwLock<Ctx>>,
    client: Arc<AsyncClient>,
}

impl Player {
    pub fn new(ctx: Arc<RwLock<Ctx>>, client: Arc<AsyncClient>) -> Self {
        Self { ctx, client }
    }
}

#[interface(name = "org.mpris.MediaPlayer2.Player")]
impl Player {
    async fn play(&self) -> fdo::Result<()> {
        self.client
            .run(|c| c.play())
            .await
            .map_err(|e| fdo::Error::Failed(format!("Failed to play: {e}")))?;
        Ok(())
    }

    async fn pause(&self) -> fdo::Result<()> {
        self.client
            .run(|c| c.pause())
            .await
            .map_err(|e| fdo::Error::Failed(format!("Failed to pause: {e}")))?;
        Ok(())
    }

    async fn play_pause(&self) -> fdo::Result<()> {
        self.client
            .run(|c| c.pause_toggle())
            .await
            .map_err(|e| fdo::Error::Failed(format!("Failed to toggle play/pause: {e}")))?;
        Ok(())
    }

    async fn stop(&self) -> fdo::Result<()> {
        self.client
            .run(|c| c.stop())
            .await
            .map_err(|e| fdo::Error::Failed(format!("Failed to stop: {e}")))?;
        Ok(())
    }

    async fn next(&self) -> fdo::Result<()> {
        self.client
            .run(|c| c.next())
            .await
            .map_err(|e| fdo::Error::Failed(format!("Failed to go to next track: {e}")))?;
        Ok(())
    }

    async fn previous(&self) -> fdo::Result<()> {
        self.client
            .run(|c| c.prev())
            .await
            .map_err(|e| fdo::Error::Failed(format!("Failed to go to previous track: {e}")))?;
        Ok(())
    }

    async fn seek(
        &self,
        offset: i64,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> fdo::Result<()> {
        let (elapsed_us, duration_us) = {
            let ctx = self.ctx.read().await;
            (ctx.status.elapsed.as_micros() as i64, ctx.status.duration.as_micros() as i64)
        };

        match seek::plan_relative_seek(elapsed_us, duration_us, offset) {
            SeekPlan::Relative { delta_secs, seeked_us } => {
                let change = if delta_secs < 0 {
                    ValueChange::Decrease(delta_secs.unsigned_abs() as u32)
                } else {
                    ValueChange::Increase(delta_secs as u32)
                };
                self.client
                    .run(move |c| c.seek_current(change))
                    .await
                    .map_err(|e| fdo::Error::Failed(format!("Failed to seek: {e}")))?;
                Player::seeked(&emitter, seeked_us).await?;
            }
            SeekPlan::Next => {
                self.client.run(|c| c.next()).await.map_err(|e| {
                    fdo::Error::Failed(format!("Failed to seek past track end: {e}"))
                })?;
            }
            SeekPlan::Absolute { .. } | SeekPlan::Ignore => {}
        }
        Ok(())
    }

    async fn set_position(
        &self,
        track_id: ObjectPath<'_>,
        position: i64,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> fdo::Result<()> {
        let requested_id = Song::id_from_object_path(track_id);
        let (current_id, duration_us) = {
            let ctx = self.ctx.read().await;
            (ctx.current_song.as_ref().map(|song| song.id), ctx.status.duration.as_micros() as i64)
        };

        match seek::plan_set_position(current_id, requested_id, position, duration_us) {
            SeekPlan::Absolute { secs, seeked_us } => {
                self.client
                    .run(move |c| c.seek_current(ValueChange::Set(secs)))
                    .await
                    .map_err(|e| fdo::Error::Failed(format!("Failed to set position: {e}")))?;
                Player::seeked(&emitter, seeked_us).await?;
            }
            SeekPlan::Relative { .. } | SeekPlan::Next | SeekPlan::Ignore => {}
        }
        Ok(())
    }

    #[zbus(signal)]
    pub async fn seeked(emitter: &SignalEmitter<'_>, position: i64) -> zbus::Result<()> {}

    #[zbus(property)]
    async fn volume(&self) -> f64 {
        (*self.ctx.read().await.status.volume.value() as f64 / 100.).clamp(0., 1.)
    }

    #[zbus(property)]
    async fn set_volume(&self, volume: f64) -> zbus::Result<()> {
        let volume = volume.clamp(0.0, 1.0);
        let mpd_volume = (volume * 100.0).round() as u32;

        self.client
            .run(move |c| c.volume(ValueChange::Set(mpd_volume)))
            .await
            .map_err(|e| fdo::Error::Failed(format!("Failed to set MPD volume: {e}")))?;

        self.ctx.write().await.status.volume.set_value(mpd_volume);

        Ok(())
    }

    #[zbus(property)]
    async fn playback_status(&self) -> &str {
        match self.ctx.read().await.status.state {
            State::Play => "Playing",
            State::Stop => "Stopped",
            State::Pause => "Paused",
        }
    }

    #[zbus(property)]
    async fn loop_status(&self) -> &str {
        let state = self.ctx.read().await;
        let single = state.status.single;
        let repeat = state.status.repeat;

        if !repeat {
            return "None";
        }

        match single {
            OnOffOneshot::On => "Track",
            OnOffOneshot::Off => "Playlist",
            OnOffOneshot::Oneshot => "Playlist",
        }
    }

    #[zbus(property)]
    async fn set_loop_status(&self, value: &str) -> zbus::Result<()> {
        let (repeat, single) = match value {
            "None" => (false, OnOffOneshot::Off),
            "Track" => (true, OnOffOneshot::On),
            "Playlist" => (true, OnOffOneshot::Off),
            other => {
                return Err(fdo::Error::InvalidArgs(format!("Invalid loop status: {other}")).into());
            }
        };

        self.client
            .run(move |c| {
                c.send_start_cmd_list()?;
                c.send_repeat(repeat)?;
                c.send_single(single)?;
                c.send_execute_cmd_list()?;
                c.read_ok()?;
                Ok(())
            })
            .await
            .map_err(|e| fdo::Error::Failed(format!("Failed to set loop status: {e}")))?;

        let mut state = self.ctx.write().await;
        state.status.repeat = repeat;
        state.status.single = single;

        Ok(())
    }

    #[zbus(property)]
    async fn shuffle(&self) -> bool {
        self.ctx.read().await.status.random
    }

    #[zbus(property)]
    async fn set_shuffle(&self, value: bool) -> zbus::Result<()> {
        self.client
            .run(move |c| c.random(value))
            .await
            .map_err(|e| fdo::Error::Failed(format!("Failed to set shuffle: {e}")))?;

        self.ctx.write().await.status.random = value;

        Ok(())
    }

    #[zbus(property)]
    async fn metadata(&self) -> zbus::fdo::Result<HashMap<&'static str, Value<'_>>> {
        self.ctx.write().await.current_song_metadata().await
    }

    #[zbus(property)]
    async fn position(&self) -> i64 {
        self.ctx.read().await.status.elapsed.as_micros() as i64
    }

    #[zbus(property)]
    fn can_control(&self) -> bool {
        true
    }

    #[zbus(property)]
    async fn can_go_next(&self) -> bool {
        let state = self.ctx.read().await;

        if state.queue.is_empty() {
            return false;
        }
        if state.status.song.is_some_and(|idx| {
            state.status.playlistlength.saturating_sub(1) as usize == idx && !state.status.repeat
        }) {
            return false;
        }

        true
    }

    #[zbus(property)]
    async fn can_go_previous(&self) -> bool {
        let state = self.ctx.read().await;

        if state.queue.is_empty() {
            return false;
        }

        if state.status.song.is_some_and(|idx| idx == 0 && !state.status.repeat) {
            return false;
        }

        true
    }

    #[zbus(property)]
    fn can_pause(&self) -> bool {
        true
    }

    #[zbus(property)]
    fn can_stop(&self) -> bool {
        true
    }

    #[zbus(property)]
    async fn can_play(&self) -> bool {
        let state = self.ctx.read().await;
        state.status.state != State::Stop || !state.queue.is_empty()
    }

    #[zbus(property)]
    fn can_seek(&self) -> bool {
        true
    }

    #[zbus(property)]
    fn minimum_rate(&self) -> f64 {
        1.0
    }

    #[zbus(property)]
    fn maximum_rate(&self) -> f64 {
        1.0
    }

    #[zbus(property)]
    fn rate(&self) -> f64 {
        1.0
    }
}
