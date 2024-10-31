#![allow(clippy::unwrap_used)]

use crossterm::event::{KeyEvent, KeyModifiers};
use rstest::{fixture, rstest};

use crate::context::AppContext;
use crate::tests::fixtures::app_context;
use crate::tests::fixtures::mpd_client::{client, TestMpdClient};
use crate::ui::browser::BrowserPane;
use crate::ui::UiEvent;

use crate::ui::panes::{browser::DirOrSong, playlists::PlaylistsPane, Pane};

mod on_idle_event {
    use super::*;
    use crate::context::AppContext;
    mod browsing_playlists {

        use super::*;

        #[rstest]
        #[case(UiEvent::StoredPlaylist)]
        #[case(UiEvent::Database)]
        fn selects_the_same_playlist_by_name(
            mut screen: PlaylistsPane,
            mut client: TestMpdClient,
            app_context: AppContext,
            #[case] mut event: UiEvent,
        ) {
            let current = screen.stack.current_mut();
            let playlist_name = client.playlists[2].name.clone();
            current.select_idx(2, 0);
            assert_eq!(
                current.selected(),
                Some(&DirOrSong::Dir {
                    name: playlist_name.clone(),
                    full_path: String::new()
                })
            );

            client.playlists.remove(0);
            screen.on_event(&mut event, &mut client, &app_context).unwrap();

            assert_eq!(
                screen.stack.current().selected(),
                Some(&DirOrSong::Dir {
                    name: playlist_name,
                    full_path: String::new()
                })
            );
        }

        #[rstest]
        #[case(UiEvent::StoredPlaylist)]
        #[case(UiEvent::Database)]
        fn selects_the_same_index_when_playlist_not_found_after_refresh(
            mut screen: PlaylistsPane,
            mut client: TestMpdClient,
            app_context: AppContext,
            #[case] mut event: UiEvent,
        ) {
            screen.stack.current_mut().select_idx(2, 0);

            client.playlists.remove(2);
            screen.on_event(&mut event, &mut client, &app_context).unwrap();

            assert_eq!(screen.stack.current().selected_with_idx().unwrap().0, 2);
        }

        #[rstest]
        #[case(UiEvent::StoredPlaylist)]
        #[case(UiEvent::Database)]
        fn selects_the_last_playlist_when_last_was_selected_and_removed(
            mut screen: PlaylistsPane,
            mut client: TestMpdClient,
            app_context: AppContext,
            #[case] mut event: UiEvent,
        ) {
            let playlist_count = client.playlists.len();
            screen.stack.current_mut().select_idx(playlist_count - 1, 0);

            client.playlists.pop();
            screen.on_event(&mut event, &mut client, &app_context).unwrap();

            assert_eq!(
                screen.stack.current().selected_with_idx().unwrap().0,
                playlist_count - 2
            );
        }

        #[rstest]
        #[case(UiEvent::StoredPlaylist)]
        #[case(UiEvent::Database)]
        fn selects_the_first_playlist_when_first_was_selected_and_removed(
            mut screen: PlaylistsPane,
            mut client: TestMpdClient,
            app_context: AppContext,
            #[case] mut event: UiEvent,
        ) {
            screen.stack.current_mut().select_idx(0, 0);

            client.playlists.remove(0);
            screen.on_event(&mut event, &mut client, &app_context).unwrap();

            assert_eq!(screen.stack.current().selected_with_idx().unwrap().0, 0);
        }
    }

    mod browsing_songs {
        use super::*;

        #[rstest]
        #[case(UiEvent::StoredPlaylist)]
        #[case(UiEvent::Database)]
        fn selects_the_same_playlist_and_song(
            #[from(screen_in_playlist_2)] mut screen: PlaylistsPane,
            mut client: TestMpdClient,
            #[case] mut event: UiEvent,
            app_context: AppContext,
        ) {
            let playlist_name = client.playlists[2].name.clone();
            screen.stack.current_mut().select_idx(5, 0);
            client.playlists[2].songs_indices.remove(0);

            client.playlists.remove(1);
            screen.on_event(&mut event, &mut client, &app_context).unwrap();

            assert_eq!(
                screen.stack.previous().selected(),
                Some(&DirOrSong::Dir {
                    name: playlist_name,
                    full_path: String::new()
                })
            );
            assert_eq!(screen.stack.current().selected_with_idx().unwrap().0, 4);
        }

