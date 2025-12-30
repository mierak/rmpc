use std::path::PathBuf;

use anyhow::Result;
use bon::vec;
use ratatui::{
    Frame,
    layout::Margin,
    macros::{constraint, constraints},
    style::Style,
    symbols::border,
    widgets::{Block, Borders, Cell, Clear, ListItem, Row, Table, TableState},
};

use crate::{
    config::{
        keys::CommonAction,
        theme::properties::{Property, SongProperty},
    },
    ctx::Ctx,
    shared::{
        id::{self, Id},
        keys::ActionEvent,
        mouse_event::MouseEvent,
        mpd_client_ext::MpdClientExt,
        ytdlp::{DownloadId, DownloadState},
    },
    ui::{
        UiEvent,
        dirstack::{Dir, DirStackItem},
        modal,
        modals::{Modal, info_modal::InfoModal, menu::modal::MenuModal},
    },
};

#[derive(Debug)]
pub struct DownloadsModal {
    id: Id,
    queue: Dir<DownloadId, TableState>,
}

impl Modal for DownloadsModal {
    fn id(&self) -> Id {
        self.id
    }

    fn render(&mut self, frame: &mut Frame, ctx: &mut Ctx) -> Result<()> {
        let popup_area = frame.area().centered(constraint!(==90%), constraint!(==90%));
        frame.render_widget(Clear, popup_area);
        if let Some(bg_color) = ctx.config.theme.modal_background_color {
            frame.render_widget(Block::default().style(Style::default().bg(bg_color)), popup_area);
        }

        let block = Block::default()
            .borders(Borders::ALL)
            .border_set(border::ROUNDED)
            .border_style(ctx.config.as_border_style())
            .title_alignment(ratatui::prelude::Alignment::Center)
            .title("Downloads");

        let area = block.inner(popup_area);

        let rows = ctx.ytdlp_manager.map_values(|item| {
            Row::new([
                Cell::from(""), // marker
                Cell::from(item.inner.id.clone()),
                Cell::from(item.inner.kind.to_string()),
                Cell::from(item.state.to_string()).style(item.state.as_style(ctx)),
            ])
        });
        let table = Table::new(rows, constraints![==1, ==33%, ==33%, ==34%])
            .row_highlight_style(ctx.config.theme.current_item_style)
            .header(Row::new(["", "Id", "Source", "State"]));

        self.queue
            .state
            .set_content_and_viewport_len(ctx.ytdlp_manager.len(), area.height as usize);
        frame.render_stateful_widget(table, area, self.queue.state.as_render_state_ref());
        frame.render_widget(block, popup_area);
        if let Some(scrollbar) = ctx.config.as_styled_scrollbar() {
            frame.render_stateful_widget(
                scrollbar,
                popup_area.inner(Margin { horizontal: 0, vertical: 1 }),
                self.queue.state.as_scrollbar_state_ref(),
            );
        }

        Ok(())
    }

    fn handle_key(&mut self, key: &mut ActionEvent, ctx: &mut Ctx) -> Result<()> {
        if let Some(action) = key.claim_common() {
            match action {
                CommonAction::Down => {
                    self.queue.next(ctx.config.scrolloff, ctx.config.wrap_navigation);
                    ctx.render()?;
                }
                CommonAction::Up => {
                    self.queue.prev(ctx.config.scrolloff, ctx.config.wrap_navigation);
                    ctx.render()?;
                }
                CommonAction::Close => {
                    self.hide(ctx)?;
                }
                CommonAction::Confirm => {
                    self.create_menu(ctx);
                }
                CommonAction::DownHalf => {
                    self.queue.next_half_viewport(ctx.config.scrolloff);
                    ctx.render()?;
                }
                CommonAction::UpHalf => {
                    self.queue.prev_viewport(ctx.config.scrolloff);
                    ctx.render()?;
                }
                CommonAction::PageUp => {
                    self.queue.prev_viewport(ctx.config.scrolloff);
                    ctx.render()?;
                }
                CommonAction::PageDown => {
                    self.queue.next_viewport(ctx.config.scrolloff);
                    ctx.render()?;
                }
                CommonAction::Top => {
                    self.queue.first();
                    ctx.render()?;
                }
                CommonAction::Bottom => {
                    self.queue.last();
                    ctx.render()?;
                }
                CommonAction::Select => {
                    // TODO
                    // self.queue.toggle_mark_selected();
                    ctx.render()?;
                }
                CommonAction::ShowInfo => {
                    // Maybe use this instead of modal for logs?
                }

                _ => {}
            }
        }
        Ok(())
    }

    fn handle_mouse_event(&mut self, event: MouseEvent, ctx: &mut Ctx) -> Result<()> {
        Ok(())
    }

