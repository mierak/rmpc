use ansi_to_tui::IntoText;
use anyhow::Result;
use crossterm::event::KeyEvent;
use itertools::Itertools;
use ratatui::{
    prelude::{Constraint, Direction, Layout, Margin, Rect},
    widgets::{List, ListItem, ListState},
    Frame,
};
use strum::Display;

use crate::{
    mpd::mpd_client::MpdClient,
    state::State,
    ui::{utils::dirstack::DirState, KeyHandleResultInternal},
};

use super::{CommonAction, Screen};

#[derive(Debug, Default)]
pub struct LogsScreen {
    scrolling_state: DirState<ListState>,
}

impl Screen for LogsScreen {
    type Actions = LogsActions;
    fn render(&mut self, frame: &mut Frame, area: Rect, app: &mut crate::state::State) -> anyhow::Result<()> {
        let lines: Vec<_> = app
            .logs
            .iter()
            .map(|l| -> Result<_> { Ok(l.into_text()?.lines) })
            .flatten_ok()
            .enumerate()
            .map(|(idx, l)| -> Result<_> {
                match l {
                    Ok(mut val) => {
                        if self.scrolling_state.get_selected().is_some_and(|v| v == idx) {
                            val = val.patch_style(app.config.ui.current_item_style);
                        }
                        Ok(ListItem::new(val))
                    }
                    Err(err) => Err(err),
                }
            })
            .try_collect()?;

        let scrollbar = app.config.as_styled_scrollbar();

        let [content, scroll] = *Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(100), Constraint::Min(0)].as_ref())
            .split(area)
        else {
            return Ok(());
        };

        let content_len = lines.len();
        self.scrolling_state.set_content_len(Some(content_len));
        self.scrolling_state.set_viewport_len(Some(content.height.into()));

        let logs_wg = List::new(lines);
        frame.render_stateful_widget(logs_wg, content, self.scrolling_state.as_render_state_ref());
        frame.render_stateful_widget(
            scrollbar,
            scroll.inner(&Margin {
                vertical: 1,
                horizontal: 0,
            }),
            self.scrolling_state.as_scrollbar_state_ref(),
        );

        Ok(())
    }

    fn before_show(&mut self, _client: &mut impl MpdClient, _app: &mut crate::state::State) -> Result<()> {
        self.scrolling_state.last();
        Ok(())
    }

    fn handle_action(
        &mut self,
        event: KeyEvent,
        _client: &mut impl MpdClient,
        app: &mut State,
    ) -> Result<KeyHandleResultInternal> {
        if let Some(action) = app.config.keybinds.logs.get(&event.into()) {
            match action {
                LogsActions::Clear => {
                    app.logs.clear();
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
            }
        } else if let Some(action) = app.config.keybinds.navigation.get(&event.into()) {
            match action {
                CommonAction::DownHalf => {
                    self.scrolling_state.next_half_viewport();
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                CommonAction::UpHalf => {
                    self.scrolling_state.prev_half_viewport();
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                CommonAction::Up => {
                    self.scrolling_state.prev();
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                CommonAction::Down => {
                    self.scrolling_state.next();
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                CommonAction::Bottom => {
                    self.scrolling_state.last();
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                CommonAction::Top => {
                    self.scrolling_state.first();
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                CommonAction::Right => Ok(KeyHandleResultInternal::SkipRender),
                CommonAction::Left => Ok(KeyHandleResultInternal::SkipRender),
                CommonAction::EnterSearch => Ok(KeyHandleResultInternal::SkipRender),
                CommonAction::NextResult => Ok(KeyHandleResultInternal::SkipRender),
                CommonAction::PreviousResult => Ok(KeyHandleResultInternal::SkipRender),
                CommonAction::Add => Ok(KeyHandleResultInternal::SkipRender),
                CommonAction::Select => Ok(KeyHandleResultInternal::SkipRender),
                CommonAction::Delete => Ok(KeyHandleResultInternal::SkipRender),
                CommonAction::Rename => Ok(KeyHandleResultInternal::SkipRender),
                CommonAction::MoveUp => Ok(KeyHandleResultInternal::SkipRender),
                CommonAction::MoveDown => Ok(KeyHandleResultInternal::SkipRender),
                CommonAction::Close => Ok(KeyHandleResultInternal::SkipRender),
                CommonAction::Confirm => Ok(KeyHandleResultInternal::SkipRender),
                CommonAction::FocusInput => Ok(KeyHandleResultInternal::SkipRender),
            }
        } else {
            Ok(KeyHandleResultInternal::KeyNotHandled)
        }
    }
}

#[derive(Debug, Display, Clone, Copy, serde::Serialize, serde::Deserialize, PartialEq, Eq, Hash)]
pub enum LogsActions {
    Clear,
}
