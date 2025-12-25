use std::{collections::HashMap, path::PathBuf, sync::Arc};

use anyhow::Context;
use itertools::Itertools;

use crate::{
    config::keys::actions::{AddOpts, AutoplayKind, Position},
    ctx::Ctx,
    mpd::{
        QueuePosition,
        commands::{IdleEvent, State, Status, outputs::Outputs, stickers::Stickers},
        errors::{ErrorCode, MpdError, MpdFailureResponse},
        mpd_client::{Filter, FilterKind, MpdClient, MpdCommand, SingleOrRange, Tag},
        proto_client::ProtoClient,
    },
    shared::macros::{status_info, status_warn},
};

pub trait MpdClientExt {
    fn resolve_and_enqueue(
        ctx: &Ctx,
        items: Vec<Enqueue>,
        position: Position,
        autoplay: AutoplayKind,
        current_song_idx: Option<usize>,
        hovered_song_idx: Option<usize>,
    ) {
        let opts = AddOpts { autoplay, position, all: false };
        let replace = matches!(position, Position::Replace);
        let (autoplay_idx, position) = match opts.autoplay_idx_and_queue_position(
            &ctx.queue,
            current_song_idx,
            hovered_song_idx,
        ) {
            Ok(v) => v,
            Err(err) => {
                status_warn!("{}", err);
                return;
            }
        };

        ctx.command(move |client| {
            client.enqueue_multiple(items, autoplay_idx, position, replace)?;
            Ok(())
        });
    }
    fn play_position_safe(&mut self, queue_len: usize) -> Result<(), MpdError>;
    fn enqueue_multiple(
        &mut self,
        items: Vec<Enqueue>,
        autoplay_idx: Option<usize>,
        position: Option<QueuePosition>,
        replace: bool,
    ) -> Result<(), MpdError>;
    fn delete_multiple(&mut self, items: Vec<MpdDelete>) -> Result<(), MpdError>;
    fn add_to_playlist_multiple(
        &mut self,
        playlist_name: &str,
        song_paths: Vec<String>,
    ) -> Result<(), MpdError>;
    fn list_partitioned_outputs(
        &mut self,
        current_partition: &str,
    ) -> Result<Vec<PartitionedOutput>, MpdError>;
    fn create_playlist(&mut self, name: &str, items: Vec<String>) -> Result<(), MpdError>;
    fn next_keep_state(&mut self, keep: bool, state: State) -> Result<(), MpdError>;
    fn prev_keep_state(&mut self, keep: bool, state: State) -> Result<(), MpdError>;
    fn fetch_song_stickers(
        &mut self,
        song_uris: Vec<String>,
    ) -> Result<HashMap<String, HashMap<String, String>>, MpdError>;
    fn set_sticker_multiple(
        &mut self,
        key: &str,
        value: String,
        items: Vec<Enqueue>,
    ) -> Result<(), MpdError>;
    fn delete_sticker_multiple(&mut self, key: &str, items: Vec<Enqueue>) -> Result<(), MpdError>;
    fn add_downloaded_files_to_queue(
        &mut self,
        paths: Vec<String>,
        cache_dir: Option<PathBuf>,
        position: Option<QueuePosition>,
    ) -> Result<(), MpdError>;
}

#[derive(Debug, Clone)]
pub enum MpdDelete {
    SongInPlaylist { playlist: Arc<str>, range: SingleOrRange },
    Playlist { name: String },
}

#[allow(dead_code, reason = "Search is currently unused")]
#[derive(Debug, Clone)]
pub enum Enqueue {
    File { path: String },
    Playlist { name: String },
    Find { filter: Vec<(Tag, FilterKind, String)> },
}

