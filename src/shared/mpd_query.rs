use std::any::Any;

use anyhow::Result;
use bon::Builder;
use crossbeam::channel::Sender;
use ratatui::widgets::ListItem;

use super::events::AppEvent;
use crate::{
    config::tabs::PaneTypeDiscriminants,
    mpd::{
        client::Client,
        commands::{Decoder, Output, Song, Status, Volume},
        mpd_client::MpdClient,
    },
    shared::{events::ClientRequest, macros::try_skip},
    ui::panes::browser::DirOrSong,
};

pub const EXTERNAL_COMMAND: &str = "external_command";
pub const GLOBAL_STATUS_UPDATE: &str = "global_status_update";
pub const GLOBAL_VOLUME_UPDATE: &str = "global_volume_update";
pub const GLOBAL_QUEUE_UPDATE: &str = "global_queue_update";

#[derive(derive_more::Debug, Builder)]
pub(crate) struct MpdQuery {
    pub id: &'static str,
    pub replace_id: Option<&'static str>,
    pub target: Option<PaneTypeDiscriminants>,
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
}

impl PreviewGroup {
    pub fn new(name: Option<&'static str>) -> Self {
        Self { name, items: Vec::new() }
    }

    pub fn from(name: Option<&'static str>, items: Vec<ListItem<'static>>) -> Self {
        Self { name, items }
    }

    pub fn push(&mut self, item: ListItem<'static>) {
        self.items.push(item);
    }
}

#[derive(Debug)]
#[allow(unused)]
pub(crate) enum MpdQueryResult {
    Preview { data: Option<Vec<PreviewGroup>>, origin_path: Option<Vec<String>> },
    SongsList { data: Vec<Song>, origin_path: Option<Vec<String>> },
    LsInfo { data: Vec<String>, origin_path: Option<Vec<String>> },
    DirOrSong { data: Vec<DirOrSong>, origin_path: Option<Vec<String>> },
    AddToPlaylist { playlists: Vec<String>, song_file: String },
    AlbumArt(Option<Vec<u8>>),
    Status(Status),
    Queue(Option<Vec<Song>>),
    Volume(Volume),
    Outputs(Vec<Output>),
    Decoders(Vec<Decoder>),
    ExternalCommand(Vec<String>, Vec<Song>),
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
            callback: Box::new(move |client| Ok(MpdQueryResult::Status(client.get_status()?))),
        })),
        "Failed to send status update query"
    );
    Ok(())
}
