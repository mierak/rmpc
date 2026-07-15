use std::time::Duration;

use anyhow::Result;
use enum_map::EnumMap;
use ratatui::{Frame, prelude::Rect, widgets::ListState};
use rmpc_mpd::{
    client::Client,
    commands::Song,
    mpd_client::{MpdClient, MpdCommand, StickerFindOptions, StickerSort},
    proto_client::ProtoClient,
};

use super::Pane;
use crate::{
    MpdQueryResult,
    config::{
        tabs::{PaneType, StickerPaneSort},
        theme::properties::{Property, SongProperty},
    },
    ctx::Ctx,
    shared::{
        events::ClientRequest,
        id::{self, Id},
        keys::ActionEvent,
        macros::{modal, try_skip},
        mouse_event::MouseEvent,
        mpd_query::MpdQuery,
    },
    ui::{
        UiEvent,
        browser::BrowserPane,
        dir_or_song::DirOrSong,
        dirstack::DirStack,
        input::InputResultEvent,
        modals::info_list_modal::{InfoListModal, SongCtx},
        widgets::browser::{Browser, BrowserArea},
    },
};

const INIT: &str = "recently_played_init";
const DEBOUNCE_DELAY: Duration = Duration::from_millis(50);

#[derive(Debug)]
pub struct StickerPane {
    sticker: String,
    sort: StickerPaneSort,
    stack: DirStack<DirOrSong, ListState>,
    browser: Browser<DirOrSong>,
    target_pane: PaneType,
    limit: Option<u32>,
    debounce_id: Id,
    initialized: bool,
    needs_refresh: bool,
}

impl StickerPane {
    pub fn new(
        sticker: String,
        sort: StickerPaneSort,
        target_pane: PaneType,
        format: Vec<Property<SongProperty>>,
        limit: Option<u32>,
        _ctx: &Ctx,
    ) -> Self {
        let browser = if format.is_empty() {
            Browser::new()
        } else {
            Browser::new().with_song_format(format)
        };
        Self {
            sticker,
            sort,
            stack: DirStack::default(),
            browser,
            target_pane,
            limit,
            debounce_id: id::new(),
            initialized: false,
            needs_refresh: false,
        }
    }

    fn fetch(&self, ctx: &Ctx) {
        ctx.query()
            .id(INIT)
            .replace_id(INIT)
            .target(self.target_pane.clone())
            .query(make_fetch_callback(self.sticker.clone(), self.sort, self.limit));
    }

    fn fetch_debounced(&self, ctx: &Ctx) {
        let target = self.target_pane.clone();
        let limit = self.limit;
        let sticker = self.sticker.clone();
        let sort = self.sort;
        ctx.scheduler.schedule_replace(self.debounce_id, DEBOUNCE_DELAY, move |(_, client_tx)| {
            try_skip!(
                client_tx.send(ClientRequest::Query(MpdQuery {
                    id: INIT,
                    replace_id: Some(INIT),
                    target: Some(target),
                    callback: Box::new(make_fetch_callback(sticker, sort, limit)),
                })),
                "Failed to send recently played debounce query"
            );
            Ok(())
        });
    }
}

impl Pane for StickerPane {
    fn render(&mut self, frame: &mut Frame, area: Rect, ctx: &Ctx) -> Result<()> {
        self.browser.render(area, frame.buffer_mut(), &mut self.stack, ctx);
        Ok(())
    }

    fn before_show(&mut self, ctx: &Ctx) -> Result<()> {
        if !self.initialized || self.needs_refresh {
            self.fetch(ctx);
            self.initialized = true;
            self.needs_refresh = false;
        }
        Ok(())
    }

