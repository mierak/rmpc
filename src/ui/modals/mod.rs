use std::borrow::Cow;

use anyhow::Result;
use ratatui::{
    Frame,
    prelude::{Constraint, Layout, Rect},
    symbols,
};

use super::UiEvent;
use crate::{
    MpdQueryResult,
    ctx::Ctx,
    shared::{id::Id, key_event::KeyEvent, mouse_event::MouseEvent},
    ui::input::InputResultEvent,
};

pub mod add_random_modal;
pub mod confirm_modal;
pub mod decoders;
pub mod info_list_modal;
pub mod info_modal;
pub mod input_modal;
pub mod keybinds;
pub mod menu;
pub mod outputs;
pub mod select_modal;

#[allow(unused)]
pub(crate) trait Modal: std::fmt::Debug {
    fn id(&self) -> Id;

    fn render(&mut self, frame: &mut Frame, ctx: &mut crate::ctx::Ctx) -> Result<()>;

    fn handle_insert_mode(&mut self, kind: InputResultEvent, ctx: &Ctx) -> Result<()> {
        Ok(())
    }

    fn handle_key(&mut self, key: &mut KeyEvent, ctx: &mut Ctx) -> Result<()>;

    fn handle_mouse_event(&mut self, event: MouseEvent, ctx: &mut Ctx) -> Result<()>;

    fn on_query_finished(
        &mut self,
        id: &'static str,
        data: &mut MpdQueryResult,
        ctx: &Ctx,
    ) -> Result<()> {
        Ok(())
    }

    fn on_event(&mut self, event: &mut UiEvent, ctx: &Ctx) -> Result<()> {
        Ok(())
    }

    fn replacement_id(&self) -> Option<&Cow<'static, str>> {
        None
    }

    fn hide(&mut self, ctx: &Ctx) -> Result<()> {
        ctx.app_event_sender
            .send(crate::AppEvent::UiEvent(crate::ui::UiAppEvent::PopModal(self.id())))?;
        Ok(())
    }
}

const BUTTON_GROUP_SYMBOLS: symbols::border::Set = symbols::border::Set {
    top_right: symbols::line::NORMAL.vertical_left,
    top_left: symbols::line::NORMAL.vertical_right,
    ..symbols::border::ROUNDED
};

#[allow(dead_code)]
pub trait RectExt {
    fn centered(&self, width_percent: u16, height_percent: u16) -> Rect;
    fn centered_exact(&self, width: u16, height: u16) -> Rect;
}

impl RectExt for Rect {
    fn centered(&self, width_percent: u16, height_percent: u16) -> Rect {
        let popup_layout = Layout::vertical([
            Constraint::Percentage((100 - height_percent) / 2),
            Constraint::Percentage(height_percent),
            Constraint::Percentage((100 - height_percent) / 2),
        ])
        .split(*self);

        Layout::horizontal([
            Constraint::Percentage((100 - width_percent) / 2),
            Constraint::Percentage(width_percent),
            Constraint::Percentage((100 - width_percent) / 2),
        ])
        .split(popup_layout[1])[1]
    }

    fn centered_exact(&self, width: u16, height: u16) -> Rect {
        let popup_layout = Layout::vertical([
            Constraint::Length((self.height.saturating_sub(height)) / 2),
            Constraint::Length(height),
            Constraint::Length((self.height.saturating_sub(height)) / 2),
        ])
        .split(*self);

        Layout::horizontal([
            Constraint::Length((self.width.saturating_sub(width)) / 2),
            Constraint::Length(width),
            Constraint::Length((self.width.saturating_sub(width)) / 2),
        ])
        .split(popup_layout[1])[1]
    }
}

#[cfg(test)]
mod tests {
    use ratatui::prelude::Rect;

    use super::RectExt;

    #[test]
    fn exact() {
        let input = Rect { x: 25, y: 25, width: 250, height: 250 };

        let result = input.centered_exact(60, 50);

        assert_eq!(result, Rect { x: 120, y: 125, width: 60, height: 50 });
    }

    #[test]
    fn exact_width_exceeded_gives_max_possible_size() {
        let input = Rect { x: 25, y: 25, width: 10, height: 10 };

        let result = input.centered_exact(60, 50);

        assert_eq!(result, input);
    }
}
