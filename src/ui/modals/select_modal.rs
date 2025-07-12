use std::fmt::Display;

use anyhow::Result;
use bon::bon;
use ratatui::{
    Frame,
    layout::Rect,
    prelude::{Constraint, Layout},
    style::{Style, Stylize},
    symbols,
    widgets::{Block, Borders, Clear, List, ListState},
};

use super::{BUTTON_GROUP_SYMBOLS, Modal, RectExt};
use crate::{
    config::keys::CommonAction,
    ctx::Ctx,
    shared::{
        id::{self, Id},
        key_event::KeyEvent,
        mouse_event::{MouseEvent, MouseEventKind},
    },
    ui::{
        dirstack::DirState,
        widgets::button::{Button, ButtonGroup, ButtonGroupState},
    },
};

#[derive(Debug)]
enum FocusedComponent {
    List,
    Buttons,
}

#[derive(derive_more::Debug)]
pub struct SelectModal<'a, V: Display, Callback: FnOnce(&Ctx, V, usize) -> Result<()>> {
    id: Id,
    button_group_state: ButtonGroupState,
    button_group: ButtonGroup<'a>,
    scrolling_state: DirState<ListState>,
    focused: FocusedComponent,
    options_area: Rect,
    options: Vec<V>,
    #[debug(skip)]
    callback: Option<Callback>,
    title: &'a str,
}

#[bon]
impl<'a, V: Display, Callback: FnOnce(&Ctx, V, usize) -> Result<()>> SelectModal<'a, V, Callback> {
    #[builder]
    pub fn new(
        ctx: &Ctx,
        title: Option<&'a str>,
        options: Vec<V>,
        on_confirm: Callback,
        confirm_label: Option<&'a str>,
    ) -> Self {
        let mut scrolling_state = DirState::default();
        scrolling_state.select(Some(0), 0);

        let mut button_group_state = ButtonGroupState::default();
        let buttons = vec![
            Button::default().label(confirm_label.unwrap_or("Confirm")),
            Button::default().label("Cancel"),
        ];
        button_group_state.set_button_count(buttons.len());

        let button_group = ButtonGroup::default()
            .buttons(buttons)
            .inactive_style(ctx.config.as_text_style())
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_set(BUTTON_GROUP_SYMBOLS)
                    .border_style(ctx.config.as_border_style()),
            );

        Self {
            id: id::new(),
            button_group,
            button_group_state,
            scrolling_state,
            focused: FocusedComponent::List,
            options_area: Rect::default(),
            options,
            callback: Some(on_confirm),
            title: title.unwrap_or_default(),
        }
    }
}

