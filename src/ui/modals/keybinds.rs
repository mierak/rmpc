use std::{borrow::Cow, collections::HashMap, fmt::Display};

use anyhow::Result;
use itertools::Itertools;
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Margin, Rect},
    macros::constraint,
    style::Style,
    symbols::border,
    text::{Line, Text},
    widgets::{Block, Borders, Cell, Clear, Row, Table, TableState},
};
use strum::{IntoDiscriminant, VariantArray};

use super::Modal;
use crate::{
    config::keys::{CommonAction, ToDescription},
    ctx::Ctx,
    shared::{
        ext::iter::IntoZipLongest2,
        id::{self, Id},
        keys::ActionEvent,
        mouse_event::{MouseEvent, MouseEventKind},
    },
    status_warn,
    ui::{
        FILTER_PREFIX,
        dirstack::DirState,
        input::{BufferId, InputResultEvent},
    },
};

#[derive(Debug)]
pub struct KeybindsModal {
    id: Id,
    scrolling_state: DirState<TableState>,
    table_area: Rect,
    filter_active: bool,
    filter_rows: Vec<Option<String>>,
    filter_buffer_id: BufferId,
}

trait KeybindsExt {
    fn sort_by_action(&self) -> impl Iterator<Item = (String, String, Cow<'static, str>)>;
}

impl<K, ActionEnum, Discriminant> KeybindsExt for HashMap<K, ActionEnum>
where
    K: Display + std::cmp::PartialEq<K>,
    ActionEnum: Display + ToDescription + IntoDiscriminant<Discriminant = Discriminant>,
    Discriminant: VariantArray + std::cmp::PartialEq<Discriminant>,
{
    fn sort_by_action(&self) -> impl Iterator<Item = (String, String, Cow<'static, str>)> {
        Discriminant::VARIANTS.iter().flat_map(|variant| {
            self.iter()
                .filter(|(_, v)| &v.discriminant() == variant)
                .map(|(k, v)| (k.to_string(), v))
                .sorted_by(|a, b| a.0.cmp(&b.0))
                .map(|(k, v)| (k, v.to_string(), v.to_description()))
        })
    }
}

impl KeybindsModal {
    pub fn new(_ctx: &mut Ctx) -> Self {
        let mut scrolling_state = DirState::default();
        scrolling_state.select(Some(0), 0);

        Self {
            id: id::new(),
            scrolling_state,
            table_area: Rect::default(),
            filter_active: false,
            filter_rows: Vec::new(),
            filter_buffer_id: BufferId::new(),
        }
    }

    pub fn jump_forward(&mut self, scrolloff: usize, ctx: &Ctx) {
        if !self.filter_active {
            status_warn!("No filter set");
            return;
        }
        let filter = ctx.input.value(self.filter_buffer_id);
        let Some(selected) = self.scrolling_state.get_selected() else {
            log::error!(state:? = self.scrolling_state; "No song selected");
            return;
        };

        let length = self.filter_rows.len();
        for i in selected + 1..length + selected {
            let i = i % length;
            if let Some(row) = &self.filter_rows[i]
                && !row.is_empty()
                && row.contains(&filter)
            {
                self.scrolling_state.select(Some(i), scrolloff);
                break;
            }
        }
    }

    pub fn jump_back(&mut self, scrolloff: usize, ctx: &Ctx) {
        if !self.filter_active {
            status_warn!("No filter set");
            return;
        }
        let Some(selected) = self.scrolling_state.get_selected() else {
            log::error!(state:? = self.scrolling_state; "No song selected");
            return;
        };

        let filter = ctx.input.value(self.filter_buffer_id);
        let length = self.filter_rows.len();
        for i in (0..length).rev() {
            let i = (i + selected) % length;
            if let Some(row) = &self.filter_rows[i]
                && !row.is_empty()
                && row.contains(&filter)
            {
                self.scrolling_state.select(Some(i), scrolloff);
                break;
            }
        }
    }

    pub fn jump_first(&mut self, scrolloff: usize, ctx: &Ctx) {
        if !self.filter_active {
            status_warn!("No filter set");
            return;
        }

        let filter = ctx.input.value(self.filter_buffer_id);
        self.filter_rows
            .iter()
            .enumerate()
            .find(|(_, item)| item.as_ref().is_some_and(|item| item.contains(&filter)))
            .inspect(|(idx, _)| self.scrolling_state.select(Some(*idx), scrolloff));
    }
}
fn row_header<'a>(
    keys: &'a [(String, String, Cow<'a, str>)],
    name: &'a str,
    header_style: Style,
) -> Option<Row<'a>> {
    if keys.is_empty() {
        None
    } else {
        Some(Row::new(vec![
            Line::raw(Cow::<str>::Borrowed(" ")).patch_style(header_style),
            Line::raw(Cow::<str>::Borrowed(name)).patch_style(header_style).centered(),
            Line::raw(Cow::<str>::Borrowed(" ")).patch_style(header_style),
        ]))
    }
}