        #[rstest]
        #[case(UiEvent::StoredPlaylist)]
        #[case(UiEvent::Database)]
        fn selects_the_same_playlist_and_last_song(
            #[from(screen_in_playlist_2)] mut screen: PlaylistsPane,
            mut client: TestMpdClient,
            #[case] mut event: UiEvent,
            app_context: AppContext,
        ) {
            let playlist_name = client.playlists[2].name.clone();
            let last_song_idx = screen.stack.current().items.len() - 1;
            screen.stack.current_mut().select_idx(last_song_idx, 0);
            client.playlists[2].songs_indices.remove(last_song_idx);

            client.playlists.remove(1);
            screen.on_event(&mut event, &mut client, &app_context).unwrap();

            assert_eq!(
                screen.stack.previous().selected(),
                Some(&DirOrSong::Dir {
                    name: playlist_name,
                    full_path: String::new()
                })
            );
            assert_eq!(screen.stack.current().selected_with_idx().unwrap().0, last_song_idx - 1);
        }

        #[rstest]
        #[case(UiEvent::StoredPlaylist)]
        #[case(UiEvent::Database)]
        fn selects_the_same_playlist_and_first_song(
            app_context: AppContext,
            #[from(screen_in_playlist_2)] mut screen: PlaylistsPane,
            mut client: TestMpdClient,
            #[case] mut event: UiEvent,
        ) {
            let playlist_name = client.playlists[2].name.clone();
            screen.stack.current_mut().select_idx(0, 0);
            client.playlists[2].songs_indices.remove(0);

            client.playlists.remove(1);

            screen.on_event(&mut event, &mut client, &app_context).unwrap();

            assert_eq!(
                screen.stack.previous().selected(),
                Some(&DirOrSong::Dir {
                    name: playlist_name,
                    full_path: String::new()
                })
            );
            assert_eq!(screen.stack.current().selected_with_idx().unwrap().0, 0);
        }

        #[rstest]
        #[case(UiEvent::StoredPlaylist)]
        #[case(UiEvent::Database)]
        fn selects_the_same_playlist_and_song_idx(
            app_context: AppContext,
            #[from(screen_in_playlist_2)] mut screen: PlaylistsPane,
            mut client: TestMpdClient,
            #[case] mut event: UiEvent,
        ) {
            screen.stack.current_mut().select_idx(5, 0);

            client.playlists.remove(2);
            screen.on_event(&mut event, &mut client, &app_context).unwrap();

            assert_eq!(screen.stack.previous().selected_with_idx().unwrap().0, 2);
            assert_eq!(screen.stack.current().selected_with_idx().unwrap().0, 5);
        }

        #[rstest]
        #[case(UiEvent::StoredPlaylist)]
        #[case(UiEvent::Database)]
        fn selects_the_same_playlist_idx_and_last_song(
            app_context: AppContext,
            #[from(screen_in_playlist_2)] mut screen: PlaylistsPane,
            mut client: TestMpdClient,
            #[case] mut event: UiEvent,
        ) {
            let playlist_len = screen.stack.current().items.len();
            screen.stack.current_mut().select_idx(playlist_len - 1, 0);

            client.playlists.remove(2);
            screen.on_event(&mut event, &mut client, &app_context).unwrap();

            assert_eq!(screen.stack.previous().selected_with_idx().unwrap().0, 2);
            assert_eq!(
                screen.stack.current().selected_with_idx().unwrap().0,
                screen.stack.current().items.len() - 1
            );
        }