impl<T: MpdClient + MpdCommand + ProtoClient> MpdClientExt for T {
    fn play_position_safe(&mut self, queue_len: usize) -> Result<(), MpdError> {
        match self.play_pos(queue_len) {
            Ok(()) => {}
            Err(MpdError::Mpd(MpdFailureResponse { code: ErrorCode::Argument, .. })) => {
                // This can happen when multiple clients modify the queue at
                // the same time. But a more robust
                // solution would require refetching the whole
                // queue and searching for the added song. This should be
                // good enough.
                log::warn!("Failed to autoplay song");
            }
            Err(err) => return Err(err),
        }
        Ok(())
    }

    fn enqueue_multiple(
        &mut self,
        mut items: Vec<Enqueue>,
        autoplay_idx: Option<usize>,
        position: Option<QueuePosition>,
        replace: bool,
    ) -> Result<(), MpdError> {
        if items.is_empty() {
            return Ok(());
        }
        let should_reverse = match position {
            Some(QueuePosition::RelativeAdd(_)) => true,
            Some(QueuePosition::RelativeSub(_)) => false,
            Some(QueuePosition::Absolute(_)) => true,
            None => false,
        };

        if should_reverse {
            items.reverse();
        }

        self.send_start_cmd_list()?;
        if replace {
            self.send_clear()?;
        }

        let items_len = items.len();
        for item in items {
            match item {
                Enqueue::File { path } => self.send_add(&path, position),
                Enqueue::Playlist { name } => self.send_load_playlist(&name, position),
                Enqueue::Find { filter } => self.send_find_add(
                    &filter
                        .into_iter()
                        .map(|(tag, kind, value)| Filter::new_with_kind(tag, value, kind))
                        .collect_vec(),
                    position,
                ),
            }?;
        }
        self.send_execute_cmd_list()?;
        self.read_ok()?;
        if items_len == 1 {
            status_info!("Added 1 item to the queue");
        } else {
            status_info!("Added {items_len} items to the queue");
        }

        if let Some(autoplay_idx) = autoplay_idx {
            self.play_position_safe(autoplay_idx)?;
        }

        Ok(())
    }

    fn delete_multiple(&mut self, items: Vec<MpdDelete>) -> Result<(), MpdError> {
        let items_len = items.len();
        if items_len == 0 {
            return Ok(());
        }

        self.send_start_cmd_list()?;
        for item in items.into_iter().rev() {
            match item {
                MpdDelete::SongInPlaylist { playlist, range } => {
                    self.send_delete_from_playlist(&playlist, &range)?;
                }
                MpdDelete::Playlist { name } => {
                    self.send_delete_playlist(&name)?;
                }
            }
        }
        self.send_execute_cmd_list()?;
        self.read_ok()?;

        if items_len == 1 {
            status_info!("Deleted 1 item");
        } else {
            status_info!("Deleted {} items", items_len);
        }

        Ok(())
    }

    fn add_to_playlist_multiple(
        &mut self,
        playlist_name: &str,
        song_paths: Vec<String>,
    ) -> Result<(), MpdError> {
        let items_len = song_paths.len();
        if items_len == 0 {
            return Ok(());
        }

        self.send_start_cmd_list()?;
        for mut path in song_paths {
            if path.starts_with('/') {
                path.insert_str(0, "file://");
                self.add_to_playlist(playlist_name, &path, None)?;
            } else {
                self.send_add_to_playlist(playlist_name, &path, None)?;
            }
        }
        self.send_execute_cmd_list()?;
        self.read_ok()?;

        if items_len == 1 {
            status_info!("Added 1 song to playlist {}", playlist_name);
        } else {
            status_info!("Added {} songs to playlist {}", items_len, playlist_name);
        }

        Ok(())
    }