    fn on_event(&mut self, event: &mut UiEvent, is_visible: bool, ctx: &Ctx) -> Result<()> {
        match event {
            UiEvent::Sticker | UiEvent::SongChanged => {
                if is_visible {
                    self.fetch_debounced(ctx);
                } else {
                    self.needs_refresh = true;
                }
            }
            UiEvent::Reconnected => {
                self.initialized = false;
                self.needs_refresh = false;
                self.before_show(ctx)?;
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_action(&mut self, event: &mut ActionEvent, ctx: &mut Ctx) -> Result<()> {
        self.handle_common_action(event, ctx)?;
        self.handle_global_action(event, ctx)?;
        Ok(())
    }

    fn handle_insert_mode(&mut self, kind: InputResultEvent, ctx: &mut Ctx) -> Result<()> {
        BrowserPane::handle_insert_mode(self, kind, ctx)?;
        Ok(())
    }

    fn handle_mouse_event(&mut self, event: MouseEvent, ctx: &Ctx) -> Result<()> {
        self.handle_mouse_action(event, ctx)
    }

    fn on_query_finished(
        &mut self,
        id: &'static str,
        data: MpdQueryResult,
        _is_visible: bool,
        ctx: &Ctx,
    ) -> Result<()> {
        if id != INIT {
            return Ok(());
        }
        let MpdQueryResult::SongsList { data: songs, .. } = data else {
            return Ok(());
        };

        let songs: Vec<DirOrSong> = songs.into_iter().map(DirOrSong::Song).collect();
        self.stack = DirStack::new(songs);
        if let Some(sel) = self.stack.current().selected() {
            self.fetch_data(sel, ctx)?;
        }
        ctx.render()?;
        Ok(())
    }
}

impl BrowserPane<DirOrSong> for StickerPane {
    fn stack(&self) -> &DirStack<DirOrSong, ListState> {
        &self.stack
    }

    fn stack_mut(&mut self) -> &mut DirStack<DirOrSong, ListState> {
        &mut self.stack
    }

    fn browser_areas(&self) -> EnumMap<BrowserArea, Rect> {
        self.browser.areas
    }

    fn list_songs_in_item(
        &self,
        item: DirOrSong,
    ) -> impl FnOnce(&mut Client<'_>) -> Result<Vec<Song>> + Send + Sync + Clone + 'static {
        move |_| {
            Ok(match item {
                DirOrSong::Song(song) => vec![song],
                DirOrSong::Dir { .. } => vec![],
            })
        }
    }

    fn fetch_data(&self, _selected: &DirOrSong, _ctx: &Ctx) -> Result<()> {
        Ok(())
    }

    fn show_info(&self, item: &DirOrSong, ctx: &Ctx) -> Result<()> {
        let DirOrSong::Song(song) = item else {
            return Ok(());
        };
        modal!(
            ctx,
            InfoListModal::builder()
                .items(SongCtx(song, ctx))
                .title("Song info")
                .column_widths(&[30, 70])
                .build()
        );
        Ok(())
    }
}

impl From<StickerPaneSort> for StickerSort {
    fn from(value: StickerPaneSort) -> Self {
        match value {
            StickerPaneSort::Uri => StickerSort::Uri,
            StickerPaneSort::UriDesc => StickerSort::UriDesc,
            StickerPaneSort::ValueIntDesc => StickerSort::ValueIntDesc,
            StickerPaneSort::ValueInt => StickerSort::ValueInt,
            StickerPaneSort::ValueDesc => StickerSort::ValueDesc,
            StickerPaneSort::Value => StickerSort::Value,
        }
    }
}

fn make_fetch_callback(
    sticker: String,
    sort: StickerPaneSort,
    limit: Option<u32>,
) -> impl FnOnce(&mut Client<'_>) -> Result<MpdQueryResult> + Send + 'static {
    move |client| {
        let uris: Vec<String> = client
            .find_stickers("", &sticker, StickerFindOptions {
                filter: None,
                sort: Some(sort.into()),
                window: limit.map(|l| (0, l)),
            })?
            .0
            .into_iter()
            .map(|s| s.file)
            .collect();

        if uris.is_empty() {
            return Ok(MpdQueryResult::SongsList { data: Vec::new(), path: None });
        }

        client.send_start_cmd_list()?;
        for uri in &uris {
            client.send_lsinfo(Some(uri.as_str()))?;
        }
        client.send_execute_cmd_list()?;
        let songs: Vec<Song> = client.read_response()?;

        Ok(MpdQueryResult::SongsList { data: songs, path: None })
    }
}
