use std::{
    collections::HashSet,
    ops::Sub,
    path::PathBuf,
    sync::{Arc, LazyLock},
    time::{Duration, Instant},
};

use crossbeam::channel::{Receiver, RecvTimeoutError};
use ratatui::{Terminal, layout::Rect, prelude::Backend};

use super::command::{create_env, run_external};
use crate::{
    config::{Config, cli::RemoteCommandQuery},
    ctx::Ctx,
    mpd::{
        commands::{IdleEvent, State},
        mpd_client::{MpdClient, SaveMode},
    },
    shared::{
        events::{AppEvent, WorkDone},
        ext::error::ErrorExt,
        id::{self, Id},
        keys::KeyResolver,
        macros::{modal, status_error, status_warn},
        mpd_client_ext::MpdClientExt,
        mpd_query::{
            EXTERNAL_COMMAND,
            GLOBAL_QUEUE_UPDATE,
            GLOBAL_STATUS_UPDATE,
            GLOBAL_STICKERS_UPDATE,
            GLOBAL_VOLUME_UPDATE,
            MpdQueryResult,
            run_status_update,
        },
    },
    ui::{
        KeyHandleResult,
        StatusMessage,
        Ui,
        UiAppEvent,
        UiEvent,
        modals::{downloads::DownloadsModal, info_modal::InfoModal, select_modal::SelectModal},
    },
};

static ON_RESIZE_SCHEDULE_ID: LazyLock<Id> = LazyLock::new(id::new);

pub fn init<B: Backend + std::io::Write + Send + 'static>(
    ctx: Ctx,
    event_rx: Receiver<AppEvent>,
    terminal: Terminal<B>,
) -> std::io::Result<std::thread::JoinHandle<Terminal<B>>> {
    std::thread::Builder::new()
        .name("main".to_owned())
        .spawn(move || main_task(ctx, event_rx, terminal))
}

