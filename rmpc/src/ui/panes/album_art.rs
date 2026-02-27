use anyhow::Result;
use ratatui::{Frame, layout::Rect};

use super::Pane;
use crate::{
    MpdQueryResult,
    ctx::Ctx,
    shared::{
        album_art::{self, ALBUM_ART},
        keys::ActionEvent,
    },
    ui::{UiEvent, image::facade::AlbumArtFacade},
};

#[derive(Debug)]
pub struct AlbumArtPane {
    album_art: AlbumArtFacade,
    is_modal_open: bool,
    fetch_needed: bool,
}

impl AlbumArtPane {
    pub fn new(ctx: &Ctx) -> Self {
        Self { album_art: AlbumArtFacade::new(ctx), is_modal_open: false, fetch_needed: false }
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

    fn handle_action(&mut self, _event: &mut ActionEvent, _ctx: &mut Ctx) -> Result<()> {
        Ok(())
    }

    fn on_hide(&mut self, ctx: &Ctx) -> Result<()> {
        self.album_art.hide(ctx)
    }

    fn resize(&mut self, area: Rect, ctx: &Ctx) -> Result<()> {
        if self.is_modal_open {
            return Ok(());
        }
        self.album_art.set_size(area);
        self.album_art.show_current(ctx)
    }

    fn before_show(&mut self, ctx: &Ctx) -> Result<()> {
        if album_art::fetch_album_art(ctx).is_none() {
            self.album_art.show_default(ctx)?;
        }
        Ok(())
    }

    fn on_query_finished(
        &mut self,
        id: &'static str,
        data: MpdQueryResult,
        is_visible: bool,
        ctx: &Ctx,
    ) -> Result<()> {
        if !is_visible || self.is_modal_open {
            return Ok(());
        }
        match (id, data) {
            (ALBUM_ART, MpdQueryResult::AlbumArt(Some(data))) => {
                self.album_art.show(data, ctx)?;
            }
            (ALBUM_ART, MpdQueryResult::AlbumArt(None)) => {
                self.album_art.show_default(ctx)?;
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
                    self.album_art.show_current(ctx)?;
                }
            }
            UiEvent::ModalOpened if is_visible => {
                if !self.is_modal_open {
                    self.album_art.hide(ctx)?;
                }
                self.is_modal_open = true;
            }
            UiEvent::ModalClosed if is_visible => {
                self.is_modal_open = false;

                if self.fetch_needed {
                    self.fetch_needed = false;
                    self.before_show(ctx)?;
                    return Ok(());
                }
                self.album_art.show_current(ctx)?;
            }
            UiEvent::ConfigChanged => {
                if is_visible && !self.is_modal_open {
                    self.album_art.show_current(ctx)?;
                }
            }
            UiEvent::Exit => {
                self.album_art.cleanup()?;
            }
            UiEvent::ImageEncoded { data } => {
                self.album_art.display(std::mem::take(data), ctx)?;
            }
            UiEvent::ImageEncodeFailed { err } => {
                self.album_art.image_processing_failed(err, ctx)?;
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
    use rmpc_mpd::commands::{Song, State};
    use rstest::rstest;

    use super::AlbumArtPane;
    use crate::{
        config::{Config, album_art::ImageMethod, tabs::PaneType},
        shared::{
            events::{AppEvent, ClientRequest, WorkRequest},
            mpd_query::MpdQuery,
        },
        tests::fixtures::{app_event_channel, client_request_channel, ctx, work_request_channel},
        ui::{
            UiEvent,
            panes::{Pane, album_art::ALBUM_ART},
        },
    };

    #[rstest]
    #[case(ImageMethod::Kitty, true)]
    #[case(ImageMethod::None, false)]
    fn searches_for_album_art_before_show(
        #[case] method: ImageMethod,
        #[case] should_search: bool,
        app_event_channel: (Sender<AppEvent>, Receiver<AppEvent>),
        work_request_channel: (Sender<WorkRequest>, Receiver<WorkRequest>),
        client_request_channel: (Sender<ClientRequest>, Receiver<ClientRequest>),
    ) {
        let rx = client_request_channel.1.clone();
        let mut ctx = ctx(app_event_channel, work_request_channel, client_request_channel);
        let selected_song_id = 333;
        let mut config = Config::default();
        config.album_art.method = method;
        ctx.config = std::sync::Arc::new(config);
        ctx.queue.push(Song { id: selected_song_id, ..Default::default() });
        ctx.status.songid = Some(selected_song_id);
        ctx.status.state = State::Play;
        let mut screen = AlbumArtPane::new(&ctx);

        screen.before_show(&ctx).unwrap();

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
    #[case(ImageMethod::None, false)]
    fn searches_for_album_art_on_event(
        #[case] method: ImageMethod,
        #[case] should_search: bool,
        app_event_channel: (Sender<AppEvent>, Receiver<AppEvent>),
        work_request_channel: (Sender<WorkRequest>, Receiver<WorkRequest>),
        client_request_channel: (Sender<ClientRequest>, Receiver<ClientRequest>),
    ) {
        let rx = client_request_channel.1.clone();
        let mut ctx = ctx(app_event_channel, work_request_channel, client_request_channel);
        let selected_song_id = 333;
        let mut config = Config::default();
        config.album_art.method = method;
        ctx.config = std::sync::Arc::new(config);
        ctx.queue.push(Song { id: selected_song_id, ..Default::default() });
        ctx.status.songid = Some(selected_song_id);
        ctx.status.state = State::Play;
        let mut screen = AlbumArtPane::new(&ctx);

        screen.on_event(&mut UiEvent::SongChanged, true, &ctx).unwrap();

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
