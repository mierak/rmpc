use ansi_to_tui::IntoText;
use anyhow::Result;
use crossterm::event::KeyEvent;
use itertools::Itertools;
use ratatui::{
    prelude::Rect,
    widgets::{Block, List, ListItem, ListState, Padding},
    Frame,
};
use strum::Display;

use crate::{
    config::Config,
    mpd::{commands::Status, mpd_client::MpdClient},
    state::MyVecDeque,
    ui::{utils::dirstack::DirState, KeyHandleResultInternal, UiEvent},
};

use super::{CommonAction, Screen};

#[derive(Debug, Default)]
pub struct LogsScreen {
    logs: MyVecDeque<Vec<u8>>,
    scrolling_state: DirState<ListState>,
}

impl Screen for LogsScreen {
    type Actions = LogsActions;
    fn render(&mut self, frame: &mut Frame, area: Rect, _status: &Status, config: &Config) -> anyhow::Result<()> {
        let lines: Vec<_> = self
            .logs
            .iter()
            .map(|l| -> Result<_> { Ok(l.into_text()?.lines) })
            .flatten_ok()
            .enumerate()
            .map(|(idx, l)| -> Result<_> {
                match l {
                    Ok(mut val) => {
                        if self.scrolling_state.get_selected().is_some_and(|v| v == idx) {
                            val = val.patch_style(config.theme.current_item_style);
                        }
                        Ok(ListItem::new(val))
                    }
                    Err(err) => Err(err),
                }
            })
            .try_collect()?;

        let content_len = lines.len();
        self.scrolling_state.set_content_len(Some(content_len));
        self.scrolling_state.set_viewport_len(Some(area.height.into()));

        let logs_wg = List::new(lines).block(Block::default().padding(Padding::right(5)));
        frame.render_stateful_widget(logs_wg, area, self.scrolling_state.as_render_state_ref());
        frame.render_stateful_widget(
            config.as_styled_scrollbar(),
            area,
            self.scrolling_state.as_scrollbar_state_ref(),
        );

        Ok(())
    }

    fn before_show(&mut self, _client: &mut impl MpdClient, _status: &mut Status, _config: &Config) -> Result<()> {
        self.scrolling_state.last();
        Ok(())
    }

    fn on_event(
        &mut self,
        event: &mut UiEvent,
        _client: &mut impl MpdClient,
        _status: &mut Status,
        _config: &Config,
    ) -> Result<KeyHandleResultInternal> {
        if let UiEvent::LogAdded(msg) = event {
            self.logs.push_back(std::mem::take(msg));
            if self.logs.len() > 1000 {
                self.logs.pop_front();
            }
            Ok(KeyHandleResultInternal::RenderRequested)
        } else {
            Ok(KeyHandleResultInternal::SkipRender)
        }
    }

    fn handle_action(
        &mut self,
        event: KeyEvent,
        _client: &mut impl MpdClient,
        _status: &mut Status,
        config: &Config,
    ) -> Result<KeyHandleResultInternal> {
        if let Some(action) = config.keybinds.logs.get(&event.into()) {
            match action {
                LogsActions::Clear => {
                    self.logs.clear();
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
            }
        } else if let Some(action) = config.keybinds.navigation.get(&event.into()) {
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