    fn list_partitioned_outputs(
        &mut self,
        current_partition: &str,
    ) -> Result<Vec<PartitionedOutput>, MpdError> {
        if current_partition == "default" {
            Ok(self
                .outputs()?
                .0
                .into_iter()
                .map(|output| PartitionedOutput {
                    id: output.id,
                    name: output.name,
                    enabled: if output.plugin == "dummy" { false } else { output.enabled },
                    kind: if output.plugin == "dummy" {
                        PartitionedOutputKind::OtherPartition
                    } else {
                        PartitionedOutputKind::CurrentPartition
                    },
                    plugin: output.plugin,
                })
                .collect())
        } else {
            // MPD lists all outputs only on the default partition so we have to
            // switch to it, list the outputs and then switch back. We also have to
            // list outputs on the current partition to find out which output is
            // actually enabled on the current partition.
            self.send_start_cmd_list_ok()?;
            self.send_switch_to_partition("default")?;
            self.send_outputs()?;
            self.send_switch_to_partition(current_partition)?;
            self.send_outputs()?;
            self.send_execute_cmd_list()?;

            self.read_ok()?; // switch to default
            let all_outputs = self.read_response::<Outputs>()?.0;
            self.read_ok()?; // switch to current
            let mut current_outputs = self.read_response::<Outputs>()?.0;
            self.read_ok()?; // OK for the whole command list

            let mut result = Vec::with_capacity(all_outputs.len());
            for output in all_outputs {
                if let Some(current) = current_outputs
                    .iter_mut()
                    .find(|o| o.name == output.name && o.plugin != "dummy")
                {
                    result.push(PartitionedOutput {
                        id: current.id,
                        name: std::mem::take(&mut current.name),
                        enabled: current.enabled,
                        plugin: std::mem::take(&mut current.plugin),
                        kind: PartitionedOutputKind::CurrentPartition,
                    });
                } else {
                    result.push(PartitionedOutput {
                        id: output.id,
                        name: output.name,
                        enabled: false,
                        plugin: output.plugin,
                        kind: PartitionedOutputKind::OtherPartition,
                    });
                }
            }

            Ok(result)
        }
    }

    fn create_playlist(&mut self, name: &str, items: Vec<String>) -> Result<(), MpdError> {
        if items.is_empty() {
            return Ok(());
        }
        self.send_start_cmd_list()?;
        // MPD does not allow creating empty playlists. We work
        // around it here by saving the current queue and then
        // clearing the newly created playlist.
        self.send_save_queue_as_playlist(name, None)?;
        self.send_clear_playlist(name)?;
        for item in &items {
            self.send_add_to_playlist(name, item, None)?;
        }
        self.send_execute_cmd_list()?;
        self.read_ok()?;

        status_info!("Created playlist {name} with {} items", items.len());

        Ok(())
    }

    fn next_keep_state(&mut self, keep: bool, state: State) -> Result<(), MpdError> {
        if !keep {
            return self.next();
        }

        match state {
            State::Play => self.next(),
            State::Stop => Ok(()),
            State::Pause => {
                self.send_start_cmd_list()?;
                self.send_next()?;
                self.send_pause()?;
                self.send_execute_cmd_list()?;
                self.read_ok()
            }
        }
    }

    fn prev_keep_state(&mut self, keep: bool, state: State) -> Result<(), MpdError> {
        if !keep {
            return self.prev();
        }

        match state {
            State::Play => self.prev(),
            State::Stop => Ok(()),
            State::Pause => {
                self.send_start_cmd_list()?;
                self.send_prev()?;
                self.send_pause()?;
                self.send_execute_cmd_list()?;
                self.read_ok()
            }
        }
    }

    fn fetch_song_stickers(
        &mut self,
        mut song_uris: Vec<String>,
    ) -> Result<HashMap<String, HashMap<String, String>>, MpdError> {
        if song_uris.is_empty() {
            return Ok(HashMap::new());
        }

        let mut list_ended_with_err = false;
        let mut i = 0;
        let mut result = HashMap::new();

        while i < song_uris.len() {
            self.send_start_cmd_list_ok()?;
            for uri in &song_uris[i..] {
                self.send_list_stickers(uri)?;
            }
            self.send_execute_cmd_list()?;

            for uri in &mut song_uris[i..] {
                let res: Result<Stickers, _> = self.read_response();
                match res {
                    Ok(stickers) => {
                        list_ended_with_err = false;
                        result.insert(std::mem::take(uri), stickers.0);
                        i += 1;
                    }
                    Err(error) => {
                        log::warn!(error:?, file = uri.as_str(); "Tried to find stickers but unexpected error occurred");
                        result.insert(std::mem::take(uri), HashMap::new());
                        list_ended_with_err = true;
                        i += 1;
                        break;
                    }
                }
            }
        }

        // In case the last sticker was fetched successfully we have to read an
        // OK as an ack for the whole command list
        if !list_ended_with_err {
            self.read_ok()?;
        }

        log::debug!(count = result.len(); "Fetched stickers for songs");
        Ok(result)
    }

