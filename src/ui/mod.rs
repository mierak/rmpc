use std::collections::HashMap;

use anyhow::{Context, Result, anyhow};
use itertools::Itertools;
use modals::{
    add_random_modal::AddRandomModal,
    decoders::DecodersModal,
    info_list_modal::InfoListModal,
    input_modal::InputModal,
    keybinds::KeybindsModal,
    menu::modal::MenuModal,
    outputs::OutputsModal,
};
use panes::{PaneContainer, Panes, pane_call};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    widgets::Block,
};
use tab_screen::TabScreen;

use self::{modals::Modal, panes::Pane};
use crate::{
    MpdQueryResult,
    config::{
        Config,
        cli::{Args, Command},
        keys::{CommonAction, GlobalAction, Key, actions::RateKind},
        tabs::{PaneType, SizedPaneOrSplit, TabName},
        theme::level_styles::LevelStyles,
    },
    core::{
        command::{create_env, run_external},
        config_watcher::ERROR_CONFIG_MODAL_ID,
    },
    ctx::{Ctx, FETCH_SONG_STICKERS, LIKE_STICKER, RATING_STICKER},
    mpd::{
        commands::{State, idle::IdleEvent},
        errors::{ErrorCode, MpdError, MpdFailureResponse},
        mpd_client::{MpdClient, MpdCommand, ValueChange},
        proto_client::ProtoClient,
        version::Version,
    },
    shared::{
        events::{Level, WorkRequest},
        id::Id,
        keys::ActionEvent,
        macros::{modal, status_error, status_info, status_warn},
        mouse_event::MouseEvent,
        mpd_client_ext::{Enqueue, MpdClientExt},
        ytdlp::YtDlpHost,
    },
    ui::{
        image::facade::EncodeData,
        input::{InputEvent, InputResultEvent},
        modals::{downloads::DownloadsModal, menu::create_rating_modal},
    },
};

pub mod browser;
pub mod dir_or_song;
pub mod dirstack;
pub mod image;
pub mod input;
pub mod modals;
pub mod panes;
pub mod tab_screen;
pub mod widgets;

#[derive(Debug)]
pub struct StatusMessage {
    pub message: String,
    pub level: Level,
    pub created: std::time::Instant,
    pub timeout: std::time::Duration,
}

#[derive(Debug)]
pub struct Ui<'ui> {
    panes: PaneContainer<'ui>,
    modals: Vec<Box<dyn Modal>>,
    tabs: HashMap<TabName, TabScreen>,
    layout: SizedPaneOrSplit,
    area: Rect,
}

const OPEN_DECODERS_MODAL: &str = "open_decoders_modal";
const OPEN_OUTPUTS_MODAL: &str = "open_outputs_modal";

pub const FILTER_PREFIX: &str = "[FILTER]:";

macro_rules! active_tab_call {
    ($self:ident, $ctx:ident, $fn:ident($($param:expr),+)) => {
        $self.tabs
            .get_mut(&$ctx.active_tab)
            .context(anyhow!("Expected tab '{}' to be defined. Please report this along with your config.", $ctx.active_tab))?
            .$fn(&mut $self.panes, $($param),+)
    }
}

