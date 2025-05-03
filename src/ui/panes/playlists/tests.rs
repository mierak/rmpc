#![allow(clippy::unwrap_used)]

use std::{
    collections::HashMap,
    sync::{
        LazyLock,
        atomic::{AtomicU32, Ordering},
    },
    time::Duration,
};

use rstest::{fixture, rstest};

use crate::{
    context::AppContext,
    mpd::commands::Song,
    tests::fixtures::app_context,
    ui::{
        browser::BrowserPane,
        panes::{Pane, browser::DirOrSong, playlists::PlaylistsPane},
    },
};

mod on_idle_event {
    use super::*;
    use crate::{
        context::AppContext,
        shared::mpd_query::MpdQueryResult,
        ui::panes::playlists::{INIT, OPEN_OR_PLAY, REINIT},
    };

    mod browsing_playlists {

        use super::*;

        #[rstest]
        fn selects_the_same_playlist_by_name(mut screen: PlaylistsPane, app_context: AppContext) {
            screen
                .on_query_finished(
                    INIT,
                    MpdQueryResult::DirOrSong {
                        data: vec![dir("pl1"), dir("pl2"), dir("pl3"), dir("pl4")],
                        origin_path: None,
                    },
                    true,
                    &app_context,
                )
                .unwrap();
            let current = screen.stack.current_mut();
            current.select_idx(1, 0);
            assert_eq!(current.selected(), Some(dir("pl2")).as_ref());

            screen
                .on_query_finished(
                    REINIT,
                    MpdQueryResult::DirOrSong {
                        data: vec![dir("pl2"), dir("pl4")],
                        origin_path: None,
                    },
                    true,
                    &app_context,
                )
                .unwrap();

            assert_eq!(screen.stack.current().selected(), Some(dir("pl2")).as_ref());
        }

        #[rstest]
        fn selects_the_same_index_when_playlist_not_found_after_refresh(
            mut screen: PlaylistsPane,
            app_context: AppContext,
        ) {
            screen
                .on_query_finished(
                    INIT,
                    MpdQueryResult::DirOrSong {
                        data: vec![dir("pl1"), dir("pl2"), dir("pl3"), dir("pl4")],
                        origin_path: None,
                    },
                    true,
                    &app_context,
                )
                .unwrap();
            screen.stack.current_mut().select_idx(2, 0);

            screen
                .on_query_finished(
                    REINIT,
                    MpdQueryResult::DirOrSong {
                        data: vec![dir("pl1"), dir("pl2"), dir("pl4")],
                        origin_path: None,
                    },
                    true,
                    &app_context,
                )
                .unwrap();

            assert_eq!(screen.stack.current().selected_with_idx().unwrap().0, 2);
        }

        #[rstest]
        fn selects_the_last_playlist_when_last_was_selected_and_removed(
            mut screen: PlaylistsPane,
            app_context: AppContext,
        ) {
            screen
                .on_query_finished(
                    INIT,
                    MpdQueryResult::DirOrSong {
                        data: vec![dir("pl1"), dir("pl2"), dir("pl3"), dir("pl4")],
                        origin_path: None,
                    },
                    true,
                    &app_context,
                )
                .unwrap();
            screen.stack.current_mut().select_idx(3, 0);

            screen
                .on_query_finished(
                    REINIT,
                    MpdQueryResult::DirOrSong {
                        data: vec![dir("pl1"), dir("pl2")],
                        origin_path: None,
                    },
                    true,
                    &app_context,
                )
                .unwrap();

            assert_eq!(screen.stack.current().selected_with_idx().unwrap().0, 1);
        }

        #[rstest]
        fn selects_the_first_playlist_when_first_was_selected_and_removed(
            mut screen: PlaylistsPane,
            app_context: AppContext,
        ) {
            screen
                .on_query_finished(
                    INIT,
                    MpdQueryResult::DirOrSong {
                        data: vec![dir("pl1"), dir("pl2"), dir("pl3"), dir("pl4")],
                        origin_path: None,
                    },
                    true,
                    &app_context,
                )
                .unwrap();
            screen.stack.current_mut().select_idx(0, 0);
            screen
                .on_query_finished(
                    INIT,
                    MpdQueryResult::DirOrSong {
                        data: vec![dir("pl3"), dir("pl4")],
                        origin_path: None,
                    },
                    true,
                    &app_context,
                )
                .unwrap();

            assert_eq!(screen.stack.current().selected_with_idx().unwrap().0, 0);
        }
    }