    fn set_sticker_multiple(
        &mut self,
        key: &str,
        value: String,
        items: Vec<Enqueue>,
    ) -> Result<(), MpdError> {
        let mut uris = Vec::new();
        for item in items {
            match item {
                Enqueue::File { path } => uris.push(path),
                Enqueue::Playlist { name } => {
                    let playlist = self.list_playlist(&name)?.0;
                    uris.extend(playlist);
                }
                Enqueue::Find { filter } => {
                    let songs = self.find(
                        &filter
                            .into_iter()
                            .map(|(tag, kind, value)| Filter::new_with_kind(tag, value, kind))
                            .collect_vec(),
                    )?;
                    uris.extend(songs.into_iter().map(|song| song.file));
                }
            }
        }

        self.send_start_cmd_list()?;
        for uri in uris {
            self.send_set_sticker(&uri, key, &value.clone())?;
        }
        self.send_execute_cmd_list()?;
        self.read_ok()?;

        Ok(())
    }

    fn delete_sticker_multiple(&mut self, key: &str, items: Vec<Enqueue>) -> Result<(), MpdError> {
        let mut uris = Vec::new();
        for item in items {
            match item {
                Enqueue::File { path } => uris.push(path),
                Enqueue::Playlist { name } => {
                    let playlist = self.list_playlist(&name)?.0;
                    uris.extend(playlist);
                }
                Enqueue::Find { filter } => {
                    let songs = self.find(
                        &filter
                            .into_iter()
                            .map(|(tag, kind, value)| Filter::new_with_kind(tag, value, kind))
                            .collect_vec(),
                    )?;
                    uris.extend(songs.into_iter().map(|song| song.file));
                }
            }
        }

        for uri in uris {
            match self.delete_sticker(&uri, key) {
                Ok(()) => {}
                Err(MpdError::Mpd(MpdFailureResponse { code: ErrorCode::NoExist, .. })) => {}
                err @ Err(_) => err?,
            }
        }

        Ok(())
    }

    fn add_downloaded_files_to_queue(
        &mut self,
        paths: Vec<String>,
        cache_dir: Option<PathBuf>,
        position: Option<QueuePosition>,
    ) -> Result<(), MpdError> {
        self.send_start_cmd_list()?;
        for file in &paths {
            self.send_add(file, position)?;
        }
        self.send_execute_cmd_list()?;
        match self.read_ok() {
            Ok(()) => {}
            Err(MpdError::Mpd(err)) if err.is_no_exist() => {
                let Some(cache_dir) = cache_dir else {
                    // This should not happen, the download should only happen when
                    // cache_dir is defined in the first place.
                    log::error!(err:?; "MPD reported error when adding files from yt-dlp cache dir, but cache_dir is not configured");
                    return Err(MpdError::Mpd(err))?;
                };
                let Some(cfg) = &self.config() else {
                    // This should not happen either, music_directory is required for
                    // MPD to work and rmpc needs socket connection to use yt-dpl so it
                    // should always have permission to access the config.
                    log::error!(err:?; "MPD reported error when adding files from yt-dlp cache dir, but cannot get music_directory from MPD");
                    return Err(MpdError::Mpd(err))?;
                };
                let music_directory = &cfg.music_directory;

                log::warn!(cache_dir:?, music_directory:?; "MPD reported noexist error when adding files from yt-dlp cache dir. Will try again after issuing database update.");

                let Ok(update_dir) = cache_dir.strip_prefix(music_directory) else {
                    // Rethrow the original error. The cache_dir is not inside the
                    // music_directory so there is no reason to try to issue update to
                    // mpd.
                    return Err(MpdError::Mpd(err))?;
                };

                log::trace!("Issuing database update");
                let job_id = self
                    .update(Some(
                        update_dir
                            .to_str() // MPD protocol is always utf-8, this should be safe
                            .context("update dir is not valid utf-8")?,
                    ))?
                    .job_id;

                // Wait for update to finish
                loop {
                    log::trace!("Entering idle, waiting for update event");
                    self.idle(Some(IdleEvent::Update))?;
                    let Status { updating_db, .. } = self.get_status()?;
                    log::trace!("Update event received");
                    match updating_db {
                        Some(current_id) if current_id > job_id => {
                            break;
                        }
                        Some(_id) => {}
                        None => break,
                    }
                }

                log::debug!("Trying to add the downloaded files again");
                self.send_start_cmd_list()?;
                for file in &paths {
                    self.send_add(file, position)?;
                }
                self.send_execute_cmd_list()?;
                self.read_ok()?;
            }
            original @ Err(_) => original?,
        }

        Ok(())
    }
}

