#![allow(clippy::unwrap_used)]

use rstest::{fixture, rstest};

use crate::config::Config;
use crate::mpd::commands::Status;
use crate::tests::fixtures::config;
use crate::tests::fixtures::mpd_client::{client, TestMpdClient};
use crate::tests::fixtures::status;
use crate::ui::UiEvent;

use crate::ui::screens::{browser::DirOrSong, playlists::PlaylistsScreen, Screen};
use crate::ui::screens::{BrowserScreen, CommonAction};

mod on_idle_event {
    use super::*;
    mod browsing_playlists {
        use super::*;

        #[rstest]
        #[case(UiEvent::StoredPlaylist)]
        #[case(UiEvent::Database)]
        fn selects_the_same_playlist_by_name(
            mut status: Status,
            mut screen: PlaylistsScreen,
            config: Config,
            mut client: TestMpdClient,
            #[case] mut event: UiEvent,
        ) {
            let current = screen.stack.current_mut();
            let playlist_name = client.playlists[2].name.clone();
            current.select_idx(2);
            assert_eq!(current.selected(), Some(&DirOrSong::Dir(playlist_name.clone())));

            client.playlists.remove(0);
            screen
                .on_event(&mut event, &mut client, &mut status, Box::leak(Box::new(config)))
                .unwrap();

            assert_eq!(screen.stack.current().selected(), Some(&DirOrSong::Dir(playlist_name)));
        }

        #[rstest]
        #[case(UiEvent::StoredPlaylist)]
        #[case(UiEvent::Database)]
        fn selects_the_same_index_when_playlist_not_found_after_refresh(
            config: Config,
            mut screen: PlaylistsScreen,
            mut status: Status,
            mut client: TestMpdClient,
            #[case] mut event: UiEvent,
        ) {
            screen.stack.current_mut().select_idx(2);

            client.playlists.remove(2);
            screen
                .on_event(&mut event, &mut client, &mut status, Box::leak(Box::new(config)))
                .unwrap();

            assert_eq!(screen.stack.current().selected_with_idx().unwrap().0, 2);
        }

        #[rstest]
        #[case(UiEvent::StoredPlaylist)]
        #[case(UiEvent::Database)]
        fn selects_the_last_playlist_when_last_was_selected_and_removed(
            config: Config,
            mut screen: PlaylistsScreen,
            mut status: Status,
            mut client: TestMpdClient,
            #[case] mut event: UiEvent,
        ) {
            let playlist_count = client.playlists.len();
            screen.stack.current_mut().select_idx(playlist_count - 1);

            client.playlists.pop();
            screen
                .on_event(&mut event, &mut client, &mut status, Box::leak(Box::new(config)))
                .unwrap();

            assert_eq!(
                screen.stack.current().selected_with_idx().unwrap().0,
                playlist_count - 2
            );
        }

        #[rstest]
        #[case(UiEvent::StoredPlaylist)]
        #[case(UiEvent::Database)]
        fn selects_the_first_playlist_when_first_was_selected_and_removed(
            config: Config,
            mut screen: PlaylistsScreen,
            mut client: TestMpdClient,
            mut status: Status,
            #[case] mut event: UiEvent,
        ) {
            screen.stack.current_mut().select_idx(0);

            client.playlists.remove(0);
            screen
                .on_event(&mut event, &mut client, &mut status, Box::leak(Box::new(config)))
                .unwrap();

            assert_eq!(screen.stack.current().selected_with_idx().unwrap().0, 0);
        }
    }

    mod browsing_songs {
        use super::*;

