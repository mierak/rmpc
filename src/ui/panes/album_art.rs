use anyhow::Result;
use ratatui::{Frame, layout::Rect};

use super::Pane;
use crate::{
    MpdQueryResult,
    config::tabs::PaneType,
    ctx::Ctx,
    mpd::mpd_client::MpdClient,
    shared::{image::ImageProtocol, key_event::KeyEvent},
    ui::{UiEvent, image::facade::AlbumArtFacade},
};

#[derive(Debug)]
pub struct AlbumArtPane {
    album_art: AlbumArtFacade,
    is_modal_open: bool,
    fetch_needed: bool,
}

const ALBUM_ART: &str = "album_art";

impl AlbumArtPane {
    pub fn new(ctx: &Ctx) -> Self {
        Self {
            album_art: AlbumArtFacade::new(&ctx.config),
            is_modal_open: false,
            fetch_needed: false,
        }
    }

    /// returns none if album art is supposed to be hidden
    fn fetch_album_art(ctx: &Ctx) -> Option<()> {
        if matches!(ctx.config.album_art.method.into(), ImageProtocol::None) {
            return None;
        }

        let (_, current_song) = ctx.find_current_song_in_queue()?;

        let disabled_protos = &ctx.config.album_art.disabled_protocols;
        let song_uri = current_song.file.as_str();
        if disabled_protos.iter().any(|proto| song_uri.starts_with(proto)) {
            log::debug!(uri = song_uri; "Not downloading album art because the protocol is disabled");
            return None;
        }

        let song_uri = song_uri.to_owned();
        ctx.query().id(ALBUM_ART).replace_id(ALBUM_ART).target(PaneType::AlbumArt).query(move |client| {
            let start = std::time::Instant::now();
            log::debug!(file = song_uri.as_str(); "Searching for album art");
            let result = client.find_album_art(&song_uri)?;
            log::debug!(elapsed:? = start.elapsed(), size = result.as_ref().map(|v|v.len()); "Found album art");

            Ok(MpdQueryResult::AlbumArt(result))
        });

        Some(())
    }
}

impl Pane for AlbumArtPane {
    fn render(&mut self, _frame: &mut Frame, area: Rect, _ctx: &Ctx) -> Result<()> {
        self.album_art.set_size(area);
        Ok(())
    }

    fn calculate_areas(&mut self, area: Rect, _ctx: &Ctx) -> Result<()> {
        self.album_art.set_size(area);
        Ok(())
    }

    fn handle_action(&mut self, _event: &mut KeyEvent, _ctx: &mut Ctx) -> Result<()> {
        Ok(())
    }

    fn on_hide(&mut self, _ctx: &Ctx) -> Result<()> {
        self.album_art.hide()
    }

    fn resize(&mut self, area: Rect, _ctx: &Ctx) -> Result<()> {
        if self.is_modal_open {
            return Ok(());
        }
        self.album_art.set_size(area);
        self.album_art.show_current()
    }

    fn before_show(&mut self, ctx: &Ctx) -> Result<()> {
        if AlbumArtPane::fetch_album_art(ctx).is_none() {
            self.album_art.show_default()?;
        }
        Ok(())
    }

    fn on_query_finished(
        &mut self,
        id: &'static str,
        data: MpdQueryResult,
        is_visible: bool,
        _ctx: &Ctx,
    ) -> Result<()> {
        if !is_visible || self.is_modal_open {
            return Ok(());
        }
        match (id, data) {
            (ALBUM_ART, MpdQueryResult::AlbumArt(Some(data))) => {
                self.album_art.show(data)?;
            }
            (ALBUM_ART, MpdQueryResult::AlbumArt(None)) => {
                self.album_art.show_default()?;
            }
            _ => {}
        }
        Ok(())
    }

