use crate::{
    config::tabs::PaneType,
    context::AppContext,
    mpd::mpd_client::MpdClient,
    shared::{image::ImageProtocol, key_event::KeyEvent, macros::try_skip},
    ui::{image::facade::AlbumArtFacade, UiEvent},
    AppEvent, MpdQueryResult,
};
use anyhow::Result;
use ratatui::{layout::Rect, Frame};

use super::Pane;

#[derive(Debug)]
pub struct AlbumArtPane {
    album_art: AlbumArtFacade,
    image_data: Option<Vec<u8>>,
}

const ALBUM_ART: &str = "album_art";

impl AlbumArtPane {
    pub fn new(context: &AppContext) -> Self {
        let sender = context.app_event_sender.clone();
        let config = context.config;
        Self {
            image_data: None,
            album_art: AlbumArtFacade::new(
                config.album_art.method.into(),
                config.theme.default_album_art,
                config.album_art.max_size_px,
                move |full_render: bool| {
                    try_skip!(
                        sender.send(AppEvent::RequestRender(full_render)),
                        "Failed to request render"
                    );
                },
            ),
        }
    }

    /// returns none if album art is supposed to be hidden
    fn fetch_album_art(context: &AppContext) -> Option<()> {
        if matches!(context.config.album_art.method.into(), ImageProtocol::None) {
            return None;
        };

        let (_, current_song) = context.find_current_song_in_queue()?;

        let disabled_protos = &context.config.album_art.disabled_protocols;
        let song_uri = current_song.file.as_str();
        if disabled_protos.iter().any(|proto| song_uri.starts_with(proto)) {
            log::debug!(uri = song_uri; "Not downloading album art because the protocol is disabled");
            return None;
        }

        let song_uri = song_uri.to_owned();
        context
            .query()
            .id(ALBUM_ART)
            .replace_id(ALBUM_ART)
            .target(PaneType::AlbumArt)
            .query(move |client| {
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
    fn render(&mut self, frame: &mut Frame, area: Rect, context: &AppContext) -> Result<()> {
        self.album_art.set_size(area);
        if let Some(data) = self.image_data.take() {
            self.album_art.set_image(Some(data))?;
            self.album_art.show();
            self.album_art.render(frame, context.config)?;
        } else {
            self.album_art.render(frame, context.config)?;
        }
        Ok(())
    }

    fn post_render(&mut self, frame: &mut ratatui::Frame, context: &AppContext) -> Result<()> {
        self.album_art.post_render(frame, context.config)?;
        Ok(())
    }

    fn handle_action(&mut self, _event: &mut KeyEvent, _context: &mut AppContext) -> Result<()> {
        Ok(())
    }

    fn on_hide(&mut self, context: &AppContext) -> Result<()> {
        self.album_art.hide(context.config.theme.background_color)?;
        Ok(())
    }

    fn before_show(&mut self, context: &AppContext) -> Result<()> {
        if AlbumArtPane::fetch_album_art(context).is_none() {
            self.album_art.set_image(None)?;
        }
        Ok(())
    }

    fn on_query_finished(&mut self, id: &'static str, data: MpdQueryResult, context: &AppContext) -> Result<()> {
        match (id, data) {
            (ALBUM_ART, MpdQueryResult::AlbumArt(data)) => {
                self.image_data = data;
                context.render()?;
            }
            _ => {}
        }
        Ok(())
    }

    fn on_event(&mut self, event: &mut UiEvent, context: &AppContext) -> Result<()> {
        match event {
            UiEvent::SongChanged => {
                if AlbumArtPane::fetch_album_art(context).is_none() {
                    self.album_art.set_image(None)?;
                }
            }
            UiEvent::Resized { columns, rows } => {
                self.album_art.resize(*columns, *rows);

                context.render()?;
            }
            UiEvent::ModalOpened => {
                self.album_art.hide(context.config.theme.background_color)?;

                context.render()?;
            }
            UiEvent::ModalClosed => {
                self.album_art.show();

                context.render()?;
            }
            UiEvent::Exit => {
                self.album_art.cleanup()?;
            }
            _ => {}
        };

        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use crossbeam::channel::RecvTimeoutError;
    use crossbeam::channel::{Receiver, Sender};
    use rstest::rstest;
    use std::time::Duration;

    use super::AlbumArtPane;

    use crate::config::Config;
    use crate::config::ImageMethod;
    use crate::config::Leak;
    use crate::mpd::commands::Song;
    use crate::mpd::commands::State;
    use crate::shared::events::WorkRequest;
    use crate::shared::mpd_query::MpdQuery;
    use crate::tests::fixtures::app_context;
    use crate::tests::fixtures::work_request_channel;
    use crate::ui::panes::Pane;
    use crate::ui::UiEvent;
    use crate::{config::tabs::PaneType, ui::panes::album_art::ALBUM_ART};

    #[rstest]
    #[case(ImageMethod::Kitty, true)]
    #[case(ImageMethod::UeberzugWayland, true)]
    #[case(ImageMethod::UeberzugX11, true)]
    #[case(ImageMethod::Iterm2, true)]
    #[case(ImageMethod::Sixel, true)]
    #[case(ImageMethod::Unsupported, false)]
    #[case(ImageMethod::None, false)]
    fn searches_for_album_art_before_show(
        #[case] method: ImageMethod,
        #[case] should_search: bool,
        work_request_channel: (Sender<WorkRequest>, Receiver<WorkRequest>),
    ) {
        let rx = work_request_channel.1.clone();
        let mut app_context = app_context(work_request_channel);
        let selected_song_id = 333;
        let mut config = Config::default();
        config.album_art.method = method;
        app_context.config = config.leak();
        app_context.queue.push(Song {
            id: selected_song_id,
            ..Default::default()
        });
        app_context.status.songid = Some(selected_song_id);
        app_context.status.state = State::Play;
        let mut screen = AlbumArtPane::new(&app_context);

        screen.before_show(&app_context).unwrap();

        if should_search {
            assert!(matches!(
                rx.recv_timeout(Duration::from_millis(100)).unwrap(),
                WorkRequest::MpdQuery(MpdQuery {
                    id: ALBUM_ART,
                    replace_id: Some(ALBUM_ART),
                    target: Some(PaneType::AlbumArt),
                    ..
                })
            ));
        } else {
            assert!(rx
                .recv_timeout(Duration::from_millis(100))
                .is_err_and(|err| RecvTimeoutError::Timeout == err));
        }
    }

    #[rstest]
    #[case(ImageMethod::Kitty, true)]
    #[case(ImageMethod::UeberzugWayland, true)]
    #[case(ImageMethod::UeberzugX11, true)]
    #[case(ImageMethod::Iterm2, true)]
    #[case(ImageMethod::Sixel, true)]
    #[case(ImageMethod::Unsupported, false)]
    #[case(ImageMethod::None, false)]
    fn searches_for_album_art_on_event(
        #[case] method: ImageMethod,
        #[case] should_search: bool,
        work_request_channel: (Sender<WorkRequest>, Receiver<WorkRequest>),
    ) {
        let rx = work_request_channel.1.clone();
        let mut app_context = app_context(work_request_channel);
        let selected_song_id = 333;
        let mut config = Config::default();
        config.album_art.method = method;
        app_context.config = config.leak();
        app_context.queue.push(Song {
            id: selected_song_id,
            ..Default::default()
        });
        app_context.status.songid = Some(selected_song_id);
        app_context.status.state = State::Play;
        let mut screen = AlbumArtPane::new(&app_context);

        screen.on_event(&mut UiEvent::SongChanged, &app_context).unwrap();

        if should_search {
            assert!(matches!(
                rx.recv_timeout(Duration::from_millis(100)).unwrap(),
                WorkRequest::MpdQuery(MpdQuery {
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