impl<'ui> Ui<'ui> {
    pub fn new(ctx: &Ctx) -> Result<Ui<'ui>> {
        Ok(Self {
            panes: PaneContainer::new(ctx)?,
            layout: ctx.config.theme.layout.clone(),
            modals: Vec::default(),
            area: Rect::default(),
            tabs: Self::init_tabs(ctx)?,
        })
    }

    fn init_tabs(ctx: &Ctx) -> Result<HashMap<TabName, TabScreen>> {
        ctx.config
            .tabs
            .tabs
            .iter()
            .map(|(name, screen)| -> Result<_> {
                Ok((name.clone(), TabScreen::new(screen.panes.clone())?))
            })
            .try_collect()
    }

    fn calc_areas(&mut self, area: Rect, _ctx: &Ctx) {
        self.area = area;
    }

    pub fn change_tab(&mut self, new_tab: TabName, ctx: &mut Ctx) -> Result<()> {
        self.layout.for_each_pane(
            self.area,
            &mut |pane, _, _, _, _| {
                match self.panes.get_mut(&pane.pane, ctx)? {
                    Panes::TabContent => {
                        active_tab_call!(self, ctx, on_hide(ctx))?;
                    }
                    _ => {}
                }
                Ok(())
            },
            ctx,
        )?;

        ctx.active_tab = new_tab.clone();
        self.on_event(UiEvent::TabChanged(new_tab), ctx)?;

        self.layout.for_each_pane(
            self.area,
            &mut |pane, pane_area, _, _, _| {
                match self.panes.get_mut(&pane.pane, ctx)? {
                    Panes::TabContent => {
                        active_tab_call!(self, ctx, before_show(pane_area, ctx))?;
                    }
                    _ => {}
                }
                Ok(())
            },
            ctx,
        )
    }

    pub fn render(&mut self, frame: &mut Frame, ctx: &mut Ctx) -> Result<()> {
        self.area = frame.area();
        if let Some(bg_color) = ctx.config.theme.background_color {
            frame
                .render_widget(Block::default().style(Style::default().bg(bg_color)), frame.area());
        }

        self.layout.for_each_pane_custom_data(
            self.area,
            &mut *frame,
            &mut |pane, pane_area, block, block_area, bg_color, frame| {
                match self.panes.get_mut(&pane.pane, ctx)? {
                    Panes::TabContent => {
                        active_tab_call!(self, ctx, render(frame, pane_area, ctx))?;
                    }
                    mut pane_instance => {
                        pane_call!(pane_instance, render(frame, pane_area, ctx))?;
                    }
                }
                if let Some(bg_color) = bg_color {
                    frame.render_widget(
                        Block::default().style(Style::default().bg(bg_color)),
                        pane_area,
                    );
                }
                let border_style =
                    pane.border_style.unwrap_or_else(|| ctx.config.as_border_style());
                frame.render_widget(block.border_style(border_style), block_area);
                Ok(())
            },
            &mut |block, block_area, background_color, frame| {
                if let Some(bg_color) = background_color {
                    frame.render_widget(
                        Block::default().style(Style::default().bg(bg_color)),
                        block.inner(block_area),
                    );
                }
                frame.render_widget(block, block_area);
                Ok(())
            },
            ctx,
        )?;

        if ctx.config.theme.modal_backdrop && !self.modals.is_empty() {
            let buffer = frame.buffer_mut();
            buffer.set_style(*buffer.area(), Style::default().fg(Color::DarkGray));
        }

        for modal in &mut self.modals {
            modal.render(frame, ctx)?;
        }

        Ok(())
    }

    pub fn handle_mouse_event(&mut self, event: MouseEvent, ctx: &mut Ctx) -> Result<()> {
        if let Some(ref mut modal) = self.modals.last_mut() {
            modal.handle_mouse_event(event, ctx)?;
            return Ok(());
        }

        self.layout.for_each_pane(
            self.area,
            &mut |pane, _, _, _, _| {
                match self.panes.get_mut(&pane.pane, ctx)? {
                    Panes::TabContent => {
                        active_tab_call!(self, ctx, handle_mouse_event(event, ctx))?;
                    }
                    mut pane_instance => {
                        pane_call!(pane_instance, handle_mouse_event(event, ctx))?;
                    }
                }
                Ok(())
            },
            ctx,
        )?;

        ctx.render()?;

        Ok(())
    }

    pub fn handle_action(
        &mut self,
        key: &mut ActionEvent,
        ctx: &mut Ctx,
    ) -> Result<KeyHandleResult> {
        if let Some(ref mut modal) = self.modals.last_mut() {
            modal.handle_key(key, ctx)?;
            return Ok(KeyHandleResult::None);
        }

        active_tab_call!(self, ctx, handle_action(key, ctx))?;

        if let Some(action) = key.claim_global() {
            match action {
                GlobalAction::Partition { name: Some(name), autocreate } => {
                    let name = name.clone();
                    let autocreate = *autocreate;
                    ctx.command(move |client| {
                        match client.switch_to_partition(&name) {
                            Ok(()) => {}
                            Err(MpdError::Mpd(MpdFailureResponse {
                                code: ErrorCode::NoExist,
                                ..
                            })) if autocreate => {
                                client.new_partition(&name)?;
                                client.switch_to_partition(&name)?;
                            }
                            err @ Err(_) => err?,
                        }
                        Ok(())
                    });
                }
                GlobalAction::Partition { name: None, .. } => {
                    let result = ctx.query_sync(move |client| {
                        let partitions = client.list_partitions()?;
                        Ok(partitions.0)
                    })?;
                    let modal = MenuModal::new(ctx)
                        .width(60)
                        .list_section(ctx, |section| {
                            if ctx.status.partition == "default" {
                                None
                            } else {
                                let section = section.item("Switch to default partition", |ctx| {
                                    ctx.command(move |client| {
                                        client.switch_to_partition("default")?;
                                        Ok(())
                                    });
                                    Ok(())
                                });

                                Some(section)
                            }
                        })
                        .multi_section(ctx, |section| {
                            let mut section = section
                                .add_action("Switch", |ctx, label| {
                                    ctx.command(move |client| {
                                        client.switch_to_partition(&label)?;
                                        Ok(())
                                    });
                                })
                                .add_action("Delete", |ctx, label| {
                                    ctx.command(move |client| {
                                        client.delete_partition(&label)?;
                                        Ok(())
                                    });
                                });
                            let mut any_non_default = false;
                            for partition in result
                                .iter()
                                .filter(|p| *p != "default" && **p != ctx.status.partition)
                            {
                                section = section.add_item(partition);
                                any_non_default = true;
                            }

                            if any_non_default { Some(section) } else { None }
                        })
                        .input_section(ctx, "New partition:", |section| {
                            let section = section.action(|ctx, value| {
                                if !value.is_empty() {
                                    ctx.command(move |client| {
                                        client.send_start_cmd_list()?;
                                        client.send_new_partition(&value)?;
                                        client.send_switch_to_partition(&value)?;
                                        client.send_execute_cmd_list()?;
                                        client.read_ok()?;
                                        Ok(())
                                    });
                                }
                            });
                            Some(section)
                        })
                        .list_section(ctx, |section| Some(section.item("Cancel", |_ctx| Ok(()))))
                        .build();

                    modal!(ctx, modal);
                }
                GlobalAction::Command { command, .. } => {
                    let cmd = command.parse();
                    log::debug!("executing {cmd:?}");

                    if let Ok(Args { command: Some(cmd), .. }) = cmd
                        && ctx.work_sender.send(WorkRequest::Command(cmd)).is_err()
                    {
                        log::error!("Failed to send command");
                    }
                }
                GlobalAction::CommandMode => {
                    let modal =
                        InputModal::new(ctx).title("Execute a command").on_confirm(|ctx, value| {
                            match Args::parse_cli_line(value) {
                                Ok(Args {
                                    command:
                                        Some(Command::SearchYt {
                                            query,
                                            provider,
                                            interactive,
                                            limit,
                                            position,
                                        }),
                                    ..
                                }) => {
                                    let kind: YtDlpHost = provider.into();

                                    let info_msg = format!("Searching '{query}' on {kind}");
                                    let send_result = ctx.work_sender.send(WorkRequest::SearchYt {
                                        query,
                                        kind,
                                        limit,
                                        interactive,
                                        position,
                                    });

                                    match send_result {
                                        Ok(()) => {
                                            status_info!("{info_msg}");
                                        }
                                        Err(err) => {
                                            log::error!("Failed to send SearchYt work: {err}");
                                        }
                                    }

                                    Ok(())
                                }

                                Ok(Args {
                                    command: Some(Command::AddYt { url, position }),
                                    ..
                                }) => {
                                    let send_result =
                                        ctx.ytdlp_manager.download_url(&url, position);
                                    match send_result {
                                        Ok(()) => {
                                            if ctx.config.auto_open_downloads {
                                                modal!(ctx, DownloadsModal::new(ctx));
                                            }
                                        }
                                        Err(err) => {
                                            status_error!(err:?; "Failed to queue yt-dlp download");
                                        }
                                    }
                                    Ok(())
                                }

                                Ok(Args { command: Some(cmd), .. }) => {
                                    if ctx.work_sender.send(WorkRequest::Command(cmd)).is_err() {
                                        log::error!("Failed to send command");
                                    }
                                    Ok(())
                                }

                                Ok(_) => {
                                    log::warn!("No subcommand provided");
                                    Ok(())
                                }

                                Err(e) => {
                                    log::error!("Parse error: {e}");
                                    Ok(())
                                }
                            }
                        });
                    modal!(ctx, modal);
                }
                GlobalAction::NextTrack if ctx.status.state != State::Stop => {
                    let keep_state = ctx.config.keep_state_on_song_change;
                    let state = ctx.status.state;
                    ctx.command(move |client| {
                        client.next_keep_state(keep_state, state)?;
                        Ok(())
                    });
                }
                GlobalAction::PreviousTrack if ctx.status.state != State::Stop => {
                    let rewind_to_start = ctx.config.rewind_to_start_sec;
                    let elapsed_sec = ctx.status.elapsed.as_secs();
                    let keep_state = ctx.config.keep_state_on_song_change;
                    let state = ctx.status.state;
                    ctx.command(move |client| {
                        match rewind_to_start {
                            Some(value) => {
                                if elapsed_sec >= value {
                                    client.seek_current(ValueChange::Set(0))?;
                                } else {
                                    client.prev_keep_state(keep_state, state)?;
                                }
                            }
                            None => {
                                client.prev_keep_state(keep_state, state)?;
                            }
                        }
                        Ok(())
                    });
                }
                GlobalAction::Stop if matches!(ctx.status.state, State::Play | State::Pause) => {
                    ctx.command(move |client| {
                        client.stop()?;
                        Ok(())
                    });
                }
                GlobalAction::ToggleRepeat => {
                    let repeat = !ctx.status.repeat;
                    ctx.command(move |client| {
                        client.repeat(repeat)?;
                        Ok(())
                    });
                }
                GlobalAction::ToggleRandom => {
                    let random = !ctx.status.random;
                    ctx.command(move |client| {
                        client.random(random)?;
                        Ok(())
                    });
                }
                GlobalAction::ToggleSingle => {
                    let single = ctx.status.single;
                    ctx.command(move |client| {
                        if client.version() < Version::new(0, 21, 0) {
                            client.single(single.cycle_skip_oneshot())?;
                        } else {
                            client.single(single.cycle())?;
                        }
                        Ok(())
                    });
                }
                GlobalAction::ToggleConsume => {
                    let consume = ctx.status.consume;
                    ctx.command(move |client| {
                        if client.version() < Version::new(0, 24, 0) {
                            client.consume(consume.cycle_skip_oneshot())?;
                        } else {
                            client.consume(consume.cycle())?;
                        }
                        Ok(())
                    });
                }
                GlobalAction::ToggleSingleOnOff => {
                    let single = ctx.status.single;
                    ctx.command(move |client| {
                        client.single(single.cycle_skip_oneshot())?;
                        Ok(())
                    });
                }
                GlobalAction::ToggleConsumeOnOff => {
                    let consume = ctx.status.consume;
                    ctx.command(move |client| {
                        client.consume(consume.cycle_skip_oneshot())?;
                        Ok(())
                    });
                }
                GlobalAction::TogglePause => {
                    if matches!(ctx.status.state, State::Play | State::Pause) {
                        ctx.command(move |client| {
                            client.pause_toggle()?;
                            Ok(())
                        });
                    } else {
                        ctx.command(move |client| {
                            client.play()?;
                            Ok(())
                        });
                    }
                }
                GlobalAction::VolumeUp => {
                    let step = ctx.config.volume_step;
                    ctx.command(move |client| {
                        client.volume(ValueChange::Increase(step.into()))?;
                        Ok(())
                    });
                }
                GlobalAction::VolumeDown => {
                    let step = ctx.config.volume_step;
                    ctx.command(move |client| {
                        client.volume(ValueChange::Decrease(step.into()))?;
                        Ok(())
                    });
                }
                GlobalAction::CrossfadeUp => {
                    let current_xfade = ctx.status.xfade.unwrap_or(0);
                    let new_xfade = current_xfade.saturating_add(1);
                    ctx.command(move |client| {
                        client.crossfade(new_xfade)?;
                        Ok(())
                    });
                }
                GlobalAction::CrossfadeDown => {
                    let current_xfade = ctx.status.xfade.unwrap_or(0);
                    let new_xfade = current_xfade.saturating_sub(1);
                    ctx.command(move |client| {
                        client.crossfade(new_xfade)?;
                        Ok(())
                    });
                }
                GlobalAction::SeekForward
                    if matches!(ctx.status.state, State::Play | State::Pause) =>
                {
                    ctx.command(move |client| {
                        client.seek_current(ValueChange::Increase(5))?;
                        Ok(())
                    });
                }
                GlobalAction::SeekBack
                    if matches!(ctx.status.state, State::Play | State::Pause) =>
                {
                    ctx.command(move |client| {
                        client.seek_current(ValueChange::Decrease(5))?;
                        Ok(())
                    });
                }
                GlobalAction::SeekToStart
                    if matches!(ctx.status.state, State::Play | State::Pause) =>
                {
                    ctx.command(move |client| {
                        client.seek_current(ValueChange::Set(0))?;
                        Ok(())
                    });
                }
                GlobalAction::Update => {
                    ctx.command(move |client| {
                        client.update(None)?;
                        Ok(())
                    });
                }
                GlobalAction::Rescan => {
                    ctx.command(move |client| {
                        client.rescan(None)?;
                        Ok(())
                    });
                }
                GlobalAction::NextTab => {
                    self.change_tab(ctx.config.next_screen(&ctx.active_tab), ctx)?;
                    ctx.render()?;
                }
                GlobalAction::PreviousTab => {
                    self.change_tab(ctx.config.prev_screen(&ctx.active_tab), ctx)?;
                    ctx.render()?;
                }
                GlobalAction::SwitchToTab(name) => {
                    if ctx.config.tabs.names.contains(name) {
                        self.change_tab(name.clone(), ctx)?;
                        ctx.render()?;
                    } else {
                        status_error!(
                            "Tab with name '{}' does not exist. Check your configuration.",
                            name
                        );
                    }
                }
                GlobalAction::NextTrack => {}
                GlobalAction::PreviousTrack => {}
                GlobalAction::Stop => {}
                GlobalAction::SeekBack => {}
                GlobalAction::SeekForward => {}
                GlobalAction::SeekToStart => {}
                GlobalAction::ExternalCommand { command, .. } => {
                    run_external(command.clone(), create_env(ctx, std::iter::empty::<&str>()));
                }
                GlobalAction::Quit => return Ok(KeyHandleResult::Quit),
                GlobalAction::ShowHelp => {
                    let modal = KeybindsModal::new(ctx);
                    modal!(ctx, modal);
                }
                GlobalAction::ShowOutputs => {
                    let current_partition = ctx.status.partition.clone();
                    ctx.query().id(OPEN_OUTPUTS_MODAL).replace_id(OPEN_OUTPUTS_MODAL).query(
                        move |client| {
                            let outputs = client.list_partitioned_outputs(&current_partition)?;
                            Ok(MpdQueryResult::Outputs(outputs))
                        },
                    );
                }
                GlobalAction::ShowDecoders => {
                    ctx.query()
                        .id(OPEN_DECODERS_MODAL)
                        .replace_id(OPEN_DECODERS_MODAL)
                        .query(|client| Ok(MpdQueryResult::Decoders(client.decoders()?.0)));
                }
                GlobalAction::ShowCurrentSongInfo => {
                    if let Some((_, current_song)) = ctx.find_current_song_in_queue() {
                        modal!(
                            ctx,
                            InfoListModal::builder()
                                .items(current_song)
                                .title("Song info")
                                .column_widths(&[30, 70])
                                .build()
                        );
                    } else {
                        status_info!("No song is currently playing");
                    }
                }
                GlobalAction::AddRandom => {
                    modal!(ctx, AddRandomModal::new(ctx));
                }
                GlobalAction::ShowDownloads => {
                    modal!(ctx, DownloadsModal::new(ctx));
                }
            }
        } else if let Some(action) = key.claim_common() {
            #[allow(
                clippy::collapsible_match,
                reason = "Future expansion, remove when adding other actions"
            )]
            match action {
                CommonAction::Rate { kind, current: true, min_rating, max_rating } => {
                    if let Some((_, song)) = ctx.find_current_song_in_queue() {
                        match kind {
                            RateKind::Modal { values, custom, like } => {
                                let items = vec![Enqueue::File { path: song.file.clone() }];
                                modal!(
                                    ctx,
                                    create_rating_modal(
                                        items,
                                        values.as_slice(),
                                        *min_rating,
                                        *max_rating,
                                        *custom,
                                        *like,
                                        ctx
                                    )
                                );
                            }
                            RateKind::Value(value) => {
                                let uri = song.file.clone();
                                let value = value.to_string();
                                ctx.command(move |client| {
                                    client.set_sticker(&uri, RATING_STICKER, &value)?;
                                    Ok(())
                                });
                            }
                            RateKind::Like() => {
                                let uri = song.file.clone();
                                ctx.command(move |client| {
                                    client.set_sticker(&uri, LIKE_STICKER, "2")?;
                                    Ok(())
                                });
                            }
                            RateKind::Dislike() => {
                                let uri = song.file.clone();
                                ctx.command(move |client| {
                                    client.set_sticker(&uri, LIKE_STICKER, "0")?;
                                    Ok(())
                                });
                            }
                            RateKind::Neutral() => {
                                let uri = song.file.clone();
                                ctx.command(move |client| {
                                    client.set_sticker(&uri, LIKE_STICKER, "1")?;
                                    Ok(())
                                });
                            }
                        }
                    } else {
                        status_error!("No song is currently playing");
                    }
                }
                _ => {}
            }
        }

        Ok(KeyHandleResult::None)
    }

    pub fn handle_insert_mode(
        &mut self,
        action: Option<&mut ActionEvent>,
        buf: &[Key],
        ctx: &mut Ctx,
    ) -> Result<()> {
        if let Some(action) = action {
            // We got some resolved keybind in insert mode. Currently only Confirm and Close
            // are possible to be bound there so this is fine.
            let kind = match action.claim_common() {
                Some(CommonAction::Confirm) => InputResultEvent::Confirm,
                Some(CommonAction::Close) => InputResultEvent::Cancel,
                other => {
                    log::error!(other:?; "Expected Confirm or Close action in insert mode");
                    return Ok(());
                }
            };

            if let Some(ref mut modal) = self.modals.last_mut() {
                modal.handle_insert_mode(kind, ctx)?;
            } else {
                active_tab_call!(self, ctx, handle_insert_mode(kind, ctx))?;
            }

            ctx.input.normal_mode();
        } else {
            // Resolve each buffered key individually
            for key in buf {
                if let Some(kind) = ctx.input.handle_input(InputEvent::from_key_event(*key)) {
                    if let Some(ref mut modal) = self.modals.last_mut() {
                        modal.handle_insert_mode(kind, ctx)?;
                    } else {
                        active_tab_call!(self, ctx, handle_insert_mode(kind, ctx))?;
                    }
                }
            }
        }

        ctx.render()?;
        Ok(())
    }

    pub fn before_show(&mut self, area: Rect, ctx: &mut Ctx) -> Result<()> {
        self.calc_areas(area, ctx);

        self.layout.for_each_pane(
            self.area,
            &mut |pane, pane_area, _, _, _| {
                match self.panes.get_mut(&pane.pane, ctx)? {
                    Panes::TabContent => {
                        active_tab_call!(self, ctx, before_show(pane_area, ctx))?;
                    }
                    mut pane_instance => {
                        pane_call!(pane_instance, calculate_areas(pane_area, ctx))?;
                        pane_call!(pane_instance, before_show(ctx))?;
                    }
                }
                Ok(())
            },
            ctx,
        )
    }

    pub fn on_ui_app_event(&mut self, event: UiAppEvent, ctx: &mut Ctx) -> Result<()> {
        match event {
            UiAppEvent::Modal(modal) => {
                let existing_modal = modal.replacement_id().and_then(|id| {
                    self.modals
                        .iter_mut()
                        .find(|m| m.replacement_id().as_ref().is_some_and(|m_id| *m_id == id))
                });

                if let Some(existing_modal) = existing_modal {
                    *existing_modal = modal;
                } else {
                    self.modals.push(modal);
                }

                self.on_event(UiEvent::ModalOpened, ctx)?;
                ctx.render()?;
            }
            UiAppEvent::PopConfigErrorModal => {
                let original_len = self.modals.len();
                self.modals
                    .retain(|m| m.replacement_id().is_none_or(|id| id != ERROR_CONFIG_MODAL_ID));
                let new_len = self.modals.len();
                if new_len == 0 {
                    self.on_event(UiEvent::ModalClosed, ctx)?;
                }
                if original_len != new_len {
                    ctx.render()?;
                }
            }
            UiAppEvent::PopModal(id) => {
                let original_len = self.modals.len();
                self.modals.retain(|m| m.id() != id);
                let new_len = self.modals.len();
                if new_len == 0 {
                    self.on_event(UiEvent::ModalClosed, ctx)?;
                }
                if original_len != new_len {
                    ctx.render()?;
                }
            }
            UiAppEvent::ChangeTab(tab_name) => {
                self.change_tab(tab_name, ctx)?;
                ctx.render()?;
            }
        }
        Ok(())
    }

    pub fn resize(&mut self, area: Rect, ctx: &Ctx) -> Result<()> {
        log::trace!(area:?; "Terminal was resized");
        self.calc_areas(area, ctx);

        self.layout.for_each_pane(
            self.area,
            &mut |pane, pane_area, _, _, _| {
                match self.panes.get_mut(&pane.pane, ctx)? {
                    Panes::TabContent => {
                        active_tab_call!(self, ctx, resize(pane_area, ctx))?;
                    }
                    mut pane_instance => {
                        pane_call!(pane_instance, calculate_areas(pane_area, ctx))?;
                        pane_call!(pane_instance, resize(pane_area, ctx))?;
                    }
                }
                Ok(())
            },
            ctx,
        )
    }

    pub fn on_event(&mut self, mut event: UiEvent, ctx: &mut Ctx) -> Result<()> {
        match event {
            UiEvent::Database => {
                ctx.input.clear_all_buffers();
                status_warn!(
                    "The music database has been updated. Some parts of the UI may have been reinitialized to prevent inconsistent behaviours."
                );
            }
            UiEvent::ConfigChanged => {
                // Call on_hide for all panes in the current tab and current layout because they
                // might not be visible after the change
                self.layout.for_each_pane(
                    self.area,
                    &mut |pane, _, _, _, _| {
                        match self.panes.get_mut(&pane.pane, ctx)? {
                            Panes::TabContent => {
                                active_tab_call!(self, ctx, on_hide(ctx))?;
                            }
                            mut pane_instance => {
                                pane_call!(pane_instance, on_hide(ctx))?;
                            }
                        }
                        Ok(())
                    },
                    ctx,
                )?;

                self.layout = ctx.config.theme.layout.clone();
                let new_active_tab = ctx
                    .config
                    .tabs
                    .names
                    .iter()
                    .find(|tab| tab == &&ctx.active_tab)
                    .or(ctx.config.tabs.names.first())
                    .context("Expected at least one tab")?;

                let mut old_other_panes = std::mem::take(&mut self.panes.others);
                for (key, new_other_pane) in PaneContainer::init_other_panes(ctx) {
                    let old = old_other_panes.remove(&key);
                    self.panes.others.insert(key, old.unwrap_or(new_other_pane));
                }
                // We have to be careful about the order of operations here as they might cause
                // a panic if done incorrectly
                self.tabs = Self::init_tabs(ctx)?;
                ctx.active_tab = new_active_tab.clone();
                self.on_event(UiEvent::TabChanged(new_active_tab.clone()), ctx)?;

                // Call before_show here, because we have "hidden" all the panes before and this
                // will force them to reinitialize
                self.before_show(self.area, ctx)?;
            }
            _ => {}
        }

        for pane_type in &ctx.config.active_panes {
            let visible = self
                .tabs
                .get(&ctx.active_tab)
                .is_some_and(|tab| tab.panes.panes_iter().any(|pane| pane.pane == *pane_type))
                || self.layout.panes_iter().any(|pane| pane.pane == *pane_type);

            match self.panes.get_mut(pane_type, ctx)? {
                #[cfg(debug_assertions)]
                Panes::Logs(p) => p.on_event(&mut event, visible, ctx),
                Panes::Queue(p) => p.on_event(&mut event, visible, ctx),
                Panes::QueueHeader(p) => p.on_event(&mut event, visible, ctx),
                Panes::Directories(p) => p.on_event(&mut event, visible, ctx),
                Panes::Albums(p) => p.on_event(&mut event, visible, ctx),
                Panes::Artists(p) => p.on_event(&mut event, visible, ctx),
                Panes::Playlists(p) => p.on_event(&mut event, visible, ctx),
                Panes::Search(p) => p.on_event(&mut event, visible, ctx),
                Panes::AlbumArtists(p) => p.on_event(&mut event, visible, ctx),
                Panes::AlbumArt(p) => p.on_event(&mut event, visible, ctx),
                Panes::Lyrics(p) => p.on_event(&mut event, visible, ctx),
                Panes::ProgressBar(p) => p.on_event(&mut event, visible, ctx),
                Panes::Header(p) => p.on_event(&mut event, visible, ctx),
                Panes::Tabs(p) => p.on_event(&mut event, visible, ctx),
                #[cfg(debug_assertions)]
                Panes::FrameCount(p) => p.on_event(&mut event, visible, ctx),
                Panes::Others(p) => p.on_event(&mut event, visible, ctx),
                Panes::Cava(p) => p.on_event(&mut event, visible, ctx),
                // Property and the dummy TabContent pane do not need to receive events
                Panes::Property(_) | Panes::TabContent => Ok(()),
                // Empty pane is a noop, no events
                Panes::Empty(_) => Ok(()),
            }?;
        }

        for modal in &mut self.modals {
            modal.on_event(&mut event, ctx)?;
        }

        Ok(())
    }

    pub(crate) fn on_command_finished(
        &mut self,
        id: &'static str,
        pane: Option<PaneType>,
        data: MpdQueryResult,
        ctx: &mut Ctx,
    ) -> Result<()> {
        match pane {
            Some(pane_type) => {
                let visible =
                    self.tabs.get(&ctx.active_tab).is_some_and(|tab| {
                        tab.panes.panes_iter().any(|pane| pane.pane == pane_type)
                    }) || self.layout.panes_iter().any(|pane| pane.pane == pane_type);

                match self.panes.get_mut(&pane_type, ctx)? {
                    #[cfg(debug_assertions)]
                    Panes::Logs(p) => p.on_query_finished(id, data, visible, ctx),
                    Panes::Queue(p) => p.on_query_finished(id, data, visible, ctx),
                    Panes::QueueHeader(p) => p.on_query_finished(id, data, visible, ctx),
                    Panes::Directories(p) => p.on_query_finished(id, data, visible, ctx),
                    Panes::Albums(p) => p.on_query_finished(id, data, visible, ctx),
                    Panes::Artists(p) => p.on_query_finished(id, data, visible, ctx),
                    Panes::Playlists(p) => p.on_query_finished(id, data, visible, ctx),
                    Panes::Search(p) => p.on_query_finished(id, data, visible, ctx),
                    Panes::AlbumArtists(p) => p.on_query_finished(id, data, visible, ctx),
                    Panes::AlbumArt(p) => p.on_query_finished(id, data, visible, ctx),
                    Panes::Lyrics(p) => p.on_query_finished(id, data, visible, ctx),
                    Panes::ProgressBar(p) => p.on_query_finished(id, data, visible, ctx),
                    Panes::Header(p) => p.on_query_finished(id, data, visible, ctx),
                    Panes::Tabs(p) => p.on_query_finished(id, data, visible, ctx),
                    Panes::Others(p) => p.on_query_finished(id, data, visible, ctx),
                    #[cfg(debug_assertions)]
                    Panes::FrameCount(p) => p.on_query_finished(id, data, visible, ctx),
                    Panes::Cava(p) => p.on_query_finished(id, data, visible, ctx),
                    // Property and the dummy TabContent pane do not need to receive command
                    // notifications
                    Panes::Property(_) | Panes::TabContent => Ok(()),
                    // Empty pane is a noop, no commands
                    Panes::Empty(_) => Ok(()),
                }?;
            }
            None => match (id, data) {
                (OPEN_OUTPUTS_MODAL, MpdQueryResult::Outputs(outputs)) => {
                    modal!(ctx, OutputsModal::new(outputs));
                }
                (OPEN_DECODERS_MODAL, MpdQueryResult::Decoders(decoders)) => {
                    modal!(ctx, DecodersModal::new(decoders));
                }
                (FETCH_SONG_STICKERS, MpdQueryResult::SongStickers(stickers)) => {
                    for (k, v) in stickers {
                        // Assume all stickers were fetched for each song so simple replace is
                        // enough
                        ctx.set_song_stickers(k, v);
                    }
                    ctx.render()?;
                }
                (id, mut data) => {
                    // TODO a proper modal target
                    for modal in &mut self.modals {
                        modal.on_query_finished(id, &mut data, ctx)?;
                    }
                }
            },
        }

        Ok(())
    }
}