fn main_task<B: Backend + std::io::Write>(
    mut ctx: Ctx,
    event_rx: Receiver<AppEvent>,
    mut terminal: Terminal<B>,
) -> Terminal<B> {
    let size = terminal.size().expect("To be able to get terminal size");
    let area = Rect::new(0, 0, size.width, size.height);
    let mut ui = Ui::new(&ctx).expect("UI to be created correctly");
    let event_receiver = event_rx;
    let mut render_wanted = false;
    let max_fps = f64::from(ctx.config.max_fps);
    let mut min_frame_duration = Duration::from_secs_f64(1f64 / max_fps);
    let mut last_render = std::time::Instant::now().sub(Duration::from_secs(10));
    let mut additional_evs = HashSet::new();
    let mut connected = true;
    ui.before_show(area, &mut ctx).expect("Initial render init to succeed");
    let mut _update_loop_guard = None;
    let mut _update_db_loop_guard = None;

    // Tmux hooks have to be initialized after ui, because ueberzugpp replaces all
    // hooks on its init instead of simply appending and might break rmpc's hooks
    let mut tmux = match crate::shared::tmux::TmuxHooks::new() {
        Ok(Some(val)) => Some(val),
        Ok(None) => None,
        Err(err) => {
            log::error!(error:? = err; "Failed to install tmux hooks");
            None
        }
    };

    // Execute on_song_change at startup if
    // configured and current song is available.
    if ctx.config.exec_on_song_change_at_start
        && let Some((_, _song)) = ctx.find_current_song_in_queue()
        && let Some(command) = &ctx.config.on_song_change
    {
        let env = create_env(&ctx, std::iter::empty());
        run_external(command.clone(), env);
    }

    // Listen to changes to lyrics when enabled
    let mut lyrics_watcher = if ctx.config.enable_lyrics_hot_reload
        && ctx.config.enable_lyrics_index
        && let Some(lyrics_dir) = &ctx.config.lyrics_dir
    {
        let lyrics_dir = PathBuf::from(lyrics_dir);
        let request_tx = ctx.work_sender.clone();
        Some(crate::core::lyrics_watcher::init(&lyrics_dir, request_tx))
    } else {
        None
    };

    match ctx.status.state {
        State::Play => {
            // Start update loop since a song is playing on startup
            _update_loop_guard = ctx
                .config
                .status_update_interval_ms
                .map(Duration::from_millis)
                .map(|interval| ctx.scheduler.repeated(interval, run_status_update));

            ctx.song_played = Some(ctx.status.elapsed);
        }
        State::Pause => {
            ctx.song_played = Some(ctx.status.elapsed);
        }
        State::Stop => {}
    }

    loop {
        let now = std::time::Instant::now();

        let event = if render_wanted {
            match event_receiver.recv_timeout(
                min_frame_duration.checked_sub(now - last_render).unwrap_or(Duration::ZERO),
            ) {
                Ok(v) => Some(v),
                Err(RecvTimeoutError::Timeout) => None,
                Err(RecvTimeoutError::Disconnected) => None,
            }
        } else {
            event_receiver.recv().ok()
        };

        if let Some(event) = event {
            match event {
                AppEvent::ConfigChanged { config: mut new_config, keep_old_theme } => {
                    // Technical limitation. Keep the old image backend because it was not rechecked
                    // anyway. Sending the escape sequences to determine image support would mess up
                    // the terminal output at this point.
                    new_config.album_art.method = ctx.config.album_art.method;
                    if keep_old_theme {
                        new_config.theme = ctx.config.theme.clone();
                    }

                    if let Err(err) = new_config.validate() {
                        status_error!(error:? = err; "Cannot change config, invalid value: '{err}'");
                        continue;
                    }

                    new_config.active_panes =
                        Config::calc_active_panes(&new_config.tabs.tabs, &new_config.theme.layout);
                    ctx.config = Arc::new(*new_config);
                    let max_fps = f64::from(ctx.config.max_fps);
                    min_frame_duration = Duration::from_secs_f64(1f64 / max_fps);

                    // Update lyrics watcher as needed
                    if ctx.config.enable_lyrics_hot_reload != lyrics_watcher.is_some()
                        && ctx.config.enable_lyrics_index
                    {
                        // IIFE may be better expressed with try blocks when it becomes stable
                        lyrics_watcher = (|| {
                            if !ctx.config.enable_lyrics_hot_reload {
                                return None;
                            }

                            let lyrics_dir = PathBuf::from(ctx.config.lyrics_dir.as_ref()?);
                            let request_tx = ctx.work_sender.clone();
                            Some(crate::core::lyrics_watcher::init(&lyrics_dir, request_tx))
                        })();
                    }

                    // Update keybinds
                    ctx.key_resolver = KeyResolver::new(&ctx.config);

                    if let Err(err) = ui.on_event(UiEvent::ConfigChanged, &mut ctx) {
                        log::error!(error:? = err; "UI failed to handle config changed event");
                        continue;
                    }

                    // Need to clear the terminal to avoid artifacts from album art and other
                    // elements
                    if let Err(err) = terminal.clear() {
                        log::error!(error:? = err; "Failed to clear terminal after config change");
                        continue;
                    }

                    render_wanted = true;
                }
                AppEvent::ThemeChanged { theme } => {
                    let mut config = ctx.config.as_ref().clone();
                    config.theme = *theme;
                    if let Err(err) = config.validate() {
                        status_error!(error:? = err; "Cannot change theme, invalid config: '{err}'");
                        continue;
                    }

                    config.tabs = match config
                        .original_tabs_definition
                        .clone()
                        .convert(&config.theme.components, &config.theme.border_symbol_sets)
                    {
                        Ok(v) => v,
                        Err(err) => {
                            status_error!(error:? = err; "Cannot change theme, failed to convert tabs: '{err}'");
                            continue;
                        }
                    };

                    config.active_panes =
                        Config::calc_active_panes(&config.tabs.tabs, &config.theme.layout);
                    ctx.config = Arc::new(config);

                    if let Err(err) = ui.on_event(UiEvent::ConfigChanged, &mut ctx) {
                        log::error!(error:? = err; "UI failed to handle config changed event");
                    }

                    // Need to clear the terminal to avoid artifacts from album art and other
                    // elements
                    if let Err(err) = terminal.clear() {
                        log::error!(error:? = err; "Failed to clear terminal after config change");
                        continue;
                    }
                    render_wanted = true;
                }
                AppEvent::UserKeyInput(key) => {
                    ctx.key_resolver.handle_key_event(key.into(), &ctx);
                    render_wanted = true;
                }
                AppEvent::UserMouseInput(ev) => match ui.handle_mouse_event(ev, &mut ctx) {
                    Ok(()) => {}
                    Err(err) => {
                        status_error!(err:?; "Error: {}", err.to_status());
                        render_wanted = true;
                    }
                },
                AppEvent::ActionResolved(mut action) => {
                    match ui.handle_action(&mut action, &mut ctx) {
                        Ok(KeyHandleResult::None) => continue,
                        Ok(KeyHandleResult::Quit) => {
                            if let Err(err) = ui.on_event(UiEvent::Exit, &mut ctx) {
                                log::error!(error:? = err; "UI failed to handle quit event");
                            }
                            break;
                        }
                        Err(err) => {
                            status_error!(err:?; "Error: {}", err.to_status());
                            render_wanted = true;
                        }
                    }
                }
                AppEvent::InsertModeFlush((mut action, buf)) => {
                    if let Err(err) = ui.handle_insert_mode(action.as_mut(), &buf, &mut ctx) {
                        log::error!(error:? = err, action:?, buf:?; "UI failed to handle insert mode flush");
                    }
                    render_wanted = true;
                }
                AppEvent::KeyTimeout => {
                    log::debug!("Key timeout reached, handling queued keys");
                    ctx.key_resolver.handle_timeout(&ctx);
                    render_wanted = true;
                }
                AppEvent::Status(mut message, level, timeout) => {
                    ctx.messages.push(StatusMessage {
                        level,
                        timeout,
                        message: std::mem::take(&mut message),
                        created: std::time::Instant::now(),
                    });

                    render_wanted = true;
                    // Send delayed render event to make the status message
                    // disappear
                    ctx.scheduler
                        .schedule(timeout, |(tx, _)| Ok(tx.send(AppEvent::RequestRender)?));
                }
                AppEvent::InfoModal { message, title, size, replacement_id: id } => {
                    if let Err(err) = ui.on_ui_app_event(
                        UiAppEvent::Modal(Box::new(
                            InfoModal::builder()
                                .ctx(&ctx)
                                .maybe_title(title)
                                .maybe_size(size)
                                .maybe_replacement_id(id)
                                .message(message)
                                .build(),
                        )),
                        &mut ctx,
                    ) {
                        log::error!(error:? = err; "UI failed to handle modal event");
                    }
                }
                AppEvent::Log(msg) => {
                    if let Err(err) = ui.on_event(UiEvent::LogAdded(msg), &mut ctx) {
                        log::error!(error:? = err; "UI failed to handle log event");
                    }
                }
                AppEvent::IdleEvent(event) => {
                    handle_idle_event(event, &ctx, &mut additional_evs);
                    for ev in additional_evs.drain().filter_map(|ev| UiEvent::try_from(ev).ok()) {
                        if let Err(err) = ui.on_event(ev, &mut ctx) {
                            status_error!(error:? = err, event:?; "UI failed to handle idle event, event: '{:?}', error: '{}'", event, err.to_status());
                        }
                    }
                    render_wanted = true;
                }
                AppEvent::RequestRender => {
                    render_wanted = true;
                }
                AppEvent::WorkDone(Ok(result)) => match result {
                    WorkDone::YtDlpPlaylistResolved { urls } => {
                        ctx.ytdlp_manager.queue_download_many(urls);
                        ctx.ytdlp_manager.download_next();
                    }
                    WorkDone::YtDlpDownloaded { id, result } => {
                        match ctx.ytdlp_manager.resolve_download(id, result) {
                            Ok((path, position)) => {
                                let cache_dir = ctx.config.cache_dir.clone();
                                ctx.command(move |client| {
                                    client.add_downloaded_files_to_queue(
                                        vec![path],
                                        cache_dir,
                                        position,
                                    )?;
                                    Ok(())
                                });
                            }
                            Err(err) => {
                                status_error!("Yt-dlp resulted in error: {err}");
                            }
                        }
                        ctx.ytdlp_manager.download_next();
                        if let Err(err) = ui.on_event(UiEvent::DownloadsUpdated, &mut ctx) {
                            log::error!(error:? = err; "UI failed to handle DownloadsUpdated event");
                        }
                    }
                    WorkDone::SearchYtResults { items, position, interactive } => {
                        if items.is_empty() {
                            status_warn!("No results found");
                        } else if !interactive {
                            let result = ctx.ytdlp_manager.download_url(&items[0].url, position);
                            match result {
                                Ok(()) => {
                                    if ctx.config.auto_open_downloads {
                                        modal!(ctx, DownloadsModal::new(&ctx));
                                    }
                                }
                                Err(err) => {
                                    status_error!("Failed to download first search result: {err}");
                                }
                            }
                        } else {
                            let labels: Vec<String> = items
                                .iter()
                                .map(|it| it.title.as_deref().unwrap_or("<no title>").to_string())
                                .collect();

                            let modal = SelectModal::builder()
                                .ctx(&ctx)
                                .title("Search results")
                                .confirm_label("Select")
                                .options(labels)
                                .on_confirm(move |ctx, _label, idx| {
                                    let result =
                                        ctx.ytdlp_manager.download_url(&items[idx].url, position);
                                    match result {
                                        Ok(()) => {
                                            if ctx.config.auto_open_downloads {
                                                modal!(ctx, DownloadsModal::new(ctx));
                                            }
                                        }
                                        Err(err) => {
                                            status_error!(
                                                "Failed to download selected item: {err}"
                                            );
                                        }
                                    }
                                    Ok(())
                                })
                                .build();

                            if let Err(err) =
                                ui.on_ui_app_event(UiAppEvent::Modal(Box::new(modal)), &mut ctx)
                            {
                                log::error!(error:? = err; "UI failed to handle modal event");
                            }
                        }

                        render_wanted = true;
                    }
                    WorkDone::ImageResized { data } => {
                        let event = match data {
                            Ok(data) => UiEvent::ImageEncoded { data },
                            Err(err) => UiEvent::ImageEncodeFailed { err },
                        };

                        if let Err(err) = ui.on_event(event, &mut ctx) {
                            log::error!(error:? = err; "UI failed to handle image resized event");
                        }
                    }
                    WorkDone::LyricsIndexed { index } => {
                        ctx.lrc_index = index;
                        if let Err(err) = ui.on_event(UiEvent::LyricsIndexed, &mut ctx) {
                            log::error!(error:? = err; "UI failed to handle lyrics indexed event");
                        }
                    }
                    WorkDone::SingleLrcIndexed { path, metadata } => {
                        if let Some(metadata) = metadata {
                            ctx.lrc_index.add(path, metadata);
                        }
                        if let Err(err) = ui.on_event(UiEvent::LyricsIndexed, &mut ctx) {
                            log::error!(error:? = err; "UI failed to handle single lyrics indexed event");
                        }
                    }
                    WorkDone::MpdCommandFinished { id, target, data } => match (id, target, data) {
                        (GLOBAL_STICKERS_UPDATE, None, MpdQueryResult::SongStickers(stickers)) => {
                            ctx.set_stickers(stickers);
                            render_wanted = true;
                        }
                        (
                            GLOBAL_STATUS_UPDATE,
                            None,
                            MpdQueryResult::Status { data: status, source_event },
                        ) => {
                            let current_song_id =
                                ctx.find_current_song_in_queue().map(|(_, song)| song.id);
                            let previous_state = ctx.status.state;
                            let current_updating_db = ctx.status.updating_db;
                            let current_playlist = ctx.status.lastloadedplaylist.take();
                            let previous_status = std::mem::replace(&mut ctx.status, status);
                            let new_playlist = ctx.status.lastloadedplaylist.as_ref();
                            let mut song_changed = false;

                            if ctx.config.reflect_changes_to_playlist
                                && matches!(source_event, Some(IdleEvent::Playlist))
                            {
                                // Try to reflect changes to saved playlist if any was loaded both
                                // before and after the update
                                if let (Some(current_playlist), Some(new_playlist)) =
                                    (current_playlist, new_playlist)
                                    && &current_playlist == new_playlist
                                {
                                    let playlist_name = current_playlist.clone();
                                    ctx.command(move |client| {
                                        client.save_queue_as_playlist(
                                            &playlist_name,
                                            Some(SaveMode::Replace),
                                        )?;
                                        Ok(())
                                    });
                                }
                            }

                            let mut start_render_loop = || {
                                _update_db_loop_guard = Some(ctx.scheduler.repeated(
                                    Duration::from_secs(1),
                                    |(tx, _)| {
                                        tx.send(AppEvent::RequestRender)?;
                                        Ok(())
                                    },
                                ));
                            };
                            match (current_updating_db, ctx.status.updating_db) {
                                (None, Some(_)) => {
                                    // update of db started
                                    ctx.db_update_start = Some(std::time::Instant::now());
                                    start_render_loop();
                                }
                                (Some(_), Some(_)) if ctx.db_update_start.is_none() => {
                                    // rmpc is opened after db started updating
                                    // beforehand so we reassign
                                    ctx.db_update_start = Some(std::time::Instant::now());
                                    start_render_loop();
                                }
                                (Some(_), None) => {
                                    // update of db ended
                                    ctx.db_update_start = None;
                                    _update_db_loop_guard = None;
                                }
                                _ => {}
                            }

                            if previous_state != ctx.status.state
                                && let Err(err) =
                                    ui.on_event(UiEvent::PlaybackStateChanged, &mut ctx)
                            {
                                status_error!(error:? = err; "UI failed to handle playback state changed event, error: '{}'", err.to_status());
                            }

                            match ctx.status.state {
                                State::Play if previous_state == ctx.status.state => {
                                    if let Some(played) = &mut ctx.song_played {
                                        *played += ctx.last_status_update.elapsed();
                                    }
                                }
                                State::Play if previous_state != ctx.status.state => {
                                    _update_loop_guard = ctx
                                        .config
                                        .status_update_interval_ms
                                        .map(Duration::from_millis)
                                        .map(|interval| {
                                            ctx.scheduler.repeated(interval, run_status_update)
                                        });
                                }
                                State::Play => {}
                                State::Pause => {
                                    _update_loop_guard = None;
                                }
                                State::Stop => {
                                    song_changed = true;
                                    ctx.song_played = None;
                                    _update_loop_guard = None;
                                }
                            }

                            if let Some((_, song)) = ctx.find_current_song_in_queue()
                                && Some(song.id) != current_song_id
                            {
                                if let Some(command) = &ctx.config.on_song_change {
                                    let mut env = create_env(&ctx, std::iter::empty());

                                    let prev_song_file = (previous_status.state != State::Stop)
                                        .then_some(
                                            previous_status
                                                .songid
                                                .and_then(|id| {
                                                    ctx.queue
                                                        .iter()
                                                        .enumerate()
                                                        .find(|(_, song)| song.id == id)
                                                })
                                                .map(|(_, s)| s.file.clone()),
                                        )
                                        .flatten();

                                    if let (Some(prev_song), Some(played)) =
                                        (prev_song_file, ctx.song_played)
                                    {
                                        env.push(("PREV_SONG".to_owned(), prev_song));
                                        env.push((
                                            "PREV_ELAPSED".to_owned(),
                                            played.as_secs().to_string(),
                                        ));
                                    }

                                    run_external(command.clone(), env);
                                }
                                song_changed = true;
                                ctx.song_played = Some(Duration::ZERO);
                            }
                            if song_changed
                                && let Err(err) = ui.on_event(UiEvent::SongChanged, &mut ctx)
                            {
                                status_error!(error:? = err; "UI failed to handle idle event, error: '{}'", err.to_status());
                            }

                            ctx.last_status_update = Instant::now();
                            render_wanted = true;
                        }
                        ("global_volume_update", None, MpdQueryResult::Volume(volume)) => {
                            ctx.status.volume = volume;
                            render_wanted = true;
                        }
                        ("global_queue_update", None, MpdQueryResult::Queue(queue)) => {
                            ctx.queue = queue.unwrap_or_default();
                            render_wanted = true;
                            log::debug!(len = ctx.queue.len(); "Queue updated");
                            if let Err(err) = ui.on_event(UiEvent::QueueChanged, &mut ctx) {
                                status_error!(error:? = err; "Ui failed to handle queue changed event, error: '{}'", err.to_status());
                            }
                        }
                        (
                            EXTERNAL_COMMAND,
                            None,
                            MpdQueryResult::ExternalCommand(command, songs),
                        ) => {
                            let songs = songs.iter().map(|s| s.file.as_str());
                            run_external(command, create_env(&ctx, songs));
                        }
                        (id, target, data) => {
                            if let Err(err) = ui.on_command_finished(id, target, data, &mut ctx) {
                                log::error!(error:? = err; "UI failed to handle command finished event");
                            }
                        }
                    },
                    WorkDone::None => {}
                },
                AppEvent::WorkDone(Err(err)) => {
                    status_error!("{}", err);
                }
                AppEvent::Resized { columns, rows } => {
                    ctx.scheduler.schedule_replace(
                        *ON_RESIZE_SCHEDULE_ID,
                        Duration::from_millis(500),
                        move |(tx, _)| {
                            tx.send(AppEvent::ResizedDebounced { columns, rows })?;
                            Ok(())
                        },
                    );
                    render_wanted = true;
                }
                AppEvent::ResizedDebounced { columns, rows } => {
                    if let Err(err) = ui.resize(Rect::new(0, 0, columns, rows), &ctx) {
                        log::error!(error:? = err, event:?; "UI failed to handle resize event");
                    }

                    if let Some(cmd) = &ctx.config.on_resize {
                        let cmd = Arc::clone(cmd);
                        let mut env = create_env(&ctx, std::iter::empty::<&str>());
                        env.push(("COLS".to_owned(), columns.to_string()));
                        env.push(("ROWS".to_owned(), rows.to_string()));
                        log::debug!("Executing on resize");
                        run_external(cmd, env);
                    }
                    if let Err(err) = terminal.clear() {
                        log::error!(error:? = err; "Failed to clear terminal after a resize");
                    }
                    render_wanted = true;
                }
                AppEvent::UiEvent(event) => match ui.on_ui_app_event(event, &mut ctx) {
                    Ok(()) => {}
                    Err(err) => {
                        status_error!(err:?; "Error: {}", err.to_status());
                        render_wanted = true;
                    }
                },
                AppEvent::RemoteSwitchTab { tab_name } => {
                    let target_tab = tab_name.as_str().into();

                    if let Some(tab) =
                        ctx.config.tabs.names.iter().find(|&name| *name == target_tab)
                    {
                        if let Err(err) =
                            ui.on_ui_app_event(UiAppEvent::ChangeTab(tab.clone()), &mut ctx)
                        {
                            status_error!(err:?; "Error switching to tab '{}': {}", tab_name, err.to_status());
                        }
                    } else {
                        let available = ctx
                            .config
                            .tabs
                            .names
                            .iter()
                            .map(|name| name.as_str())
                            .collect::<Vec<_>>()
                            .join(", ");
                        status_error!(
                            "Tab '{}' does not exist. Available tabs: {}",
                            tab_name,
                            available
                        );
                    }
                    render_wanted = true;
                }
                AppEvent::IpcQuery { mut stream, targets } => {
                    for target in targets {
                        match target {
                            RemoteCommandQuery::ActiveTab => {
                                stream
                                    .insert_response(target.to_string(), ctx.active_tab.0.as_str());
                            }
                        }
                    }
                }
                AppEvent::Reconnected => {
                    for ev in [IdleEvent::Player, IdleEvent::Playlist, IdleEvent::Options] {
                        handle_idle_event(ev, &ctx, &mut additional_evs);
                    }
                    if let Err(err) = ui.on_event(UiEvent::Reconnected, &mut ctx) {
                        log::error!(error:? = err, event:?; "UI failed to handle resize event");
                    }
                    status_warn!("rmpc reconnected to MPD and will reinitialize");
                    connected = true;
                }
                AppEvent::LostConnection => {
                    if ctx.status.state != State::Stop {
                        _update_loop_guard = None;
                        ctx.status.state = State::Stop;
                    }
                    if connected {
                        status_error!("rmpc lost connection to MPD and will try to reconnect");
                    }
                    connected = false;
                }
                AppEvent::TmuxHook { hook } => {
                    if let Some(tmux) = &mut tmux {
                        let old_visible = tmux.visible;
                        if let Err(err) = tmux.update_visible() {
                            log::error!(err:?, hook:?; "Failed to update tmux visibility");
                            continue;
                        }

                        let event = match (tmux.visible, old_visible) {
                            (true, false) => UiEvent::Displayed,
                            (false, true) => UiEvent::Hidden,
                            _ => continue,
                        };

                        match ui.on_event(event, &mut ctx) {
                            Ok(()) => {}
                            Err(err) => {
                                status_error!(err:?; "Error: {}", err.to_status());
                                render_wanted = true;
                            }
                        }
                    }
                }
            }
        }
        if render_wanted {
            let till_next_frame =
                min_frame_duration.saturating_sub(now.duration_since(last_render));
            if till_next_frame != Duration::ZERO {
                continue;
            }
            terminal
                .draw(|frame| {
                    if let Err(err) = ui.render(frame, &mut ctx) {
                        log::error!(error:? = err; "Failed to render a frame");
                    }
                })
                .expect("Expected render to succeed");

            ctx.finish_frame();
            last_render = now;
            render_wanted = false;
        }
    }

    terminal
}

