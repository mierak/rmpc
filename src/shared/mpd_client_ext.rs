use std::{collections::HashMap, sync::Arc};

use itertools::Itertools;

use crate::{
    config::keys::actions::Position,
    mpd::{
        QueuePosition,
        commands::{State, outputs::Outputs, stickers::Stickers},
        errors::{ErrorCode, MpdError, MpdFailureResponse},
        mpd_client::{Filter, FilterKind, MpdClient, MpdCommand, SingleOrRange, Tag},
        proto_client::ProtoClient,
    },
    shared::macros::status_info,
    status_warn,
};

pub trait MpdClientExt {
    fn play_position_safe(&mut self, queue_len: usize) -> Result<(), MpdError>;
    fn enqueue_multiple(
        &mut self,
        items: Vec<Enqueue>,
        position: Position,
        autoplay: Autoplay,
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

pub enum Autoplay {
    First {
        queue_len: usize,
        current_song_idx: Option<usize>,
    },
    Hovered {
        queue_len: usize,
        current_song_idx: Option<usize>,
        hovered_song_idx: Option<usize>,
    },
    HoveredOrFirst {
        queue_len: usize,
        current_song_idx: Option<usize>,
        hovered_song_idx: Option<usize>,
    },
    None,
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
        position: Position,
        autoplay: Autoplay,
    ) -> Result<(), MpdError> {
        if items.is_empty() {
            return Ok(());
        }
        let should_reverse = match position {
            Position::AfterCurrentSong | Position::StartOfQueue => true,
            Position::BeforeCurrentSong | Position::EndOfQueue | Position::Replace => false,
        };

        if should_reverse {
            items.reverse();
        }

        let autoplay_idx = match autoplay {
            Autoplay::First { queue_len, current_song_idx: Some(curr) } => match position {
                Position::AfterCurrentSong => Some(curr + 1),
                Position::BeforeCurrentSong => Some(curr),
                Position::StartOfQueue => Some(0),
                Position::EndOfQueue => Some(queue_len),
                Position::Replace => Some(0),
            },
            Autoplay::First { queue_len, current_song_idx: None } => match position {
                Position::AfterCurrentSong => {
                    status_warn!("No current song to queue after");
                    return Ok(());
                }
                Position::BeforeCurrentSong => {
                    status_warn!("No current song to queue before");
                    return Ok(());
                }
                Position::StartOfQueue => Some(0),
                Position::EndOfQueue => Some(queue_len),
                Position::Replace => Some(0),
            },
            Autoplay::Hovered { queue_len, current_song_idx, hovered_song_idx } => match position {
                Position::AfterCurrentSong => {
                    let Some(current_song_idx) = current_song_idx else {
                        status_warn!("No current song to queue after");
                        return Ok(());
                    };

                    hovered_song_idx.map(|i| i + 1 + current_song_idx)
                }
                Position::BeforeCurrentSong => {
                    let Some(current_song_idx) = current_song_idx else {
                        status_warn!("No current song to queue before");
                        return Ok(());
                    };

                    hovered_song_idx.map(|i| i + current_song_idx)
                }
                Position::StartOfQueue => hovered_song_idx,
                Position::EndOfQueue => hovered_song_idx.map(|i| i + queue_len),
                Position::Replace => hovered_song_idx,
            },
            Autoplay::HoveredOrFirst { queue_len, current_song_idx, hovered_song_idx } => {
                match position {
                    Position::AfterCurrentSong => {
                        let Some(current_song_idx) = current_song_idx else {
                            status_warn!("No current song to queue after");
                            return Ok(());
                        };

                        hovered_song_idx
                            .map(|i| i + 1 + current_song_idx)
                            .or(Some(current_song_idx + 1))
                    }
                    Position::BeforeCurrentSong => {
                        let Some(current_song_idx) = current_song_idx else {
                            status_warn!("No current song to queue before");
                            return Ok(());
                        };
                        hovered_song_idx.map(|i| i + current_song_idx).or(Some(current_song_idx))
                    }
                    Position::StartOfQueue => hovered_song_idx.or(Some(0)),
                    Position::EndOfQueue => {
                        hovered_song_idx.map(|i| i + queue_len).or(Some(queue_len))
                    }
                    Position::Replace => hovered_song_idx.or(Some(0)),
                }
            }
            Autoplay::None => None,
        };

        self.send_start_cmd_list()?;
        if matches!(position, Position::Replace) {
            self.send_clear()?;
        }
        let position: Option<QueuePosition> = position.into();
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
            self.send_set_sticker(&uri, key, &value.to_string())?;
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