impl<V: Display + std::fmt::Debug, Callback: FnOnce(&Ctx, V, usize) -> Result<()>> Modal
    for SelectModal<'_, V, Callback>
{
    fn id(&self) -> Id {
        self.id
    }

    fn render(&mut self, frame: &mut Frame, ctx: &mut Ctx) -> Result<()> {
        let popup_area = frame.area().centered_exact(80, 15);
        frame.render_widget(Clear, popup_area);
        if let Some(bg_color) = ctx.config.theme.modal_background_color {
            frame.render_widget(Block::default().style(Style::default().bg(bg_color)), popup_area);
        }

        let [list_area, buttons_area] =
            Layout::vertical([Constraint::Length(12), Constraint::Max(3)]).areas(popup_area);

        let content_len = self.options.len();
        self.scrolling_state.set_content_len(Some(content_len));
        self.scrolling_state.set_viewport_len(Some(list_area.height.into()));

        let options =
            self.options.iter().enumerate().map(|(idx, v)| format!("{:>3}: {v}", idx + 1));
        let playlists = List::new(options)
            .style(ctx.config.as_text_style())
            .highlight_style(match self.focused {
                FocusedComponent::Buttons => Style::default().reversed(),
                FocusedComponent::List => ctx.config.theme.current_item_style,
            })
            .block(
                Block::default()
                    .borders(Borders::TOP | Borders::LEFT | Borders::RIGHT)
                    .border_set(symbols::border::ROUNDED)
                    .border_style(ctx.config.as_border_style())
                    .title_alignment(ratatui::prelude::Alignment::Center)
                    .title(self.title.bold()),
            );

        self.button_group.set_active_style(match self.focused {
            FocusedComponent::List => Style::default().reversed(),
            FocusedComponent::Buttons => ctx.config.theme.current_item_style,
        });

        let scrollbar_area =
            Block::default().padding(ratatui::widgets::Padding::new(0, 0, 1, 0)).inner(list_area);

        self.options_area = list_area;

        frame.render_stateful_widget(
            playlists,
            list_area,
            self.scrolling_state.as_render_state_ref(),
        );
        if let Some(scrollbar) = ctx.config.as_styled_scrollbar() {
            frame.render_stateful_widget(
                scrollbar,
                scrollbar_area,
                self.scrolling_state.as_scrollbar_state_ref(),
            );
        }
        frame.render_stateful_widget(
            &mut self.button_group,
            buttons_area,
            &mut self.button_group_state,
        );
        Ok(())
    }

    fn handle_key(&mut self, key: &mut KeyEvent, ctx: &mut Ctx) -> Result<()> {
        if let Some(action) = key.as_common_action(ctx) {
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
                                self.scrolling_state.next(ctx.config.scrolloff, true);
                            }
                        }
                        FocusedComponent::Buttons => {
                            if self.button_group_state.selected
                                == self.button_group_state.button_count() - 1
                            {
                                self.focused = FocusedComponent::List;
                                self.scrolling_state.first();
                            } else {
                                self.button_group_state.next();
                            }
                        }
                    }

                    ctx.render()?;
                }
                CommonAction::Up => {
                    match self.focused {
                        FocusedComponent::List => {
                            if self.scrolling_state.get_selected().is_some_and(|s| s == 0) {
                                self.focused = FocusedComponent::Buttons;
                                self.button_group_state.last();
                            } else {
                                self.scrolling_state.prev(ctx.config.scrolloff, true);
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

                    ctx.render()?;
                }
                CommonAction::Confirm => match self.focused {
                    FocusedComponent::List => {
                        self.focused = FocusedComponent::Buttons;
                        self.button_group_state.first();

                        ctx.render()?;
                    }
                    FocusedComponent::Buttons if self.button_group_state.selected == 0 => {
                        if let Some(idx) = self.scrolling_state.get_selected() {
                            if let Some(callback) = self.callback.take() {
                                (callback)(ctx, self.options.remove(idx), idx)?;
                            }
                        }
                        self.hide(ctx)?;
                        ctx.render()?;
                    }
                    FocusedComponent::Buttons => {
                        self.button_group_state = ButtonGroupState::default();
                        self.hide(ctx)?;
                        ctx.render()?;
                    }
                },
                CommonAction::Close => {
                    self.button_group_state = ButtonGroupState::default();
                    self.hide(ctx)?;
                    ctx.render()?;
                }
                _ => {}
            }
        }

        Ok(())
    }

    fn handle_mouse_event(&mut self, event: MouseEvent, ctx: &mut Ctx) -> Result<()> {
        match event.kind {
            MouseEventKind::LeftClick if self.options_area.contains(event.into()) => {
                let y: usize = event.y.saturating_sub(self.options_area.y).into();
                let y = y.saturating_sub(1); // Subtract one to account for the header
                if let Some(idx) = self.scrolling_state.get_at_rendered_row(y) {
                    self.focused = FocusedComponent::List;
                    self.scrolling_state.select(Some(idx), ctx.config.scrolloff);
                    ctx.render()?;
                }
            }
            MouseEventKind::LeftClick => {
                if let Some(idx) = self.button_group.get_button_idx_at(event.into()) {
                    self.button_group_state.select(idx);
                    self.focused = FocusedComponent::Buttons;
                    ctx.render()?;
                }
            }
            MouseEventKind::DoubleClick => {
                match self.button_group.get_button_idx_at(event.into()) {
                    Some(0) => {
                        if let Some(idx) = self.scrolling_state.get_selected() {
                            if let Some(callback) = self.callback.take() {
                                (callback)(ctx, self.options.remove(idx), idx)?;
                            }
                        }
                        self.hide(ctx)?;
                        ctx.render()?;
                    }
                    Some(_) => {
                        self.hide(ctx)?;
                    }
                    None => {}
                }
            }
            MouseEventKind::MiddleClick => {}
            MouseEventKind::RightClick => {}
            MouseEventKind::ScrollUp
                if self.button_group.get_button_idx_at(event.into()).is_some() =>
            {
                self.focused = FocusedComponent::Buttons;
                self.button_group_state.prev();
                ctx.render()?;
            }
            MouseEventKind::ScrollDown
                if self.button_group.get_button_idx_at(event.into()).is_some() =>
            {
                self.focused = FocusedComponent::Buttons;
                self.button_group_state.next();
                ctx.render()?;
            }
            MouseEventKind::ScrollUp if self.options_area.contains(event.into()) => {
                self.focused = FocusedComponent::List;
                self.scrolling_state.prev(ctx.config.scrolloff, false);
                ctx.render()?;
            }
            MouseEventKind::ScrollDown if self.options_area.contains(event.into()) => {
                self.focused = FocusedComponent::List;
                self.scrolling_state.next(ctx.config.scrolloff, false);
                ctx.render()?;
            }
            MouseEventKind::ScrollDown => {}
            MouseEventKind::ScrollUp => {}
            MouseEventKind::Drag => {}
        }
        Ok(())
    }
}