    mod browsing_songs {
        use crossbeam::channel::{Receiver, Sender};

        use super::*;
        use crate::{
            shared::events::{ClientRequest, WorkRequest},
            tests::fixtures::{client_request_channel, work_request_channel},
        };

        #[rstest]
        fn selects_the_same_playlist_and_song(
            mut screen: PlaylistsPane,
            work_request_channel: (Sender<WorkRequest>, Receiver<WorkRequest>),
            client_request_channel: (Sender<ClientRequest>, Receiver<ClientRequest>),
        ) {
            let rx = client_request_channel.1.clone();
            let app_context = app_context(work_request_channel, client_request_channel);
            let initial_songs = vec![song("s1"), song("s2"), song("s3"), song("s4")];
            screen
                .on_query_finished(
                    INIT,
                    MpdQueryResult::DirOrSong {
                        data: vec![dir("pl1"), dir("pl2"), dir("pl3"), dir("pl4")],
                        origin_path: None,
                    },
                    true,
                    &app_context,
                )
                .unwrap();
            screen.stack.current_mut().select_idx(2, 0);
            screen.stack_mut().push(Vec::new());
            screen
                .on_query_finished(
                    OPEN_OR_PLAY,
                    MpdQueryResult::SongsList { data: initial_songs.clone(), origin_path: None },
                    true,
                    &app_context,
                )
                .unwrap();
            screen.stack.current_mut().select_idx(2, 0);
            assert_eq!(
                screen.stack.current().selected(),
                Some(&DirOrSong::Song(initial_songs[2].clone()))
            );

            while rx.recv_timeout(Duration::from_millis(1)).is_ok() {}

            // then
            let rx2 = rx.clone();
            let new_songs = vec![song("s2"), song("s3"), song("s4")];
            let new_songs2 = new_songs.clone();
            std::thread::spawn(move || {
                let req = rx2.recv().unwrap();
                if let ClientRequest::QuerySync(qry) = req {
                    qry.tx.send(MpdQueryResult::Any(Box::new(new_songs2))).unwrap();
                }
            });
            screen
                .on_query_finished(
                    REINIT,
                    MpdQueryResult::DirOrSong {
                        data: vec![dir("pl2"), dir("pl3"), dir("pl4")],
                        origin_path: None,
                    },
                    true,
                    &app_context,
                )
                .unwrap();
            assert_eq!(screen.stack.previous().selected(), Some(&dir("pl3")));
            assert_eq!(
                screen.stack.current().selected(),
                Some(&DirOrSong::Song(new_songs[1].clone()))
            );
        }

        #[rstest]
        fn selects_the_same_playlist_and_last_song(
            mut screen: PlaylistsPane,
            work_request_channel: (Sender<WorkRequest>, Receiver<WorkRequest>),
            client_request_channel: (Sender<ClientRequest>, Receiver<ClientRequest>),
        ) {
            let rx = client_request_channel.1.clone();
            let app_context = app_context(work_request_channel, client_request_channel);
            let initial_songs = vec![song("s1"), song("s2"), song("s3"), song("s4")];
            screen
                .on_query_finished(
                    INIT,
                    MpdQueryResult::DirOrSong {
                        data: vec![dir("pl1"), dir("pl2"), dir("pl3"), dir("pl4")],
                        origin_path: None,
                    },
                    true,
                    &app_context,
                )
                .unwrap();
            screen.stack.current_mut().select_idx(2, 0);
            screen.stack_mut().push(Vec::new());
            screen
                .on_query_finished(
                    OPEN_OR_PLAY,
                    MpdQueryResult::SongsList { data: initial_songs.clone(), origin_path: None },
                    true,
                    &app_context,
                )
                .unwrap();
            screen.stack.current_mut().select_idx(2, 0);
            assert_eq!(
                screen.stack.current().selected(),
                Some(&DirOrSong::Song(initial_songs[2].clone()))
            );
            while rx.recv_timeout(Duration::from_millis(1)).is_ok() {}

            // then
            let rx2 = rx.clone();
            let new_songs = vec![song("s1"), song("s2")];
            let new_songs2 = new_songs.clone();
            std::thread::spawn(move || {
                let req = rx2.recv().unwrap();
                if let ClientRequest::QuerySync(qry) = req {
                    qry.tx.send(MpdQueryResult::Any(Box::new(new_songs2))).unwrap();
                }
            });
            screen
                .on_query_finished(
                    REINIT,
                    MpdQueryResult::DirOrSong {
                        data: vec![dir("pl1"), dir("pl2"), dir("pl3"), dir("pl4")],
                        origin_path: None,
                    },
                    true,
                    &app_context,
                )
                .unwrap();
            assert_eq!(screen.stack.previous().selected(), Some(&dir("pl3")));

            assert_eq!(
                screen.stack.current().selected(),
                Some(&DirOrSong::Song(new_songs[1].clone()))
            );
        }