#[derive(Debug)]
pub enum UiAppEvent {
    Modal(Box<dyn Modal + Send + Sync>),
    PopModal(Id),
    PopConfigErrorModal,
    ChangeTab(TabName),
}

#[derive(Debug)]
#[allow(dead_code)]
pub enum UiEvent {
    Player,
    QueueChanged,
    Database,
    Output,
    StoredPlaylist,
    LogAdded(Vec<u8>),
    ModalOpened,
    ModalClosed,
    Exit,
    LyricsIndexed,
    SongChanged,
    Reconnected,
    TabChanged(TabName),
    Displayed,
    Hidden,
    ConfigChanged,
    PlaybackStateChanged,
    ImageEncoded { data: EncodeData },
    ImageEncodeFailed { err: anyhow::Error },
    DownloadsUpdated,
}

impl TryFrom<IdleEvent> for UiEvent {
    type Error = ();

    fn try_from(event: IdleEvent) -> Result<Self, ()> {
        Ok(match event {
            IdleEvent::Player => UiEvent::Player,
            IdleEvent::Database => UiEvent::Database,
            IdleEvent::StoredPlaylist => UiEvent::StoredPlaylist,
            IdleEvent::Output => UiEvent::Output,
            _ => return Err(()),
        })
    }
}

pub enum KeyHandleResult {
    None,
    Quit,
}

