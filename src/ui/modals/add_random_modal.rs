use anyhow::Result;
use crossterm::event::KeyCode;
use enum_map::{Enum, EnumMap, enum_map};
use ratatui::{
    Frame,
    layout::Rect,
    prelude::{Constraint, Layout},
    style::{Style, Stylize},
    symbols::border,
    widgets::{Block, Borders, Clear},
};

use super::{BUTTON_GROUP_SYMBOLS, Modal, RectExt};
use crate::{
    WorkRequest,
    config::{
        cli::{AddRandom, Command},
        keys::CommonAction,
    },
    context::AppContext,
    shared::{
        key_event::KeyEvent,
        macros::pop_modal,
        mouse_event::{MouseEvent, MouseEventKind},
    },
    ui::widgets::{
        button::{Button, ButtonGroup, ButtonGroupState},
        input::Input,
    },
};

#[derive(derive_more::Debug)]
pub struct AddRandomModal<'a> {
    button_group_state: ButtonGroupState,
    button_group: ButtonGroup<'a>,
    active_input: InputType,
    input_areas: EnumMap<InputAreas, Rect>,
    count: String,
    selected_tag: AddRandom,
}

#[derive(Debug, Enum)]
enum InputAreas {
    Tag,
    Count,
    Buttons,
}

#[derive(Debug)]
enum InputType {
    Tag,
    Count,
    CountFocused,
    Buttons,
}

impl AddRandom {
    fn next(self) -> Self {
        match self {
            AddRandom::Song => AddRandom::Artist,
            AddRandom::Artist => AddRandom::Album,
            AddRandom::Album => AddRandom::AlbumArtist,
            AddRandom::AlbumArtist => AddRandom::Genre,
            AddRandom::Genre => AddRandom::Song,
        }
    }
}

impl AddRandomModal<'_> {
    pub fn new(context: &AppContext) -> Self {
        let mut button_group_state = ButtonGroupState::default();
        let buttons = vec![Button::default().label("Add"), Button::default().label("Cancel")];
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
            button_group_state,
            button_group,
            active_input: InputType::Tag,
            input_areas: enum_map! {
                _ => Rect::default(),
            },
            count: String::from("5"),
            selected_tag: AddRandom::Song,
        }
    }

    fn add_random(tag: AddRandom, count: &str, ctx: &AppContext, insert: bool) -> Result<()> {
        Ok(ctx.work_sender.send(WorkRequest::Command(Command::AddRandom {
            tag,
            count: count.parse()?,
            insert,
        }))?)
    }
}