        #[rstest]
        fn selects_the_same_playlist_and_first_song(
            mut screen: PlaylistsPane,
            work_request_channel: (Sender<WorkRequest>, Receiver<WorkRequest>),
            client_request_channel: (Sender<ClientRequest>, Receiver<ClientRequest>),
        ) {
            let rx = client_request_channel.1.clone();
            let app_context = app_context(work_request_channel, client_request_channel);
            let initial_songs = vec![song("s1"), song("s2"), song("s3"), song("s4")];
            screen
                .on_query_finished(
                    INIT,
                    MpdQueryResult::DirOrSong {
                        data: vec![dir("pl1"), dir("pl2"), dir("pl3"), dir("pl4")],
                        origin_path: None,
                    },
                    true,
                    &app_context,
                )
                .unwrap();
            screen.stack.current_mut().select_idx(2, 0);
            screen.stack_mut().push(Vec::new());
            screen
                .on_query_finished(
                    OPEN_OR_PLAY,
                    MpdQueryResult::SongsList { data: initial_songs.clone(), origin_path: None },
                    true,
                    &app_context,
                )
                .unwrap();
            screen.stack.current_mut().select_idx(2, 0);
            assert_eq!(
                screen.stack.current().selected(),
                Some(&DirOrSong::Song(initial_songs[2].clone()))
            );
            while rx.recv_timeout(Duration::from_millis(1)).is_ok() {}

            // then
            let rx2 = rx.clone();
            let new_songs = vec![song("s3"), song("s4")];
            let new_songs2 = new_songs.clone();
            std::thread::spawn(move || {
                let req = rx2.recv().unwrap();
                if let ClientRequest::QuerySync(qry) = req {
                    qry.tx.send(MpdQueryResult::Any(Box::new(new_songs2))).unwrap();
                }
            });
            screen
                .on_query_finished(
                    REINIT,
                    MpdQueryResult::DirOrSong {
                        data: vec![dir("pl1"), dir("pl2"), dir("pl3"), dir("pl4")],
                        origin_path: None,
                    },
                    true,
                    &app_context,
                )
                .unwrap();
            assert_eq!(screen.stack.previous().selected(), Some(&dir("pl3")));
            assert_eq!(
                screen.stack.current().selected(),
                Some(&DirOrSong::Song(new_songs[0].clone()))
            );
        }

        #[rstest]
        fn selects_the_same_playlist_and_song_idx(
            mut screen: PlaylistsPane,
            work_request_channel: (Sender<WorkRequest>, Receiver<WorkRequest>),
            client_request_channel: (Sender<ClientRequest>, Receiver<ClientRequest>),
        ) {
            let rx = client_request_channel.1.clone();
            let app_context = app_context(work_request_channel, client_request_channel);
            let initial_songs = vec![song("s1"), song("s2"), song("s3"), song("s4")];
            screen
                .on_query_finished(
                    INIT,
                    MpdQueryResult::DirOrSong {
                        data: vec![dir("pl1"), dir("pl2"), dir("pl3"), dir("pl4")],
                        origin_path: None,
                    },
                    true,
                    &app_context,
                )
                .unwrap();
            screen.stack.current_mut().select_idx(2, 0);
            screen.stack_mut().push(Vec::new());
            screen
                .on_query_finished(
                    OPEN_OR_PLAY,
                    MpdQueryResult::SongsList { data: initial_songs.clone(), origin_path: None },
                    true,
                    &app_context,
                )
                .unwrap();
            screen.stack.current_mut().select_idx(1, 0);
            assert_eq!(
                screen.stack.current().selected(),
                Some(&DirOrSong::Song(initial_songs[1].clone()))
            );
            while rx.recv_timeout(Duration::from_millis(1)).is_ok() {}

            // then
            let rx2 = rx.clone();
            let new_songs = vec![song("s1"), song("s3"), song("s4")];
            let new_songs2 = new_songs.clone();
            std::thread::spawn(move || {
                let req = rx2.recv().unwrap();
                if let ClientRequest::QuerySync(qry) = req {
                    qry.tx.send(MpdQueryResult::Any(Box::new(new_songs2))).unwrap();
                }
            });
            screen
                .on_query_finished(
                    REINIT,
                    MpdQueryResult::DirOrSong {
                        data: vec![dir("pl1"), dir("pl2"), dir("pl3"), dir("pl4")],
                        origin_path: None,
                    },
                    true,
                    &app_context,
                )
                .unwrap();
            assert_eq!(screen.stack.previous().selected(), Some(&dir("pl3")));
            assert_eq!(
                screen.stack.current().selected(),
                Some(&DirOrSong::Song(new_songs[1].clone()))
            );
        }