impl From<&Level> for Color {
    fn from(value: &Level) -> Self {
        match value {
            Level::Info => Color::Blue,
            Level::Warn => Color::Yellow,
            Level::Error => Color::Red,
            Level::Debug => Color::LightGreen,
            Level::Trace => Color::Magenta,
        }
    }
}

impl Level {
    pub fn into_style(self, config: &LevelStyles) -> Style {
        match self {
            Level::Trace => config.trace,
            Level::Debug => config.debug,
            Level::Warn => config.warn,
            Level::Error => config.error,
            Level::Info => config.info,
        }
    }
}

impl Config {
    fn next_screen(&self, current_screen: &TabName) -> TabName {
        let names = &self.tabs.names;
        names
            .iter()
            .enumerate()
            .find(|(_, s)| *s == current_screen)
            .and_then(|(idx, _)| names.get((idx + 1) % names.len()))
            .unwrap_or(current_screen)
            .clone()
    }

    fn prev_screen(&self, current_screen: &TabName) -> TabName {
        let names = &self.tabs.names;
        self.tabs
            .names
            .iter()
            .enumerate()
            .find(|(_, s)| *s == current_screen)
            .and_then(|(idx, _)| {
                names.get((if idx == 0 { names.len() - 1 } else { idx - 1 }) % names.len())
            })
            .unwrap_or(current_screen)
            .clone()
    }

