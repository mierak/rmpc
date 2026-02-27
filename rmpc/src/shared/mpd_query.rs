use std::{any::Any, collections::HashMap, sync::Arc};

use anyhow::Result;
use bon::Builder;
use crossbeam::channel::Sender;
use ratatui::{style::Style, widgets::ListItem};
use rmpc_mpd::{
    client::Client,
    commands::{Decoder, IdleEvent, Song, Status, Volume},
    mpd_client::MpdClient,
};

use super::{events::AppEvent, mpd_client_ext::PartitionedOutput};
use crate::{
    config::tabs::PaneType,
    shared::{events::ClientRequest, macros::try_skip},
    ui::{dir_or_song::DirOrSong, dirstack::Path},
};

pub const EXTERNAL_COMMAND: &str = "external_command";
pub const GLOBAL_STATUS_UPDATE: &str = "global_status_update";
pub const GLOBAL_VOLUME_UPDATE: &str = "global_volume_update";
pub const GLOBAL_QUEUE_UPDATE: &str = "global_queue_update";
pub const GLOBAL_STICKERS_UPDATE: &str = "global_stickers_update";

#[derive(derive_more::Debug, Builder)]
pub(crate) struct MpdQuery {
    pub id: &'static str,
    pub replace_id: Option<&'static str>,
    pub target: Option<PaneType>,
    #[debug(skip)]
    pub callback: Box<dyn FnOnce(&mut Client<'_>) -> Result<MpdQueryResult> + Send>,
}

#[derive(derive_more::Debug, Builder)]
pub(crate) struct MpdQuerySync {
    #[debug(skip)]
    pub callback: Box<dyn FnOnce(&mut Client<'_>) -> Result<MpdQueryResult> + Send>,
    pub tx: Sender<MpdQueryResult>,
}

#[derive(derive_more::Debug)]
pub struct MpdCommand {
    #[debug(skip)]
    pub callback: Box<dyn FnOnce(&mut Client<'_>) -> Result<()> + Send>,
}

impl MpdQuery {
    pub(crate) fn should_be_skipped(&self, other: &Self) -> bool {
        let Some(self_replace_id) = self.replace_id else {
            return false;
        };
        let Some(other_replace_id) = other.replace_id else {
            return false;
        };

        return self.id == other.id
            && self_replace_id == other_replace_id
            && self.target == other.target;
    }
}

#[derive(Debug, Clone, Default)]
pub struct PreviewGroup {
    pub name: Option<&'static str>,
    pub items: Vec<ListItem<'static>>,
    pub header_style: Option<Style>,
}

impl PreviewGroup {
    pub fn new(name: Option<&'static str>, header_style: Option<Style>) -> Self {
        Self { name, items: Vec::new(), header_style }
    }

    pub fn push(&mut self, item: ListItem<'static>) {
        self.items.push(item);
    }
}

#[derive(Debug)]
#[allow(unused, clippy::large_enum_variant)]
pub(crate) enum MpdQueryResult {
    SongsList { data: Vec<Song>, path: Option<Path> },
    LsInfo { data: Vec<String>, path: Option<Path> },
    DirOrSong { data: Vec<DirOrSong>, path: Option<Path> },
    SearchResult { data: Vec<Song> },
    AddToPlaylist { playlists: Vec<String>, song_file: String },
    AddToPlaylistMultiple { playlists: Vec<String>, song_files: Vec<String> },
    AlbumArt(Option<Vec<u8>>),
    Status { data: Status, source_event: Option<IdleEvent> },
    Queue(Option<Vec<Song>>),
    Volume(Volume),
    Outputs(Vec<PartitionedOutput>),
    Decoders(Vec<Decoder>),
    ExternalCommand(Arc<Vec<String>>, Vec<String>, Vec<Song>),
    SongStickers(HashMap<String, HashMap<String, String>>),
    Any(Box<dyn Any + Send + Sync>),
}

// Is used as a scheduled function and thus needs the -> Result<()>
#[allow(clippy::unnecessary_wraps)]
pub fn run_status_update((_, client_tx): &(Sender<AppEvent>, Sender<ClientRequest>)) -> Result<()> {
    try_skip!(
        client_tx.send(ClientRequest::Query(MpdQuery {
            id: GLOBAL_STATUS_UPDATE,
            target: None,
            replace_id: Some("status"),
            callback: Box::new(move |client| Ok(MpdQueryResult::Status {
                data: client.get_status()?,
                source_event: None
            })),
        })),
        "Failed to send status update query"
    );
    Ok(())
}