fn handle_idle_event(event: IdleEvent, ctx: &Ctx, result_ui_evs: &mut HashSet<IdleEvent>) {
    match event {
        IdleEvent::Mixer if ctx.supported_commands.contains("getvol") => {
            ctx.query()
                .id(GLOBAL_VOLUME_UPDATE)
                .replace_id("volume")
                .query(move |client| Ok(MpdQueryResult::Volume(client.get_volume()?)));
        }
        IdleEvent::Mixer => {
            ctx.query().id(GLOBAL_STATUS_UPDATE).replace_id("status").query(move |client| {
                Ok(MpdQueryResult::Status {
                    data: client.get_status()?,
                    source_event: Some(IdleEvent::Mixer),
                })
            });
        }
        IdleEvent::Options => {
            ctx.query().id(GLOBAL_STATUS_UPDATE).replace_id("status").query(move |client| {
                Ok(MpdQueryResult::Status {
                    data: client.get_status()?,
                    source_event: Some(IdleEvent::Options),
                })
            });
        }
        IdleEvent::Player => {
            ctx.query().id(GLOBAL_STATUS_UPDATE).replace_id("status").query(move |client| {
                Ok(MpdQueryResult::Status {
                    data: client.get_status()?,
                    source_event: Some(IdleEvent::Player),
                })
            });
        }
        IdleEvent::Playlist => {
            ctx.query()
                .id(GLOBAL_QUEUE_UPDATE)
                .replace_id("playlist")
                .query(move |client| Ok(MpdQueryResult::Queue(client.playlist_info()?)));
            if ctx.config.reflect_changes_to_playlist {
                // Do not replace because we want to update currently loaded playlist if any
                ctx.query().id(GLOBAL_STATUS_UPDATE).replace_id("status_from_playlist").query(
                    move |client| {
                        Ok(MpdQueryResult::Status {
                            data: client.get_status()?,
                            source_event: Some(IdleEvent::Playlist),
                        })
                    },
                );
            }
        }
        IdleEvent::Sticker => {
            if ctx.stickers_supported.into() {
                let songs: Vec<_> = ctx.stickers().keys().cloned().collect();
                ctx.query().id(GLOBAL_STICKERS_UPDATE).replace_id("global_stickers_update").query(
                    move |client| {
                        Ok(MpdQueryResult::SongStickers(client.fetch_song_stickers(songs)?))
                    },
                );
            }
        }
        IdleEvent::StoredPlaylist => {}
        IdleEvent::Database => {
            ctx.query().id(GLOBAL_STATUS_UPDATE).replace_id("status").query(move |client| {
                Ok(MpdQueryResult::Status {
                    data: client.get_status()?,
                    source_event: Some(IdleEvent::Database),
                })
            });
        }
        IdleEvent::Update => {}
        IdleEvent::Output => {}
        IdleEvent::Partition
        | IdleEvent::Subscription
        | IdleEvent::Message
        | IdleEvent::Neighbor
        | IdleEvent::Mount => {
            log::warn!(event:?; "Received unhandled event");
        }
    }

    result_ui_evs.insert(event);
}