/// Output where ID is only defined when the output is on the current
/// partition.
#[derive(Debug)]
pub struct PartitionedOutput {
    pub id: u32,
    pub name: String,
    pub enabled: bool,
    pub plugin: String,
    pub kind: PartitionedOutputKind,
}

#[derive(Debug, Clone, Copy)]
pub enum PartitionedOutputKind {
    OtherPartition,
    CurrentPartition,
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use rstest::{fixture, rstest};

    use crate::{
        config::keys::actions::{AddOpts, AutoplayKind, Position},
        ctx::Ctx,
        mpd::{QueuePosition, commands::Song},
        tests::fixtures::ctx,
    };

    mod enqueue_multiple {
        use std::collections::HashMap;

        use super::*;

        #[fixture]
        fn ctx_with_queue(mut ctx: Ctx) -> Ctx {
            let albums = ["a", "b", "b", "b", "c", "c", "d", "e", "e", "f"];
            for i in 0..10 {
                ctx.queue.push(Song {
                    id: i,
                    file: format!("song{i}"),
                    metadata: HashMap::from([(
                        "album".to_owned(),
                        albums[i as usize].to_owned().into(),
                    )]),
                    ..Default::default()
                });
            }
            ctx
        }

        #[rstest]
        fn first_after_current_album(ctx_with_queue: Ctx) {
            let position = Position::AfterCurrentAlbum;
            let autoplay = AutoplayKind::First;
            let current_song_idx = Some(4);
            let hovered = None;
            let opts = AddOpts { autoplay, position, all: false };

            let (autoplay_idx, queue_position) = opts
                .autoplay_idx_and_queue_position(&ctx_with_queue.queue, current_song_idx, hovered)
                .unwrap();

            assert_eq!(queue_position, Some(QueuePosition::Absolute(6)));
            assert_eq!(autoplay_idx, Some(6));
        }

        #[rstest]
        fn hovered_after_current_album(ctx_with_queue: Ctx) {
            let position = Position::AfterCurrentAlbum;
            let current_song_idx = Some(4);
            let hovered = Some(1);
            let autoplay = AutoplayKind::Hovered;
            let opts = AddOpts { autoplay, position, all: false };

            let (autoplay_idx, queue_position) = opts
                .autoplay_idx_and_queue_position(&ctx_with_queue.queue, current_song_idx, hovered)
                .unwrap();

            assert_eq!(queue_position, Some(QueuePosition::Absolute(6)));
            assert_eq!(autoplay_idx, Some(7));
        }

        #[rstest]
        fn none_after_current_album(ctx_with_queue: Ctx) {
            let position = Position::AfterCurrentAlbum;
            let current_song_idx = Some(4);
            let hovered = Some(1);
            let autoplay = AutoplayKind::None;
            let opts = AddOpts { autoplay, position, all: false };

            let (autoplay_idx, queue_position) = opts
                .autoplay_idx_and_queue_position(&ctx_with_queue.queue, current_song_idx, hovered)
                .unwrap();

            assert_eq!(queue_position, Some(QueuePosition::Absolute(6)));
            assert_eq!(autoplay_idx, None);
        }

