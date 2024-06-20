use itertools::Itertools;
use ratatui::{
    prelude::{Constraint, Layout},
    style::{Style, Stylize},
    symbols,
    widgets::{Block, Borders, Clear, List, ListState},
};

use crate::{
    mpd::mpd_client::MpdClient,
    ui::{
        screens::CommonAction,
        utils::dirstack::DirState,
        widgets::button::{Button, ButtonGroup, ButtonGroupState},
    },
};

use super::{KeyHandleResultInternal, RectExt};

use super::Modal;

#[derive(Debug)]
enum FocusedComponent {
    Playlists,
    Buttons,
}

#[derive(Debug)]
pub struct AddToPlaylistModal {
    button_group: ButtonGroupState,
    scrolling_state: DirState<ListState>,
    uri: String,
    playlists: Vec<String>,
    focused: FocusedComponent,
}

impl AddToPlaylistModal {
    pub fn new(uri: String, playlists: Vec<String>) -> Self {
        let mut scrolling_state = DirState::default();
        if !playlists.is_empty() {
            scrolling_state.select(Some(0));
        }
        Self {
            button_group: ButtonGroupState::default(),
            scrolling_state,
            uri,
            playlists,
            focused: FocusedComponent::Playlists,
        }
    }
}

const BUTTON_GROUP_SYMBOLS: symbols::border::Set = symbols::border::Set {
    top_right: symbols::line::NORMAL.vertical_left,
    top_left: symbols::line::NORMAL.vertical_right,
    ..symbols::border::ROUNDED
};

impl Modal for AddToPlaylistModal {
    fn render(&mut self, frame: &mut ratatui::Frame, app: &mut crate::state::State) -> anyhow::Result<()> {
        let popup_area = frame.size().centered_exact(80, 15);
        frame.render_widget(Clear, popup_area);
        if let Some(bg_color) = app.config.theme.modal_background_color {
            frame.render_widget(Block::default().style(Style::default().bg(bg_color)), popup_area);
        }

        let [list_area, buttons_area] =
            *Layout::vertical([Constraint::Length(12), Constraint::Max(3)]).split(popup_area)
        else {
            return Ok(());
        };

        let content_len = self.playlists.len();
        self.scrolling_state.set_content_len(Some(content_len));
        self.scrolling_state.set_viewport_len(Some(list_area.height.into()));

        let playlists = List::new(
            self.playlists
                .iter()
                .enumerate()
                .map(|(idx, v)| format!("{:>3}: {v}", idx + 1))
                .collect_vec(),
        )
        .style(app.config.as_text_style())
        .highlight_style(match self.focused {
            FocusedComponent::Buttons => Style::default().reversed(),
            FocusedComponent::Playlists => app.config.theme.current_item_style,
        })
        .block(
            Block::default()
                .borders(Borders::TOP | Borders::LEFT | Borders::RIGHT)
                .border_set(symbols::border::ROUNDED)
                .border_style(app.config.as_border_style())
                .title_alignment(ratatui::prelude::Alignment::Center)
                .title("Select a playlist".bold()),
        );
        let buttons = vec![Button::default().label("Add"), Button::default().label("Cancel")];
        self.button_group.set_button_count(buttons.len());
        let button_group = ButtonGroup::default()
            .buttons(buttons)
            .inactive_style(app.config.as_text_style())
            .active_style(match self.focused {
                FocusedComponent::Playlists => Style::default().reversed(),
                FocusedComponent::Buttons => app.config.theme.current_item_style,
            })
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_set(BUTTON_GROUP_SYMBOLS)
                    .border_style(app.config.as_border_style()),
            );