        #[rstest]
        #[case(UiEvent::StoredPlaylist)]
        #[case(UiEvent::Database)]
        fn selects_the_same_playlist_and_song(
            config: Config,
            mut status: Status,
            #[from(screen_in_playlist_2)] mut screen: PlaylistsScreen,
            mut client: TestMpdClient,
            #[case] mut event: UiEvent,
        ) {
            let playlist_name = client.playlists[2].name.clone();
            screen.stack.current_mut().select_idx(5);
            client.playlists[2].songs_indices.remove(0);

            client.playlists.remove(1);
            screen
                .on_event(&mut event, &mut client, &mut status, Box::leak(Box::new(config)))
                .unwrap();

            assert_eq!(screen.stack.previous().selected(), Some(&DirOrSong::Dir(playlist_name)));
            assert_eq!(screen.stack.current().selected_with_idx().unwrap().0, 4);
        }

        #[rstest]
        #[case(UiEvent::StoredPlaylist)]
        #[case(UiEvent::Database)]
        fn selects_the_same_playlist_and_last_song(
            config: Config,
            mut status: Status,
            #[from(screen_in_playlist_2)] mut screen: PlaylistsScreen,
            mut client: TestMpdClient,
            #[case] mut event: UiEvent,
        ) {
            let playlist_name = client.playlists[2].name.clone();
            let last_song_idx = screen.stack.current().items.len() - 1;
            screen.stack.current_mut().select_idx(last_song_idx);
            client.playlists[2].songs_indices.remove(last_song_idx);

            client.playlists.remove(1);
            screen
                .on_event(&mut event, &mut client, &mut status, Box::leak(Box::new(config)))
                .unwrap();

            assert_eq!(screen.stack.previous().selected(), Some(&DirOrSong::Dir(playlist_name)));
            assert_eq!(screen.stack.current().selected_with_idx().unwrap().0, last_song_idx - 1);
        }

        #[rstest]
        #[case(UiEvent::StoredPlaylist)]
        #[case(UiEvent::Database)]
        fn selects_the_same_playlist_and_first_song(
            config: Config,
            mut status: Status,
            #[from(screen_in_playlist_2)] mut screen: PlaylistsScreen,
            mut client: TestMpdClient,
            #[case] mut event: UiEvent,
        ) {
            let playlist_name = client.playlists[2].name.clone();
            screen.stack.current_mut().select_idx(0);
            client.playlists[2].songs_indices.remove(0);

            client.playlists.remove(1);

            screen
                .on_event(&mut event, &mut client, &mut status, Box::leak(Box::new(config)))
                .unwrap();

            assert_eq!(screen.stack.previous().selected(), Some(&DirOrSong::Dir(playlist_name)));
            assert_eq!(screen.stack.current().selected_with_idx().unwrap().0, 0);
        }

        #[rstest]
        #[case(UiEvent::StoredPlaylist)]
        #[case(UiEvent::Database)]
        fn selects_the_same_playlist_and_song_idx(
            config: Config,
            mut status: Status,
            #[from(screen_in_playlist_2)] mut screen: PlaylistsScreen,
            mut client: TestMpdClient,
            #[case] mut event: UiEvent,
        ) {
            screen.stack.current_mut().select_idx(5);

            client.playlists.remove(2);
            screen
                .on_event(&mut event, &mut client, &mut status, Box::leak(Box::new(config)))
                .unwrap();

            assert_eq!(screen.stack.previous().selected_with_idx().unwrap().0, 2);
            assert_eq!(screen.stack.current().selected_with_idx().unwrap().0, 5);
        }

        #[rstest]
        #[case(UiEvent::StoredPlaylist)]
        #[case(UiEvent::Database)]
        fn selects_the_same_playlist_idx_and_last_song(
            config: Config,
            mut status: Status,
            #[from(screen_in_playlist_2)] mut screen: PlaylistsScreen,
            mut client: TestMpdClient,
            #[case] mut event: UiEvent,
        ) {
            let playlist_len = screen.stack.current().items.len();
            screen.stack.current_mut().select_idx(playlist_len - 1);

            client.playlists.remove(2);
            screen
                .on_event(&mut event, &mut client, &mut status, Box::leak(Box::new(config)))
                .unwrap();

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
            config: Config,
            mut status: Status,
            #[from(screen_in_playlist_2)] mut screen: PlaylistsScreen,
            mut client: TestMpdClient,
            #[case] mut event: UiEvent,
        ) {
            screen.stack.current_mut().select_idx(0);

            client.playlists.remove(2);
            screen
                .on_event(&mut event, &mut client, &mut status, Box::leak(Box::new(config)))
                .unwrap();

            assert_eq!(screen.stack.previous().selected_with_idx().unwrap().0, 2);
            assert_eq!(screen.stack.current().selected_with_idx().unwrap().0, 0);
        }