        #[rstest]
        fn selects_the_same_playlist_idx_and_last_song(
            mut screen: PlaylistsPane,
            work_request_channel: (Sender<WorkRequest>, Receiver<WorkRequest>),
            client_request_channel: (Sender<ClientRequest>, Receiver<ClientRequest>),
        ) {
            let rx = client_request_channel.1.clone();
            let app_context = app_context(work_request_channel, client_request_channel);
            let initial_songs = vec![song("s1"), song("s2"), song("s3"), song("s4")];
            let initial_playlists = vec![dir("pl1"), dir("pl2"), dir("pl3"), dir("pl4")];
            screen
                .on_query_finished(
                    INIT,
                    MpdQueryResult::DirOrSong { data: initial_playlists, origin_path: None },
                    true,
                    &app_context,
                )
                .unwrap();
            screen.stack.current_mut().select_idx(2, 0);
            screen.stack_mut().push(Vec::new());
            screen
                .on_query_finished(
                    OPEN_OR_PLAY,
                    MpdQueryResult::SongsList { data: initial_songs.clone(), origin_path: None },
                    true,
                    &app_context,
                )
                .unwrap();
            screen.stack.current_mut().select_idx(1, 0);
            assert_eq!(
                screen.stack.current().selected(),
                Some(&DirOrSong::Song(initial_songs[1].clone()))
            );
            while rx.recv_timeout(Duration::from_millis(1)).is_ok() {}

            // then
            let rx2 = rx.clone();
            let new_songs = vec![song("s1"), song("s3"), song("s4")];
            let new_songs2 = new_songs.clone();
            std::thread::spawn(move || {
                let req = rx2.recv().unwrap();
                if let ClientRequest::QuerySync(qry) = req {
                    qry.tx.send(MpdQueryResult::Any(Box::new(new_songs2))).unwrap();
                }
            });
            screen
                .on_query_finished(
                    REINIT,
                    MpdQueryResult::DirOrSong {
                        data: vec![dir("pl1"), dir("pl2"), dir("pl4")],
                        origin_path: None,
                    },
                    true,
                    &app_context,
                )
                .unwrap();
            assert_eq!(screen.stack.previous().selected(), Some(&dir("pl4")));
            assert_eq!(
                screen.stack.current().selected(),
                Some(&DirOrSong::Song(new_songs[1].clone()))
            );
        }
    }
}

static LAST_ID: AtomicU32 = AtomicU32::new(1);
static NOW: LazyLock<chrono::DateTime<chrono::Utc>> = LazyLock::new(chrono::Utc::now);

pub fn new_id() -> u32 {
    LAST_ID.fetch_add(1, Ordering::Relaxed)
}
fn song(name: &str) -> Song {
    Song {
        id: new_id(),
        file: name.to_string(),
        duration: Some(Duration::from_secs(1)),
        metadata: HashMap::new(),
        stickers: None,
        last_modified: *NOW,
        added: None,
    }
}

fn dir(name: &str) -> DirOrSong {
    DirOrSong::Dir { name: name.to_string(), full_path: name.to_string(), last_modified: *NOW }
}

#[fixture]
fn screen(app_context: AppContext) -> PlaylistsPane {
    let mut screen = PlaylistsPane::new(&app_context);
    screen.before_show(&app_context).unwrap();
    screen
}
