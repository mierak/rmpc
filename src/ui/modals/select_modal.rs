use std::fmt::Display;

use anyhow::Result;
use ratatui::{
    layout::Rect,
    prelude::{Constraint, Layout},
    style::{Style, Stylize},
    symbols,
    widgets::{Block, Borders, Clear, List, ListState},
    Frame,
};

use crate::{
    config::keys::CommonAction,
    context::AppContext,
    shared::{
        key_event::KeyEvent,
        macros::pop_modal,
        mouse_event::{MouseEvent, MouseEventKind},
    },
    ui::{
        dirstack::DirState,
        widgets::button::{Button, ButtonGroup, ButtonGroupState},
    },
};

use super::RectExt;

use super::Modal;

#[derive(Debug)]
enum FocusedComponent {
    List,
    Buttons,
}

pub struct SelectModal<'a, V: Display, Callback: FnMut(&AppContext, &V, usize) -> Result<()>> {
    button_group_state: ButtonGroupState,
    button_group: ButtonGroup<'a>,
    scrolling_state: DirState<ListState>,
    focused: FocusedComponent,
    options_area: Rect,
    options: Vec<V>,
    callback: Option<Callback>,
    title: &'a str,
}

impl<'a, V: Display, Callback: FnMut(&AppContext, &V, usize) -> Result<()>> std::fmt::Debug
    for SelectModal<'a, V, Callback>
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "SelectModal(title = {:?}, button_group = {:?}, button_group_state = {:?})",
            self.title, self.button_group, self.button_group_state,
        )
    }
}

impl<'a, V: Display, Callback: FnMut(&AppContext, &V, usize) -> Result<()>> SelectModal<'a, V, Callback> {
    pub fn new(context: &AppContext) -> Self {
        let mut scrolling_state = DirState::default();
        scrolling_state.select(Some(0), 0);

        let mut button_group_state = ButtonGroupState::default();
        let buttons = vec![Button::default().label("Confirm"), Button::default().label("Cancel")];
        button_group_state.set_button_count(buttons.len());

        let button_group = ButtonGroup::default()
            .buttons(buttons)
            .inactive_style(context.config.as_text_style())
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_set(BUTTON_GROUP_SYMBOLS)
                    .border_style(context.config.as_border_style()),
            );

        Self {
            button_group,
            button_group_state,
            scrolling_state,
            focused: FocusedComponent::List,
            options_area: Rect::default(),
            options: Vec::new(),
            callback: None,
            title: "",
        }
    }

    pub fn options(mut self, options: Vec<V>) -> Self {
        self.options = options;
        self
    }

    pub fn on_confirm(mut self, callback: Callback) -> Self {
        self.callback = Some(callback);
        self
    }

    pub fn confirm_label(mut self, label: &'a str) -> Self {
        let buttons = vec![Button::default().label(label), Button::default().label("Cancel")];
        self.button_group = self.button_group.buttons(buttons);
        self
    }

    pub fn title(mut self, title: &'a str) -> Self {
        self.title = title;
        self
    }
}

const BUTTON_GROUP_SYMBOLS: symbols::border::Set = symbols::border::Set {
    top_right: symbols::line::NORMAL.vertical_left,
    top_left: symbols::line::NORMAL.vertical_right,
    ..symbols::border::ROUNDED
};

impl<'a, V: Display, Callback: FnMut(&AppContext, &V, usize) -> Result<()>> Modal for SelectModal<'a, V, Callback> {
    fn render(&mut self, frame: &mut Frame, app: &mut AppContext) -> Result<()> {
        let popup_area = frame.area().centered_exact(80, 15);
        frame.render_widget(Clear, popup_area);
        if let Some(bg_color) = app.config.theme.modal_background_color {
            frame.render_widget(Block::default().style(Style::default().bg(bg_color)), popup_area);
        }

        let [list_area, buttons_area] =
            Layout::vertical([Constraint::Length(12), Constraint::Max(3)]).areas(popup_area);

        let content_len = self.options.len();
        self.scrolling_state.set_content_len(Some(content_len));
        self.scrolling_state.set_viewport_len(Some(list_area.height.into()));

        let options = self
            .options
            .iter()
            .enumerate()
            .map(|(idx, v)| format!("{:>3}: {v}", idx + 1));
        let playlists = List::new(options)
            .style(app.config.as_text_style())
            .highlight_style(match self.focused {
                FocusedComponent::Buttons => Style::default().reversed(),
                FocusedComponent::List => app.config.theme.current_item_style,
            })
            .block(
                Block::default()
                    .borders(Borders::TOP | Borders::LEFT | Borders::RIGHT)
                    .border_set(symbols::border::ROUNDED)
                    .border_style(app.config.as_border_style())
                    .title_alignment(ratatui::prelude::Alignment::Center)
                    .title(self.title.bold()),
            );

        self.button_group.set_active_style(match self.focused {
            FocusedComponent::List => Style::default().reversed(),
            FocusedComponent::Buttons => app.config.theme.current_item_style,
        });

        let scrollbar_area = Block::default()
            .padding(ratatui::widgets::Padding::new(0, 0, 1, 0))
            .inner(list_area);

        self.options_area = list_area;

        frame.render_stateful_widget(playlists, list_area, self.scrolling_state.as_render_state_ref());
        frame.render_stateful_widget(
            app.config.as_styled_scrollbar(),
            scrollbar_area,
            self.scrolling_state.as_scrollbar_state_ref(),
        );
        frame.render_stateful_widget(&mut self.button_group, buttons_area, &mut self.button_group_state);
        Ok(())
    }

