use std::any::Any;

use crate::{
    config::tabs::PaneType,
    mpd::{
        client::Client,
        commands::{Decoder, Output, Song, Status, Volume},
    },
    ui::panes::browser::DirOrSong,
};
use anyhow::Result;
use bon::Builder;
use crossbeam::channel::Sender;
use ratatui::widgets::ListItem;

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

        return self.id == other.id && self_replace_id == other_replace_id && self.target == other.target;
    }
}

#[derive(Debug)]
#[allow(unused)]
pub(crate) enum MpdQueryResult {
    Preview {
        data: Option<Vec<ListItem<'static>>>,
        origin_path: Option<Vec<String>>,
    },
    SongsList {
        data: Vec<Song>,
        origin_path: Option<Vec<String>>,
    },
    LsInfo {
        data: Vec<String>,
        origin_path: Option<Vec<String>>,
    },
    DirOrSong {
        data: Vec<DirOrSong>,
        origin_path: Option<Vec<String>>,
    },
    AddToPlaylist {
        playlists: Vec<String>,
        song_file: String,
    },
    AlbumArt(Option<Vec<u8>>),
    Status(Status),
    Queue(Option<Vec<Song>>),
    Volume(Volume),
    Outputs(Vec<Output>),
    Decoders(Vec<Decoder>),
    ExternalCommand(&'static [&'static str], Vec<Song>),
    Any(Box<dyn Any + Send + Sync>),
}