    fn on_event(&mut self, event: &mut UiEvent, is_visible: bool, ctx: &Ctx) -> Result<()> {
        match event {
            UiEvent::SongChanged | UiEvent::Reconnected if is_visible => {
                if self.is_modal_open {
                    self.fetch_needed = true;
                    return Ok(());
                }
                self.before_show(ctx)?;
            }
            UiEvent::Displayed if is_visible => {
                if is_visible && !self.is_modal_open {
                    self.album_art.show_current()?;
                }
            }
            UiEvent::ModalOpened if is_visible => {
                self.is_modal_open = true;
                self.album_art.hide()?;
            }
            UiEvent::ModalClosed if is_visible => {
                self.is_modal_open = false;

                if self.fetch_needed {
                    self.fetch_needed = false;
                    self.before_show(ctx)?;
                    return Ok(());
                }
                self.album_art.show_current()?;
            }
            UiEvent::ConfigChanged => {
                self.album_art.set_config(&ctx.config)?;
                if is_visible && !self.is_modal_open {
                    self.album_art.show_current()?;
                }
            }
            UiEvent::Exit => {
                self.album_art.cleanup()?;
            }
            _ => {}
        }

        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::time::Duration;

    use crossbeam::channel::{Receiver, RecvTimeoutError, Sender};
    use rstest::rstest;

    use super::AlbumArtPane;
    use crate::{
        config::{Config, album_art::ImageMethod, tabs::PaneType},
        mpd::commands::{Song, State},
        shared::{
            events::{ClientRequest, WorkRequest},
            mpd_query::MpdQuery,
        },
        tests::fixtures::{app_context, client_request_channel, work_request_channel},
        ui::{
            UiEvent,
            panes::{Pane, album_art::ALBUM_ART},
        },
    };

    #[rstest]
    #[case(ImageMethod::Kitty, true)]
    #[case(ImageMethod::Unsupported, false)]
    #[case(ImageMethod::None, false)]
    fn searches_for_album_art_before_show(
        #[case] method: ImageMethod,
        #[case] should_search: bool,
        work_request_channel: (Sender<WorkRequest>, Receiver<WorkRequest>),
        client_request_channel: (Sender<ClientRequest>, Receiver<ClientRequest>),
    ) {
        let rx = client_request_channel.1.clone();
        let mut app_context = app_context(work_request_channel, client_request_channel);
        let selected_song_id = 333;
        let mut config = Config::default();
        config.album_art.method = method;
        app_context.config = std::sync::Arc::new(config);
        app_context.queue.push(Song { id: selected_song_id, ..Default::default() });
        app_context.status.songid = Some(selected_song_id);
        app_context.status.state = State::Play;
        let mut screen = AlbumArtPane::new(&app_context);

        screen.before_show(&app_context).unwrap();

        if should_search {
            assert!(matches!(
                rx.recv_timeout(Duration::from_millis(100)).unwrap(),
                ClientRequest::Query(MpdQuery {
                    id: ALBUM_ART,
                    replace_id: Some(ALBUM_ART),
                    target: Some(PaneType::AlbumArt),
                    ..
                })
            ));
        } else {
            assert!(
                rx.recv_timeout(Duration::from_millis(100))
                    .is_err_and(|err| RecvTimeoutError::Timeout == err)
            );
        }
    }

    #[rstest]
    #[case(ImageMethod::Kitty, true)]
    #[case(ImageMethod::Unsupported, false)]
    #[case(ImageMethod::None, false)]
    fn searches_for_album_art_on_event(
        #[case] method: ImageMethod,
        #[case] should_search: bool,
        work_request_channel: (Sender<WorkRequest>, Receiver<WorkRequest>),
        client_request_channel: (Sender<ClientRequest>, Receiver<ClientRequest>),
    ) {
        let rx = client_request_channel.1.clone();
        let mut app_context = app_context(work_request_channel, client_request_channel);
        let selected_song_id = 333;
        let mut config = Config::default();
        config.album_art.method = method;
        app_context.config = std::sync::Arc::new(config);
        app_context.queue.push(Song { id: selected_song_id, ..Default::default() });
        app_context.status.songid = Some(selected_song_id);
        app_context.status.state = State::Play;
        let mut screen = AlbumArtPane::new(&app_context);

        screen.on_event(&mut UiEvent::SongChanged, true, &app_context).unwrap();

        if should_search {
            assert!(matches!(
                rx.recv_timeout(Duration::from_millis(100)).unwrap(),
                ClientRequest::Query(MpdQuery {
                    id: ALBUM_ART,
                    replace_id: Some(ALBUM_ART),
                    target: Some(PaneType::AlbumArt),
                    ..
                })
            ));
        } else {
            let result = rx.recv_timeout(Duration::from_millis(100));
            assert!(result.is_err_and(|err| RecvTimeoutError::Timeout == err));
        }
    }
}