    fn handle_key(&mut self, key: &mut KeyEvent, context: &mut AppContext) -> Result<()> {
        if let Some(action) = key.as_common_action(context) {
            match action {
                CommonAction::Down => {
                    match self.focused {
                        FocusedComponent::List => {
                            if self
                                .scrolling_state
                                .get_selected()
                                .is_some_and(|s| s == self.options.len() - 1)
                            {
                                self.focused = FocusedComponent::Buttons;
                                self.button_group_state.first();
                            } else {
                                self.scrolling_state.next(context.config.scrolloff, true);
                            }
                        }
                        FocusedComponent::Buttons => {
                            if self.button_group_state.selected == self.button_group_state.button_count() - 1 {
                                self.focused = FocusedComponent::List;
                                self.scrolling_state.first();
                            } else {
                                self.button_group_state.next();
                            }
                        }
                    }

                    context.render()?;
                }
                CommonAction::Up => {
                    match self.focused {
                        FocusedComponent::List => {
                            if self.scrolling_state.get_selected().is_some_and(|s| s == 0) {
                                self.focused = FocusedComponent::Buttons;
                                self.button_group_state.last();
                            } else {
                                self.scrolling_state.prev(context.config.scrolloff, true);
                            }
                        }
                        FocusedComponent::Buttons => {
                            if self.button_group_state.selected == 0 {
                                self.focused = FocusedComponent::List;
                                self.scrolling_state.last();
                            } else {
                                self.button_group_state.prev();
                            }
                        }
                    }

                    context.render()?;
                }
                CommonAction::Confirm => match self.focused {
                    FocusedComponent::List => {
                        self.focused = FocusedComponent::Buttons;
                        self.button_group_state.first();

                        context.render()?;
                    }
                    FocusedComponent::Buttons if self.button_group_state.selected == 0 => {
                        if let Some(idx) = self.scrolling_state.get_selected() {
                            if let Some(ref mut callback) = self.callback {
                                (callback)(context, &self.options[idx], idx)?;
                            }
                        }
                        pop_modal!(context);
                        context.render()?;
                    }
                    FocusedComponent::Buttons => {
                        self.button_group_state = ButtonGroupState::default();
                        pop_modal!(context);
                        context.render()?;
                    }
                },
                CommonAction::Close => {
                    self.button_group_state = ButtonGroupState::default();
                    pop_modal!(context);
                    context.render()?;
                }
                _ => {}
            }
        };

        Ok(())
    }

    fn handle_mouse_event(&mut self, event: MouseEvent, context: &mut AppContext) -> Result<()> {
        match event.kind {
            MouseEventKind::LeftClick if self.options_area.contains(event.into()) => {
                let y: usize = event.y.saturating_sub(self.options_area.y).into();
                let y = y.saturating_sub(1); // Subtract one to account for the header
                if let Some(idx) = self.scrolling_state.get_at_rendered_row(y) {
                    self.focused = FocusedComponent::List;
                    self.scrolling_state.select(Some(idx), context.config.scrolloff);
                    context.render()?;
                }
            }
            MouseEventKind::LeftClick => {
                if let Some(idx) = self.button_group.get_button_idx_at(event.into()) {
                    self.button_group_state.select(idx);
                    self.focused = FocusedComponent::Buttons;
                    context.render()?;
                }
            }
            MouseEventKind::DoubleClick => {
                match self.button_group.get_button_idx_at(event.into()) {
                    Some(0) => {
                        if let Some(idx) = self.scrolling_state.get_selected() {
                            if let Some(ref mut callback) = self.callback {
                                (callback)(context, &self.options[idx], idx)?;
                            }
                        }
                        pop_modal!(context);
                        context.render()?;
                    }
                    Some(_) => {
                        pop_modal!(context);
                    }
                    None => {}
                };
            }
            MouseEventKind::MiddleClick => {}
            MouseEventKind::RightClick => {}
            MouseEventKind::ScrollUp if self.button_group.get_button_idx_at(event.into()).is_some() => {
                self.focused = FocusedComponent::Buttons;
                self.button_group_state.prev();
                context.render()?;
            }
            MouseEventKind::ScrollDown if self.button_group.get_button_idx_at(event.into()).is_some() => {
                self.focused = FocusedComponent::Buttons;
                self.button_group_state.next();
                context.render()?;
            }
            MouseEventKind::ScrollUp if self.options_area.contains(event.into()) => {
                self.focused = FocusedComponent::List;
                self.scrolling_state.prev(context.config.scrolloff, false);
                context.render()?;
            }
            MouseEventKind::ScrollDown if self.options_area.contains(event.into()) => {
                self.focused = FocusedComponent::List;
                self.scrolling_state.next(context.config.scrolloff, false);
                context.render()?;
            }
            MouseEventKind::ScrollDown => {}
            MouseEventKind::ScrollUp => {}
        }
        Ok(())
    }
}