        #[rstest]
        #[case(UiEvent::StoredPlaylist)]
        #[case(UiEvent::Database)]
        fn selects_the_same_playlist_idx_and_first_song(
            app_context: AppContext,
            #[from(screen_in_playlist_2)] mut screen: PlaylistsPane,
            mut client: TestMpdClient,
            #[case] mut event: UiEvent,
        ) {
            screen.stack.current_mut().select_idx(0, 0);

            client.playlists.remove(2);
            screen.on_event(&mut event, &mut client, &app_context).unwrap();

            assert_eq!(screen.stack.previous().selected_with_idx().unwrap().0, 2);
            assert_eq!(screen.stack.current().selected_with_idx().unwrap().0, 0);
        }

        #[rstest]
        #[case(UiEvent::StoredPlaylist)]
        #[case(UiEvent::Database)]
        fn selects_the_first_playlist_and_same_song_idx(
            app_context: AppContext,
            #[from(screen_in_playlist_0)] mut screen: PlaylistsPane,
            mut client: TestMpdClient,
            #[case] mut event: UiEvent,
        ) {
            screen.stack.current_mut().select_idx(5, 0);

            client.playlists.remove(0);
            screen.on_event(&mut event, &mut client, &app_context).unwrap();

            assert_eq!(screen.stack.previous().selected_with_idx().unwrap().0, 0);
            assert_eq!(screen.stack.current().selected_with_idx().unwrap().0, 5);
        }

        #[rstest]
        #[case(UiEvent::StoredPlaylist)]
        #[case(UiEvent::Database)]
        fn selects_the_last_playlist_and_same_song_idx(
            app_context: AppContext,
            #[from(screen_in_playlist_4)] mut screen: PlaylistsPane,
            mut client: TestMpdClient,
            #[case] mut event: UiEvent,
        ) {
            let playlist_count = client.playlists.len();
            screen.stack.current_mut().select_idx(5, 0);

            client.playlists.remove(playlist_count - 1);
            screen.on_event(&mut event, &mut client, &app_context).unwrap();

            assert_eq!(
                screen.stack.previous().selected_with_idx().unwrap().0,
                playlist_count - 2
            );
            assert_eq!(screen.stack.current().selected_with_idx().unwrap().0, 5);
        }
    }
}

#[fixture]
fn screen_in_playlist_0(mut client: TestMpdClient, app_context: AppContext) -> PlaylistsPane {
    let mut screen = PlaylistsPane::new(&app_context);
    screen.before_show(&mut client, &app_context).unwrap();
    screen.stack.current_mut().select_idx(0, 0);
    let right = KeyEvent::new(crossterm::event::KeyCode::Char('l'), KeyModifiers::NONE);
    screen
        .handle_common_action(&mut right.into(), &mut client, &app_context)
        .unwrap();
    screen
}

#[fixture]
fn screen_in_playlist_2(mut client: TestMpdClient, app_context: AppContext) -> PlaylistsPane {
    let mut screen = PlaylistsPane::new(&app_context);
    screen.before_show(&mut client, &app_context).unwrap();
    screen.stack.current_mut().select_idx(2, 0);
    let right = KeyEvent::new(crossterm::event::KeyCode::Char('l'), KeyModifiers::NONE);
    screen
        .handle_common_action(&mut right.into(), &mut client, &app_context)
        .unwrap();
    screen
}

#[fixture]
fn screen_in_playlist_4(mut client: TestMpdClient, app_context: AppContext) -> PlaylistsPane {
    let mut screen = PlaylistsPane::new(&app_context);
    screen.before_show(&mut client, &app_context).unwrap();
    screen.stack.current_mut().select_idx(2, 0);
    let right = KeyEvent::new(crossterm::event::KeyCode::Char('l'), KeyModifiers::NONE);
    screen
        .handle_common_action(&mut right.into(), &mut client, &app_context)
        .unwrap();
    screen
}

#[fixture]
fn screen(mut client: TestMpdClient, app_context: AppContext) -> PlaylistsPane {
    let mut screen = PlaylistsPane::new(&app_context);
    screen.before_show(&mut client, &app_context).unwrap();
    screen
}