        #[rstest]
        #[case(UiEvent::StoredPlaylist)]
        #[case(UiEvent::Database)]
        fn selects_the_first_playlist_and_same_song_idx(
            config: Config,
            mut status: Status,
            #[from(screen_in_playlist_0)] mut screen: PlaylistsScreen,
            mut client: TestMpdClient,
            #[case] mut event: UiEvent,
        ) {
            screen.stack.current_mut().select_idx(5);

            client.playlists.remove(0);
            screen
                .on_event(&mut event, &mut client, &mut status, Box::leak(Box::new(config)))
                .unwrap();

            assert_eq!(screen.stack.previous().selected_with_idx().unwrap().0, 0);
            assert_eq!(screen.stack.current().selected_with_idx().unwrap().0, 5);
        }

        #[rstest]
        #[case(UiEvent::StoredPlaylist)]
        #[case(UiEvent::Database)]
        fn selects_the_last_playlist_and_same_song_idx(
            config: Config,
            mut status: Status,
            #[from(screen_in_playlist_4)] mut screen: PlaylistsScreen,
            mut client: TestMpdClient,
            #[case] mut event: UiEvent,
        ) {
            let playlist_count = client.playlists.len();
            screen.stack.current_mut().select_idx(5);

            client.playlists.remove(playlist_count - 1);
            screen
                .on_event(&mut event, &mut client, &mut status, Box::leak(Box::new(config)))
                .unwrap();

            assert_eq!(
                screen.stack.previous().selected_with_idx().unwrap().0,
                playlist_count - 2
            );
            assert_eq!(screen.stack.current().selected_with_idx().unwrap().0, 5);
        }
    }
}

#[fixture]
fn screen_in_playlist_0(mut client: TestMpdClient, mut status: Status, config: Config) -> PlaylistsScreen {
    let mut screen = PlaylistsScreen::default();
    screen
        .before_show(&mut client, &mut status, Box::leak(Box::new(config.clone())))
        .unwrap();
    screen.stack.current_mut().select_idx(0);
    screen
        .handle_common_action(CommonAction::Right, &mut client, Box::leak(Box::new(config)))
        .unwrap();
    screen
}

#[fixture]
fn screen_in_playlist_2(mut client: TestMpdClient, mut status: Status, config: Config) -> PlaylistsScreen {
    let mut screen = PlaylistsScreen::default();
    screen
        .before_show(&mut client, &mut status, Box::leak(Box::new(config.clone())))
        .unwrap();
    screen.stack.current_mut().select_idx(2);
    screen
        .handle_common_action(CommonAction::Right, &mut client, Box::leak(Box::new(config)))
        .unwrap();
    screen
}

#[fixture]
fn screen_in_playlist_4(mut client: TestMpdClient, mut status: Status, config: Config) -> PlaylistsScreen {
    let mut screen = PlaylistsScreen::default();
    screen
        .before_show(&mut client, &mut status, Box::leak(Box::new(config.clone())))
        .unwrap();
    screen.stack.current_mut().select_idx(2);
    screen
        .handle_common_action(CommonAction::Right, &mut client, Box::leak(Box::new(config)))
        .unwrap();
    screen
}

#[fixture]
fn screen(mut client: TestMpdClient, mut status: Status, config: Config) -> PlaylistsScreen {
    let mut screen = PlaylistsScreen::default();
    screen
        .before_show(&mut client, &mut status, Box::leak(Box::new(config)))
        .unwrap();
    screen
}