        #[rstest]
        fn first_after_current_album_when_at_the_end_of_queue(ctx_with_queue: Ctx) {
            let position = Position::AfterCurrentAlbum;
            let autoplay = AutoplayKind::First;
            let current_song_idx = Some(9);
            let hovered = None;
            let opts = AddOpts { autoplay, position, all: false };

            let (autoplay_idx, queue_position) = opts
                .autoplay_idx_and_queue_position(&ctx_with_queue.queue, current_song_idx, hovered)
                .unwrap();

            assert_eq!(queue_position, Some(QueuePosition::Absolute(10)));
            assert_eq!(autoplay_idx, Some(10));
        }

        #[rstest]
        fn first_before_current_album(ctx_with_queue: Ctx) {
            let position = Position::BeforeCurrentAlbum;
            let autoplay = AutoplayKind::First;
            let current_song_idx = Some(5);
            let hovered = None;
            let opts = AddOpts { autoplay, position, all: false };

            let (autoplay_idx, queue_position) = opts
                .autoplay_idx_and_queue_position(&ctx_with_queue.queue, current_song_idx, hovered)
                .unwrap();

            assert_eq!(queue_position, Some(QueuePosition::Absolute(4)));
            assert_eq!(autoplay_idx, Some(4));
        }

        #[rstest]
        fn hovered_before_current_album(ctx_with_queue: Ctx) {
            let position = Position::BeforeCurrentAlbum;
            let current_song_idx = Some(5);
            let hovered = Some(1);
            let autoplay = AutoplayKind::Hovered;
            let opts = AddOpts { autoplay, position, all: false };

            let (autoplay_idx, queue_position) = opts
                .autoplay_idx_and_queue_position(&ctx_with_queue.queue, current_song_idx, hovered)
                .unwrap();

            assert_eq!(queue_position, Some(QueuePosition::Absolute(4)));
            assert_eq!(autoplay_idx, Some(5));
        }

        #[rstest]
        fn none_before_current_album(ctx_with_queue: Ctx) {
            let position = Position::BeforeCurrentAlbum;
            let current_song_idx = Some(5);
            let hovered = Some(1);
            let autoplay = AutoplayKind::None;
            let opts = AddOpts { autoplay, position, all: false };

            let (autoplay_idx, queue_position) = opts
                .autoplay_idx_and_queue_position(&ctx_with_queue.queue, current_song_idx, hovered)
                .unwrap();

            assert_eq!(queue_position, Some(QueuePosition::Absolute(4)));
            assert_eq!(autoplay_idx, None);
        }

        #[rstest]
        fn first_after_current_song(ctx_with_queue: Ctx) {
            let position = Position::AfterCurrentSong;
            let autoplay = AutoplayKind::First;
            let current_song_idx = Some(5);
            let hovered = None;
            let opts = AddOpts { autoplay, position, all: false };

            let (autoplay_idx, queue_position) = opts
                .autoplay_idx_and_queue_position(&ctx_with_queue.queue, current_song_idx, hovered)
                .unwrap();

            assert_eq!(queue_position, Some(QueuePosition::RelativeAdd(0)));
            assert_eq!(autoplay_idx, Some(6));
        }

        #[rstest]
        fn hovered_after_current_song(ctx_with_queue: Ctx) {
            let position = Position::AfterCurrentSong;
            let current_song_idx = Some(5);
            let hovered = Some(1);
            let autoplay = AutoplayKind::Hovered;
            let opts = AddOpts { autoplay, position, all: false };

            let (autoplay_idx, queue_position) = opts
                .autoplay_idx_and_queue_position(&ctx_with_queue.queue, current_song_idx, hovered)
                .unwrap();

            assert_eq!(queue_position, Some(QueuePosition::RelativeAdd(0)));
            assert_eq!(autoplay_idx, Some(7));
        }