fn row<'a>(
    keys: &'a [(String, String, Cow<'a, str>)],
    key_width: u16,
    action_width: u16,
    description_width: u16,
    filter: Option<&str>,
    match_style: Style,
) -> impl Iterator<Item = (String, Row<'a>)> {
    keys.iter().flat_map(move |(key, action, description)| {
        let matches = if let Some(filter) = filter {
            key.to_lowercase().contains(&filter.to_lowercase())
                || action.to_lowercase().contains(&filter.to_lowercase())
                || description.to_lowercase().contains(&filter.to_lowercase())
        } else {
            false
        };

        let key = textwrap::wrap(key, key_width as usize);
        let action = textwrap::wrap(action, action_width as usize);
        let description = textwrap::wrap(description, description_width as usize);

        key.into_iter().zip_longest2(action.into_iter(), description.into_iter()).enumerate().map(
            move |(idx, (key, action, description))| {
                let mut result = if idx == 0 && filter.is_some() {
                    (
                        [
                            key.as_ref().map(|v| v.to_lowercase()).unwrap_or_default(),
                            action.as_ref().map(|v| v.to_lowercase()).unwrap_or_default(),
                            description.as_ref().map(|v| v.to_lowercase()).unwrap_or_default(),
                        ]
                        .join(""),
                        Row::new([
                            Cell::from(Text::from(key.unwrap_or_default())),
                            Cell::from(Text::from(action.unwrap_or_default())),
                            Cell::from(Text::from(description.unwrap_or_default())),
                        ]),
                    )
                } else {
                    (
                        String::new(),
                        Row::new([
                            Cell::from(Text::from(key.unwrap_or_default())),
                            Cell::from(Text::from(action.unwrap_or_default())),
                            Cell::from(Text::from(description.unwrap_or_default())),
                        ]),
                    )
                };

                if matches {
                    result.1 = result.1.style(match_style);
                }

                result
            },
        )
    })
}

impl Modal for KeybindsModal {
    fn id(&self) -> Id {
        self.id
    }

    fn render(&mut self, frame: &mut Frame, ctx: &mut Ctx) -> Result<()> {
        let popup_area = frame.area().centered(constraint!(==90%), constraint!(==90%));
        frame.render_widget(Clear, popup_area);
        if let Some(bg_color) = ctx.config.theme.modal_background_color {
            frame.render_widget(Block::default().style(Style::default().bg(bg_color)), popup_area);
        }
        let filter = ctx.input.value(self.filter_buffer_id);

        let mut block = Block::default()
            .borders(Borders::ALL)
            .border_set(border::ROUNDED)
            .border_style(ctx.config.as_border_style())
            .title_alignment(ratatui::prelude::Alignment::Center);
        if self.filter_active {
            block = block.title(format!("Keybinds | {FILTER_PREFIX} {filter}"));
        } else {
            block = block.title("Keybinds");
        }

        let margin = Margin { horizontal: 1, vertical: 0 };
        let [header_area, table_area] =
            Layout::vertical([Constraint::Length(2), Constraint::Percentage(100)])
                .areas(block.inner(popup_area));
        let header_area = header_area.inner(margin);
        let table_area = table_area.inner(margin);

        let (key_width, action_width, desc_width) = (20, 30, 50);
        let constraints = [
            Constraint::Percentage(key_width),
            Constraint::Percentage(action_width),
            Constraint::Percentage(desc_width),
        ];
        let [key_area, mut action_area, mut desc_area] =
            Layout::horizontal(constraints).areas(table_area);
        action_area.width = action_area.width.saturating_sub(1); // account for the column spacing
        desc_area.width = desc_area.width.saturating_sub(2); // account for the column spacing

        let keybinds = &ctx.config.keybinds;
        let header_style = ctx.config.theme.current_item_style;

        let global = keybinds.global.sort_by_action().collect_vec();
        let navigation = keybinds.navigation.sort_by_action().collect_vec();
        let queue = keybinds.queue.sort_by_action().collect_vec();
        let global_rows: (Vec<_>, Vec<_>) = row(
            &global,
            key_area.width,
            action_area.width,
            desc_area.width,
            self.filter_active.then_some(filter.as_str()),
            ctx.config.theme.highlighted_item_style,
        )
        .unzip();
        let nav_rows: (Vec<_>, Vec<_>) = row(
            &navigation,
            key_area.width,
            action_area.width,
            desc_area.width,
            self.filter_active.then_some(filter.as_str()),
            ctx.config.theme.highlighted_item_style,
        )
        .unzip();
        let queue_rows: (Vec<_>, Vec<_>) = row(
            &queue,
            key_area.width,
            action_area.width,
            desc_area.width,
            self.filter_active.then_some(filter.as_str()),
            ctx.config.theme.highlighted_item_style,
        )
        .unzip();

        let rows = row_header(&global, "Global", header_style)
            .into_iter()
            .chain(global_rows.1)
            .chain(row_header(&navigation, "Navigation", header_style))
            .chain(nav_rows.1)
            .chain(row_header(&queue, "Queue", header_style))
            .chain(queue_rows.1)
            .collect_vec();

        self.filter_rows = Vec::new();
        self.filter_rows.push(None);
        self.filter_rows.extend(global_rows.0.into_iter().map(Some));
        self.filter_rows.push(None);
        self.filter_rows.extend(nav_rows.0.into_iter().map(Some));
        self.filter_rows.push(None);
        self.filter_rows.extend(queue_rows.0.into_iter().map(Some));

        self.scrolling_state.set_content_and_viewport_len(rows.len(), table_area.height.into());

        let header_table = Table::new(
            vec![Row::new([Cell::from("Key"), Cell::from("Action"), Cell::from("Description")])],
            constraints,
        )
        .column_spacing(1)
        .block(
            Block::default().borders(Borders::BOTTOM).border_style(ctx.config.as_border_style()),
        );

        let table = Table::new(rows, constraints)
            .column_spacing(1)
            .style(ctx.config.as_text_style())
            .row_highlight_style(ctx.config.theme.current_item_style);

        self.table_area = table_area;

        frame.render_widget(block, popup_area);
        frame.render_widget(header_table, header_area);
        frame.render_stateful_widget(table, table_area, self.scrolling_state.as_render_state_ref());
        if let Some(scrollbar) = ctx.config.as_styled_scrollbar() {
            frame.render_stateful_widget(
                scrollbar,
                popup_area.inner(Margin { horizontal: 0, vertical: 1 }),
                self.scrolling_state.as_scrollbar_state_ref(),
            );
        }

        return Ok(());
    }

