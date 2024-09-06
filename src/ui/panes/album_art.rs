use crate::{
    config::keys::ToDescription,
    context::AppContext,
    mpd::mpd_client::MpdClient,
    ui::{image::facade::AlbumArtFacade, KeyHandleResultInternal, UiEvent},
    utils::{image_proto::ImageProtocol, macros::try_skip},
    AppEvent,
};
use anyhow::Result;

use super::Pane;

#[derive(Debug)]
pub struct AlbumArtPane {
    album_art: AlbumArtFacade,
}

impl AlbumArtPane {
    pub fn new(context: &AppContext) -> Self {
        let sender = context.app_event_sender.clone();
        let config = context.config;
        Self {
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
}

pub enum AlbumArtActions {}
impl ToDescription for AlbumArtActions {
    fn to_description(&self) -> &str {
        ""
    }
}

impl Pane for AlbumArtPane {
    fn render(
        &mut self,
        frame: &mut ratatui::Frame,
        area: ratatui::prelude::Rect,
        context: &crate::context::AppContext,
    ) -> anyhow::Result<()> {
        self.album_art.render(frame, area, context.config)?;
        Ok(())
    }

    fn post_render(&mut self, frame: &mut ratatui::Frame, context: &AppContext) -> Result<()> {
        self.album_art.post_render(frame, context.config)?;
        Ok(())
    }

    fn handle_action(
        &mut self,
        _event: crossterm::event::KeyEvent,
        _client: &mut impl crate::mpd::mpd_client::MpdClient,
        _context: &crate::context::AppContext,
    ) -> anyhow::Result<crate::ui::KeyHandleResultInternal> {
        Ok(crate::ui::KeyHandleResultInternal::KeyNotHandled)
    }

    fn on_hide(
        &mut self,
        _client: &mut impl crate::mpd::mpd_client::MpdClient,
        context: &crate::context::AppContext,
    ) -> anyhow::Result<()> {
        self.album_art.hide(context.config.theme.background_color)?;
        Ok(())
    }

    fn before_show(
        &mut self,
        client: &mut impl crate::mpd::mpd_client::MpdClient,
        context: &crate::context::AppContext,
    ) -> anyhow::Result<()> {
        if !matches!(context.config.album_art.method.into(), ImageProtocol::None) {
            let album_art =
                if let Some(current_song) = context.queue.iter().find(|v| Some(v.id) == context.status.songid) {
                    let start = std::time::Instant::now();
                    log::debug!(file = current_song.file.as_str(); "Searching for album art");
                    let result = client.find_album_art(current_song.file.as_str())?;
                    log::debug!(elapsed:? = start.elapsed(), size = result.as_ref().map(|v|v.len()); "Found album art");
                    result
                } else {
                    None
                };
            self.album_art.set_image(album_art)?;
            self.album_art.show();
        }
        Ok(())
    }

    fn on_event(
        &mut self,
        event: &mut UiEvent,
        client: &mut impl MpdClient,
        context: &AppContext,
    ) -> Result<KeyHandleResultInternal> {
        match event {
            UiEvent::Player => {
                if let Some((_, current_song)) = context
                    .queue
                    .iter()
                    .enumerate()
                    .find(|(_, v)| Some(v.id) == context.status.songid)
                {
                    if !matches!(context.config.album_art.method.into(), ImageProtocol::None) {
                        let start = std::time::Instant::now();
                        log::debug!(file = current_song.file.as_str(); "Searching for album art");
                        let album_art = client.find_album_art(current_song.file.as_str())?;
                        log::debug!(elapsed:? = start.elapsed(), size = album_art.as_ref().map(|v|v.len()); "Found album art");
                        self.album_art.set_image(album_art)?;
                        return Ok(KeyHandleResultInternal::RenderRequested);
                    }
                }

                Ok(KeyHandleResultInternal::SkipRender)
            }
            UiEvent::Resized { columns, rows } => {
                self.album_art.resize(*columns, *rows);
                Ok(KeyHandleResultInternal::RenderRequested)
            }
            UiEvent::ModalOpened => {
                self.album_art.hide(context.config.theme.background_color)?;
                Ok(KeyHandleResultInternal::RenderRequested)
            }
            UiEvent::ModalClosed => {
                self.album_art.show();
                Ok(KeyHandleResultInternal::RenderRequested)
            }
            UiEvent::Exit => {
                self.album_art.cleanup()?;
                Ok(KeyHandleResultInternal::SkipRender)
            }
            _ => Ok(KeyHandleResultInternal::SkipRender),
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use rstest::rstest;

    use crate::config::Config;
    use crate::config::Leak;
    use crate::mpd::commands::Song;
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
        let mut screen = AlbumArtPane::new(&app_context);

        screen
            .on_event(&mut UiEvent::Player, &mut client, &app_context)
            .unwrap();

        assert_eq!(
            client.calls.get("find_album_art").map_or(0, |v| *v),
            u32::from(should_search)
        );
    }
}