        let scrollbar_area = Block::default()
            .padding(ratatui::widgets::Padding::new(0, 0, 1, 0))
            .inner(list_area);
        frame.render_stateful_widget(playlists, list_area, self.scrolling_state.as_render_state_ref());
        frame.render_stateful_widget(
            app.config.as_styled_scrollbar(),
            scrollbar_area,
            self.scrolling_state.as_scrollbar_state_ref(),
        );
        frame.render_stateful_widget(button_group, buttons_area, &mut self.button_group);
        Ok(())
    }

    fn handle_key(
        &mut self,
        key: crossterm::event::KeyEvent,
        client: &mut crate::mpd::client::Client<'_>,
        app: &mut crate::state::State,
    ) -> anyhow::Result<KeyHandleResultInternal> {
        if let Some(action) = app.config.keybinds.navigation.get(&key.into()) {
            match action {
                CommonAction::Down => {
                    match self.focused {
                        FocusedComponent::Playlists => {
                            if self
                                .scrolling_state
                                .get_selected()
                                .is_some_and(|s| s == self.playlists.len() - 1)
                            {
                                self.focused = FocusedComponent::Buttons;
                                self.button_group.first();
                            } else {
                                self.scrolling_state.next();
                            }
                        }
                        FocusedComponent::Buttons => {
                            if self.button_group.selected == self.button_group.button_count() - 1 {
                                self.focused = FocusedComponent::Playlists;
                                self.scrolling_state.first();
                            } else {
                                self.button_group.next();
                            }
                        }
                    }
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                CommonAction::Up => {
                    match self.focused {
                        FocusedComponent::Playlists => {
                            if self.scrolling_state.get_selected().is_some_and(|s| s == 0) {
                                self.focused = FocusedComponent::Buttons;
                                self.button_group.last();
                            } else {
                                self.scrolling_state.prev();
                            }
                        }
                        FocusedComponent::Buttons => {
                            if self.button_group.selected == 0 {
                                self.focused = FocusedComponent::Playlists;
                                self.scrolling_state.last();
                            } else {
                                self.button_group.prev();
                            }
                        }
                    }
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                CommonAction::Confirm => match self.focused {
                    FocusedComponent::Playlists => {
                        self.focused = FocusedComponent::Buttons;
                        self.button_group.first();
                        Ok(KeyHandleResultInternal::RenderRequested)
                    }
                    FocusedComponent::Buttons if self.button_group.selected == 0 => {
                        if let Some(selected) = self.scrolling_state.get_selected() {
                            client.add_to_playlist(&self.playlists[selected], &self.uri, None)?;
                        }
                        Ok(KeyHandleResultInternal::Modal(None))
                    }
                    FocusedComponent::Buttons => {
                        self.button_group = ButtonGroupState::default();
                        Ok(KeyHandleResultInternal::Modal(None))
                    }
                },
                CommonAction::Close => {
                    self.button_group = ButtonGroupState::default();
                    Ok(KeyHandleResultInternal::Modal(None))
                }
                CommonAction::MoveDown => Ok(KeyHandleResultInternal::SkipRender),
                CommonAction::MoveUp => Ok(KeyHandleResultInternal::SkipRender),
                CommonAction::DownHalf => Ok(KeyHandleResultInternal::SkipRender),
                CommonAction::UpHalf => Ok(KeyHandleResultInternal::SkipRender),
                CommonAction::Right => Ok(KeyHandleResultInternal::SkipRender),
                CommonAction::Left => Ok(KeyHandleResultInternal::SkipRender),
                CommonAction::Top => Ok(KeyHandleResultInternal::SkipRender),
                CommonAction::Bottom => Ok(KeyHandleResultInternal::SkipRender),
                CommonAction::EnterSearch => Ok(KeyHandleResultInternal::SkipRender),
                CommonAction::NextResult => Ok(KeyHandleResultInternal::SkipRender),
                CommonAction::PreviousResult => Ok(KeyHandleResultInternal::SkipRender),
                CommonAction::Select => Ok(KeyHandleResultInternal::SkipRender),
                CommonAction::Add => Ok(KeyHandleResultInternal::SkipRender),
                CommonAction::Delete => Ok(KeyHandleResultInternal::SkipRender),
                CommonAction::Rename => Ok(KeyHandleResultInternal::SkipRender),
                CommonAction::FocusInput => Ok(KeyHandleResultInternal::SkipRender),
            }
        } else {
            Ok(KeyHandleResultInternal::SkipRender)
        }
    }
}