    fn on_event(&mut self, event: &mut UiEvent, ctx: &Ctx) -> Result<()> {
        match event {
            UiEvent::DownloadsUpdated => {
                self.queue.items = ctx.ytdlp_manager.ids();
                if !self.queue.items.is_empty() && self.queue.selected().is_none() {
                    self.queue.state.select(Some(0), 0);
                }
            }
            _ => {}
        }
        Ok(())
    }
}

impl DownloadsModal {
    pub fn new(ctx: &Ctx) -> Self {
        let mut queue = Dir::new(ctx.ytdlp_manager.ids());
        if !queue.items.is_empty() {
            queue.state.select(Some(0), 0);
        }

        Self { id: id::new(), queue }
    }

    pub fn create_menu(&self, ctx: &mut Ctx) {
        if let Some((id, current)) =
            self.queue.selected().and_then(|id| ctx.ytdlp_manager.get(*id).map(|item| (id, item)))
        {
            let actions = match &current.state {
                DownloadState::Queued => vec![ContextAction::Cancel(*id)],
                DownloadState::Downloading => vec![],
                DownloadState::Completed { logs, path } => {
                    vec![ContextAction::Add(path.clone()), ContextAction::Logs(logs.clone())]
                }
                DownloadState::Failed { logs } => {
                    vec![ContextAction::Retry(*id), ContextAction::Logs(logs.clone())]
                }
                DownloadState::Canceled => vec![ContextAction::Requeue(*id)],
                DownloadState::AlreadyDownloaded { path } => {
                    vec![ContextAction::Add(path.clone())]
                }
            };

            let modal = MenuModal::new(ctx)
                .list_section(ctx, |mut section| {
                    let is_empty = actions.is_empty();
                    for mut action in actions {
                        match action {
                            ContextAction::Cancel(id) => {
                                section.add_item(action.to_string(), move |ctx| {
                                    ctx.ytdlp_manager.cancel_download(id);
                                    Ok(())
                                });
                            }
                            ContextAction::Add(ref mut path) => {
                                let path = std::mem::take(path);
                                section.add_item(action.to_string(), move |ctx| {
                                    let cache_dir = ctx.config.cache_dir.clone();
                                    ctx.command(|client| {
                                        client.add_downloaded_files_to_queue(
                                            vec![path],
                                            cache_dir,
                                            None,
                                        )?;
                                        Ok(())
                                    });
                                    Ok(())
                                });
                            }
                            ContextAction::Requeue(id) => {
                                section.add_item(action.to_string(), move |ctx| {
                                    ctx.ytdlp_manager.redownload(id);
                                    Ok(())
                                });
                            }
                            ContextAction::Logs(ref mut logs) => {
                                let logs = std::mem::take(logs);
                                section.add_item(action.to_string(), move |ctx| {
                                    let modal = InfoModal::builder()
                                        .ctx(ctx)
                                        .title("Logs")
                                        .percent_width(80.0)
                                        .message(logs)
                                        .replacement_id("download_logs")
                                        .build();
                                    modal!(ctx, modal);
                                    Ok(())
                                });
                            }
                            ContextAction::Retry(id) => {
                                section.add_item(action.to_string(), move |ctx| {
                                    ctx.ytdlp_manager.redownload(id);
                                    Ok(())
                                });
                            }
                        }
                    }

                    if is_empty { None } else { Some(section) }
                })
                .build();

            modal!(ctx, modal);
        }
    }
}

#[derive(strum::Display)]
enum ContextAction {
    Cancel(DownloadId),
    Add(PathBuf),
    Requeue(DownloadId),
    Logs(Vec<String>),
    Retry(DownloadId),
}

impl DownloadState {
    fn as_style(&self, ctx: &Ctx) -> ratatui::style::Style {
        match self {
            DownloadState::Queued => ctx.config.theme.level_styles.info,
            DownloadState::Downloading => ctx.config.theme.level_styles.warn,
            DownloadState::Completed { .. } => ctx.config.theme.level_styles.info,
            DownloadState::AlreadyDownloaded { .. } => ctx.config.theme.level_styles.info,
            DownloadState::Failed { .. } => ctx.config.theme.level_styles.error,
            DownloadState::Canceled => ctx.config.theme.level_styles.error,
        }
    }
}

impl DirStackItem for DownloadId {
    fn as_path(&self) -> &'static str {
        ""
    }

    fn is_file(&self) -> bool {
        true
    }

    fn to_file_preview(&self, _ctx: &Ctx) -> Vec<crate::shared::mpd_query::PreviewGroup> {
        Vec::new()
    }

    fn matches(&self, _song_format: &[Property<SongProperty>], _ctx: &Ctx, _filter: &str) -> bool {
        true
    }

    fn to_list_item<'a>(
        &self,
        _ctx: &Ctx,
        _is_marked: bool,
        _matches_filter: bool,
        _additional_content: Option<String>,
    ) -> ListItem<'a> {
        ListItem::new("")
    }
}