        #[rstest]
        fn none_after_current_song(ctx_with_queue: Ctx) {
            let position = Position::AfterCurrentSong;
            let current_song_idx = Some(5);
            let hovered = Some(1);
            let autoplay = AutoplayKind::None;
            let opts = AddOpts { autoplay, position, all: false };

            let (autoplay_idx, queue_position) = opts
                .autoplay_idx_and_queue_position(&ctx_with_queue.queue, current_song_idx, hovered)
                .unwrap();

            assert_eq!(queue_position, Some(QueuePosition::RelativeAdd(0)));
            assert_eq!(autoplay_idx, None);
        }

        #[rstest]
        fn first_start_of_queue(ctx_with_queue: Ctx) {
            let position = Position::StartOfQueue;
            let current_song_idx = Some(5);
            let hovered = None;
            let autoplay = AutoplayKind::First;
            let opts = AddOpts { autoplay, position, all: false };

            let (autoplay_idx, queue_position) = opts
                .autoplay_idx_and_queue_position(&ctx_with_queue.queue, current_song_idx, hovered)
                .unwrap();

            assert_eq!(queue_position, Some(QueuePosition::Absolute(0)));
            assert_eq!(autoplay_idx, Some(0));
        }

        #[rstest]
        fn hovered_start_of_queue(ctx_with_queue: Ctx) {
            let position = Position::StartOfQueue;
            let current_song_idx = Some(5);
            let hovered = Some(1);
            let autoplay = AutoplayKind::Hovered;
            let opts = AddOpts { autoplay, position, all: false };

            let (autoplay_idx, queue_position) = opts
                .autoplay_idx_and_queue_position(&ctx_with_queue.queue, current_song_idx, hovered)
                .unwrap();

            assert_eq!(queue_position, Some(QueuePosition::Absolute(0)));
            assert_eq!(autoplay_idx, Some(1));
        }

        #[rstest]
        fn none_start_of_queue(ctx_with_queue: Ctx) {
            let position = Position::StartOfQueue;
            let current_song_idx = Some(5);
            let hovered = Some(1);
            let autoplay = AutoplayKind::None;
            let opts = AddOpts { autoplay, position, all: false };

            let (autoplay_idx, queue_position) = opts
                .autoplay_idx_and_queue_position(&ctx_with_queue.queue, current_song_idx, hovered)
                .unwrap();

            assert_eq!(queue_position, Some(QueuePosition::Absolute(0)));
            assert_eq!(autoplay_idx, None);
        }

        #[rstest]
        fn first_replace(ctx_with_queue: Ctx) {
            let position = Position::Replace;
            let current_song_idx = Some(5);
            let hovered = Some(1);
            let autoplay = AutoplayKind::First;
            let opts = AddOpts { autoplay, position, all: false };

            let (autoplay_idx, queue_position) = opts
                .autoplay_idx_and_queue_position(&ctx_with_queue.queue, current_song_idx, hovered)
                .unwrap();

            assert_eq!(queue_position, None);
            assert_eq!(autoplay_idx, Some(0));
        }

        #[rstest]
        fn hovered_replace(ctx_with_queue: Ctx) {
            let position = Position::Replace;
            let current_song_idx = Some(5);
            let hovered = Some(1);
            let autoplay = AutoplayKind::Hovered;
            let opts = AddOpts { autoplay, position, all: false };

            let (autoplay_idx, queue_position) = opts
                .autoplay_idx_and_queue_position(&ctx_with_queue.queue, current_song_idx, hovered)
                .unwrap();

            assert_eq!(queue_position, None);
            assert_eq!(autoplay_idx, Some(1));
        }