    fn handle_insert_mode(&mut self, kind: InputResultEvent, ctx: &Ctx) -> Result<()> {
        match kind {
            InputResultEvent::Push => {
                self.jump_first(ctx.config.scrolloff, ctx);
            }
            InputResultEvent::Pop => {}
            InputResultEvent::Confirm => {}
            InputResultEvent::Cancel => {
                ctx.input.clear_buffer(self.filter_buffer_id);
                self.filter_active = false;
            }
            InputResultEvent::NoChange => {}
        }
        ctx.render()?;
        Ok(())
    }

    fn handle_key(&mut self, key: &mut ActionEvent, ctx: &mut Ctx) -> Result<()> {
        if let Some(action) = key.claim_common() {
            match action {
                CommonAction::DownHalf => {
                    self.scrolling_state.next_half_viewport(ctx.config.scrolloff);

                    ctx.render()?;
                }
                CommonAction::UpHalf => {
                    self.scrolling_state.prev_half_viewport(ctx.config.scrolloff);

                    ctx.render()?;
                }
                CommonAction::Up => {
                    self.scrolling_state.prev(ctx.config.scrolloff, ctx.config.wrap_navigation);

                    ctx.render()?;
                }
                CommonAction::Down => {
                    self.scrolling_state.next(ctx.config.scrolloff, ctx.config.wrap_navigation);

                    ctx.render()?;
                }
                CommonAction::Bottom => {
                    self.scrolling_state.last();

                    ctx.render()?;
                }
                CommonAction::Top => {
                    self.scrolling_state.first();

                    ctx.render()?;
                }
                CommonAction::Close => {
                    self.hide(ctx)?;
                }
                CommonAction::EnterSearch => {
                    ctx.input.clear_buffer(self.filter_buffer_id);
                    ctx.input.insert_mode(self.filter_buffer_id);
                    self.filter_active = true;

                    ctx.render()?;
                }
                CommonAction::NextResult => {
                    self.jump_forward(ctx.config.scrolloff, ctx);

                    ctx.render()?;
                }
                CommonAction::PreviousResult => {
                    self.jump_back(ctx.config.scrolloff, ctx);

                    ctx.render()?;
                }
                _ => {}
            }
        }

        Ok(())
    }

    fn handle_mouse_event(&mut self, event: MouseEvent, ctx: &mut Ctx) -> Result<()> {
        if !self.table_area.contains(event.into()) {
            return Ok(());
        }

        match event.kind {
            MouseEventKind::LeftClick | MouseEventKind::DoubleClick => {
                let y: usize = event.y.saturating_sub(self.table_area.y).into();
                if let Some(idx) = self.scrolling_state.get_at_rendered_row(y) {
                    self.scrolling_state.select(Some(idx), ctx.config.scrolloff);
                    ctx.render()?;
                }
            }
            MouseEventKind::MiddleClick => {}
            MouseEventKind::RightClick => {}
            MouseEventKind::ScrollDown => {
                self.scrolling_state.scroll_down(ctx.config.scroll_amount, ctx.config.scrolloff);
                ctx.render()?;
            }
            MouseEventKind::ScrollUp => {
                self.scrolling_state.scroll_up(ctx.config.scroll_amount, ctx.config.scrolloff);
                ctx.render()?;
            }
            MouseEventKind::Drag { drag_start_position: _ } => {}
        }

        Ok(())
    }
}
