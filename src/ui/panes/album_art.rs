use crate::{
    context::AppContext,
    mpd::mpd_client::MpdClient,
    shared::{image::ImageProtocol, key_event::KeyEvent, macros::try_skip},
    ui::{image::facade::AlbumArtFacade, UiEvent},
    AppEvent,
};
use anyhow::Result;
use ratatui::{layout::Rect, Frame};

use super::Pane;

#[derive(Debug)]
pub struct AlbumArtPane {
    album_art: AlbumArtFacade,
    image_data: Option<Vec<u8>>,
}

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

    fn fetch_album_art(client: &mut impl MpdClient, context: &AppContext) -> Result<Option<Vec<u8>>> {
        if matches!(context.config.album_art.method.into(), ImageProtocol::None) {
            return Ok(None);
        };

        let Some((_, current_song)) = context.find_current_song_in_queue() else {
            return Ok(None);
        };

        let disabled_protos = &context.config.album_art.disabled_protocols;
        let song_uri = current_song.file.as_str();
        if disabled_protos.iter().any(|proto| song_uri.starts_with(proto)) {
            log::debug!(uri = song_uri; "Not downloading album art because the protocol is disabled");
            return Ok(None);
        }

        let start = std::time::Instant::now();
        log::debug!(file = song_uri; "Searching for album art");
        let result = client.find_album_art(song_uri)?;
        log::debug!(elapsed:? = start.elapsed(), size = result.as_ref().map(|v|v.len()); "Found album art");

        Ok(result)
    }
}

impl Pane for AlbumArtPane {
    fn render(&mut self, frame: &mut Frame, area: Rect, context: &AppContext) -> Result<()> {
        if let Some(data) = self.image_data.take() {
            self.album_art.set_size(area);
            self.album_art.set_image(Some(data))?;
            self.album_art.show();
            self.album_art.render(frame, context.config)?;
        } else {
            self.album_art.set_size(area);
            self.album_art.render(frame, context.config)?;
        }
        Ok(())
    }

    fn post_render(&mut self, frame: &mut ratatui::Frame, context: &AppContext) -> Result<()> {
        self.album_art.post_render(frame, context.config)?;
        Ok(())
    }

    fn handle_action(
        &mut self,
        _event: &mut KeyEvent,
        _client: &mut impl MpdClient,
        _context: &AppContext,
    ) -> Result<()> {
        Ok(())
    }

    fn on_hide(&mut self, _client: &mut impl MpdClient, context: &AppContext) -> Result<()> {
        self.album_art.hide(context.config.theme.background_color)?;
        Ok(())
    }

    fn before_show(&mut self, client: &mut impl MpdClient, context: &AppContext) -> Result<()> {
        self.image_data = AlbumArtPane::fetch_album_art(client, context)?;
        Ok(())
    }

    fn on_event(&mut self, event: &mut UiEvent, client: &mut impl MpdClient, context: &AppContext) -> Result<()> {
        match event {
            UiEvent::SongChanged => {
                self.album_art
                    .set_image(AlbumArtPane::fetch_album_art(client, context)?)?;
                context.render()?;
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
    use rstest::rstest;

    use crate::config::Config;
    use crate::config::Leak;
    use crate::mpd::commands::Song;
    use crate::mpd::commands::State;
    use crate::tests::fixtures::app_context;
    use crate::tests::fixtures::mpd_client::client;
    use crate::tests::fixtures::mpd_client::TestMpdClient;
    use crate::ui::panes::Pane;
    use crate::ui::UiEvent;
    use crate::{config::ImageMethod, context::AppContext};

    use super::AlbumArtPane;

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
        mut app_context: AppContext,
        mut client: TestMpdClient,
    ) {
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

        screen.before_show(&mut client, &app_context).unwrap();

        assert_eq!(
            client.calls.get("find_album_art").map_or(0, |v| *v),
            u32::from(should_search)
        );
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
        mut app_context: AppContext,
        mut client: TestMpdClient,
    ) {
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

        screen
            .on_event(&mut UiEvent::SongChanged, &mut client, &app_context)
            .unwrap();

        assert_eq!(
            client.calls.get("find_album_art").map_or(0, |v| *v),
            u32::from(should_search)
        );
    }
}