        #[rstest]
        fn none_replace(ctx_with_queue: Ctx) {
            let position = Position::Replace;
            let current_song_idx = Some(5);
            let hovered = Some(1);
            let autoplay = AutoplayKind::None;
            let opts = AddOpts { autoplay, position, all: false };

            let (autoplay_idx, queue_position) = opts
                .autoplay_idx_and_queue_position(&ctx_with_queue.queue, current_song_idx, hovered)
                .unwrap();

            assert_eq!(queue_position, None);
            assert_eq!(autoplay_idx, None);
        }

        #[rstest]
        fn first_before_current_song(ctx_with_queue: Ctx) {
            let position = Position::BeforeCurrentSong;
            let current_song_idx = Some(5);
            let hovered = Some(1);
            let autoplay = AutoplayKind::First;
            let opts = AddOpts { autoplay, position, all: false };

            let (autoplay_idx, queue_position) = opts
                .autoplay_idx_and_queue_position(&ctx_with_queue.queue, current_song_idx, hovered)
                .unwrap();

            assert_eq!(queue_position, Some(QueuePosition::RelativeSub(0)));
            assert_eq!(autoplay_idx, Some(5));
        }

        #[rstest]
        fn hovered_before_current_song(ctx_with_queue: Ctx) {
            let position = Position::BeforeCurrentSong;
            let current_song_idx = Some(5);
            let hovered = Some(1);
            let autoplay = AutoplayKind::Hovered;
            let opts = AddOpts { autoplay, position, all: false };

            let (autoplay_idx, queue_position) = opts
                .autoplay_idx_and_queue_position(&ctx_with_queue.queue, current_song_idx, hovered)
                .unwrap();

            assert_eq!(queue_position, Some(QueuePosition::RelativeSub(0)));
            assert_eq!(autoplay_idx, Some(6));
        }

        #[rstest]
        fn none_before_current_song(ctx_with_queue: Ctx) {
            let position = Position::BeforeCurrentSong;
            let current_song_idx = Some(5);
            let hovered = Some(1);
            let autoplay = AutoplayKind::None;
            let opts = AddOpts { autoplay, position, all: false };

            let (autoplay_idx, queue_position) = opts
                .autoplay_idx_and_queue_position(&ctx_with_queue.queue, current_song_idx, hovered)
                .unwrap();

            assert_eq!(queue_position, Some(QueuePosition::RelativeSub(0)));
            assert_eq!(autoplay_idx, None);
        }

        #[rstest]
        fn first_end_of_queue(ctx_with_queue: Ctx) {
            let position = Position::EndOfQueue;
            let current_song_idx = Some(5);
            let hovered = Some(1);
            let autoplay = AutoplayKind::First;
            let opts = AddOpts { autoplay, position, all: false };

            let (autoplay_idx, queue_position) = opts
                .autoplay_idx_and_queue_position(&ctx_with_queue.queue, current_song_idx, hovered)
                .unwrap();

            assert_eq!(queue_position, None);
            assert_eq!(autoplay_idx, Some(10));
        }

        #[rstest]
        fn hovered_end_of_queue(ctx_with_queue: Ctx) {
            let position = Position::EndOfQueue;
            let current_song_idx = Some(5);
            let hovered = Some(1);
            let autoplay = AutoplayKind::Hovered;
            let opts = AddOpts { autoplay, position, all: false };

            let (autoplay_idx, queue_position) = opts
                .autoplay_idx_and_queue_position(&ctx_with_queue.queue, current_song_idx, hovered)
                .unwrap();

            assert_eq!(queue_position, None);
            assert_eq!(autoplay_idx, Some(11));
        }

        #[rstest]
        fn none_end_of_queue(ctx_with_queue: Ctx) {
            let position = Position::EndOfQueue;
            let current_song_idx = Some(5);
            let hovered = Some(1);
            let autoplay = AutoplayKind::None;
            let opts = AddOpts { autoplay, position, all: false };

            let (autoplay_idx, queue_position) = opts
                .autoplay_idx_and_queue_position(&ctx_with_queue.queue, current_song_idx, hovered)
                .unwrap();

            assert_eq!(queue_position, None);
            assert_eq!(autoplay_idx, None);
        }
    }
}