    fn as_border_style(&self) -> ratatui::style::Style {
        self.theme.borders_style
    }

    fn as_focused_border_style(&self) -> ratatui::style::Style {
        self.theme.highlight_border_style
    }

    fn as_text_style(&self) -> ratatui::style::Style {
        self.theme.text_color.map(|color| Style::default().fg(color)).unwrap_or_default()
    }

    fn as_styled_scrollbar(&self) -> Option<ratatui::widgets::Scrollbar<'_>> {
        let scrollbar = self.theme.scrollbar.as_ref()?;
        let symbols = &scrollbar.symbols;
        Some(
            ratatui::widgets::Scrollbar::default()
                .orientation(ratatui::widgets::ScrollbarOrientation::VerticalRight)
                .track_symbol(if symbols[0].is_empty() { None } else { Some(&symbols[0]) })
                .thumb_symbol(&scrollbar.symbols[1])
                .begin_symbol(if symbols[2].is_empty() { None } else { Some(&symbols[2]) })
                .end_symbol(if symbols[3].is_empty() { None } else { Some(&symbols[3]) })
                .track_style(scrollbar.track_style)
                .begin_style(scrollbar.ends_style)
                .end_style(scrollbar.ends_style)
                .thumb_style(scrollbar.thumb_style),
        )
    }
}