impl Modal for AddRandomModal<'_> {
    fn render(&mut self, frame: &mut Frame, app: &mut AppContext) -> Result<()> {
        let block = Block::default()
            .borders(Borders::TOP | Borders::LEFT | Borders::RIGHT)
            .border_set(border::ROUNDED)
            .border_style(app.config.as_border_style())
            .title_alignment(ratatui::prelude::Alignment::Center);

        let popup_area = frame.area().centered_exact(50, 6);
        frame.render_widget(Clear, popup_area);
        if let Some(bg_color) = app.config.theme.modal_background_color {
            frame.render_widget(Block::default().style(Style::default().bg(bg_color)), popup_area);
        }

        let [body_area, buttons_area] =
            Layout::vertical([Constraint::Length(3), Constraint::Max(3)]).areas(popup_area);

        let [tag_area, count_area] =
            Layout::vertical([Constraint::Length(1), Constraint::Length(1)])
                .areas(block.inner(body_area));

        let mut combobox = Input::default()
            .set_label("Tag:   ")
            .set_label_style(app.config.as_text_style())
            .set_text(self.selected_tag.into())
            .set_focused(false)
            .set_borderless(true)
            .set_unfocused_style(app.config.as_border_style());

        if matches!(self.active_input, InputType::Tag) {
            combobox = combobox
                .set_label_style(app.config.theme.current_item_style)
                .set_input_style(app.config.theme.current_item_style);
        }

        let mut count = Input::default()
            .set_label("Count: ")
            .set_label_style(app.config.as_text_style())
            .set_text(&self.count)
            .set_focused(matches!(self.active_input, InputType::CountFocused))
            .set_focused_style(app.config.theme.highlight_border_style)
            .set_borderless(true)
            .set_unfocused_style(app.config.as_border_style());

        if matches!(self.active_input, InputType::Count) {
            count = count
                .set_label_style(app.config.theme.current_item_style)
                .set_input_style(app.config.theme.current_item_style);
        }

        self.button_group.set_active_style(match self.active_input {
            InputType::Buttons => app.config.theme.current_item_style,
            _ => Style::default().reversed(),
        });

        self.input_areas[InputAreas::Tag] = tag_area;
        self.input_areas[InputAreas::Count] = count_area;
        self.input_areas[InputAreas::Buttons] = buttons_area;

        frame.render_widget(combobox, tag_area);
        frame.render_widget(count, count_area);
        frame.render_widget(block, body_area);
        frame.render_stateful_widget(
            &mut self.button_group,
            buttons_area,
            &mut self.button_group_state,
        );
        Ok(())
    }

    fn handle_key(&mut self, key: &mut KeyEvent, context: &mut AppContext) -> Result<()> {
        let action = key.as_common_action(context);
        match self.active_input {
            InputType::CountFocused => {
                // handle typing into input field
                if let Some(CommonAction::Close) = action {
                    self.active_input = InputType::Count;
                    context.render()?;
                    return Ok(());
                } else if let Some(CommonAction::Confirm) = action {
                    Self::add_random(self.selected_tag, &self.count, context, false)?;
                    pop_modal!(context);
                    return Ok(());
                }

                match key.code() {
                    KeyCode::Char(c) => {
                        self.count.push(c);

                        context.render()?;
                    }
                    KeyCode::Backspace => {
                        self.count.pop();

                        context.render()?;
                    }
                    _ => {}
                }
            }
            InputType::Tag => {
                let Some(action) = action else {
                    return Ok(());
                };
                match action {
                    CommonAction::Down => {
                        self.active_input = InputType::Count;
                        context.render()?;
                    }
                    CommonAction::Up => {
                        self.active_input = InputType::Buttons;
                        self.button_group_state.last();
                        context.render()?;
                    }
                    CommonAction::FocusInput | CommonAction::Confirm => {
                        self.selected_tag = self.selected_tag.next();
                        context.render()?;
                    }
                    CommonAction::Close => {
                        pop_modal!(context);
                    }
                    _ => {}
                }
            }
            InputType::Count => {
                let Some(action) = action else {
                    return Ok(());
                };
                match action {
                    CommonAction::Down => {
                        self.active_input = InputType::Buttons;
                        self.button_group_state.first();
                        context.render()?;
                    }
                    CommonAction::Up => {
                        self.active_input = InputType::Tag;
                        context.render()?;
                    }
                    CommonAction::FocusInput | CommonAction::Confirm => {
                        self.active_input = InputType::CountFocused;
                        context.render()?;
                    }
                    CommonAction::Close => {
                        pop_modal!(context);
                    }
                    _ => {}
                }
            }
            InputType::Buttons => {
                // handle switching between inputs and also handle buttons
                let Some(action) = action else {
                    return Ok(());
                };
                let state = &mut self.button_group_state;
                match action {
                    CommonAction::Down => {
                        if state.selected == state.button_count() - 1 {
                            self.active_input = InputType::Tag;
                        } else {
                            self.button_group_state.next();
                        }

                        context.render()?;
                    }
                    CommonAction::Up => {
                        if state.selected == 0 {
                            self.active_input = InputType::Count;
                        } else {
                            self.button_group_state.prev();
                        }

                        context.render()?;
                    }
                    CommonAction::Close => {
                        pop_modal!(context);
                    }
                    CommonAction::Confirm => {
                        if state.selected == 0 {
                            Self::add_random(self.selected_tag, &self.count, context, false)?;
                        }
                        pop_modal!(context);
                    }
                    _ => {}
                }
            }
        }

        Ok(())
    }

    fn handle_mouse_event(&mut self, event: MouseEvent, context: &mut AppContext) -> Result<()> {
        match event.kind {
            MouseEventKind::LeftClick
                if self.input_areas[InputAreas::Tag].contains(event.into()) =>
            {
                self.active_input = InputType::Tag;
                context.render()?;
            }
            MouseEventKind::DoubleClick | MouseEventKind::RightClick
                if self.input_areas[InputAreas::Tag].contains(event.into()) =>
            {
                self.active_input = InputType::Tag;
                self.selected_tag = self.selected_tag.next();
                context.render()?;
            }
            MouseEventKind::LeftClick
                if self.input_areas[InputAreas::Count].contains(event.into()) =>
            {
                self.active_input = InputType::Count;
                context.render()?;
            }
            MouseEventKind::DoubleClick
                if self.input_areas[InputAreas::Count].contains(event.into()) =>
            {
                self.active_input = InputType::CountFocused;
                context.render()?;
            }
            MouseEventKind::LeftClick => {
                if let Some(idx) = self.button_group.get_button_idx_at(event.into()) {
                    self.button_group_state.select(idx);
                    self.active_input = InputType::Buttons;
                    context.render()?;
                }
            }
            MouseEventKind::DoubleClick => {
                match self.button_group.get_button_idx_at(event.into()) {
                    Some(0) => {
                        Self::add_random(self.selected_tag, &self.count, context, false)?;
                        pop_modal!(context);
                    }
                    Some(_) => {
                        pop_modal!(context);
                    }
                    None => {}
                }
            }
            MouseEventKind::MiddleClick => {}
            MouseEventKind::RightClick => {}
            MouseEventKind::ScrollUp => {
                if self.button_group.get_button_idx_at(event.into()).is_some() {
                    self.active_input = InputType::Buttons;
                    self.button_group_state.prev();
                } else {
                    match self.active_input {
                        InputType::Tag => {
                            self.active_input = InputType::Buttons;
                            self.button_group_state.last();
                        }
                        InputType::Count | InputType::CountFocused => {
                            self.active_input = InputType::Tag;
                        }
                        InputType::Buttons
                            if self.button_group_state.selected
                                == self.button_group_state.button_count() - 1 =>
                        {
                            self.button_group_state.prev();
                        }
                        InputType::Buttons => {
                            self.active_input = InputType::Count;
                        }
                    }
                }
                context.render()?;
            }
            MouseEventKind::ScrollDown => {
                if self.button_group.get_button_idx_at(event.into()).is_some() {
                    self.active_input = InputType::Buttons;
                    self.button_group_state.next();
                } else {
                    match self.active_input {
                        InputType::Tag => {
                            self.active_input = InputType::Count;
                        }
                        InputType::Count | InputType::CountFocused => {
                            self.active_input = InputType::Buttons;
                            self.button_group_state.first();
                        }
                        InputType::Buttons if self.button_group_state.selected == 0 => {
                            self.button_group_state.next();
                        }
                        InputType::Buttons => {
                            self.active_input = InputType::Tag;
                        }
                    }
                }
                context.render()?;
            }
        }
        Ok(())
    }
}
