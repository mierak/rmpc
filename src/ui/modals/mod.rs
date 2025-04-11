use anyhow::Result;
use ratatui::{
    Frame,
    prelude::{Constraint, Layout, Rect},
};

use crate::{
    MpdQueryResult,
    context::AppContext,
    shared::{key_event::KeyEvent, mouse_event::MouseEvent},
};

pub mod confirm_modal;
pub mod decoders;
pub mod info_modal;
pub mod input_modal;
pub mod keybinds;
pub mod outputs;
pub mod select_modal;
pub mod song_info;

#[allow(unused)]
pub(crate) trait Modal: std::fmt::Debug {
    fn render(&mut self, frame: &mut Frame, app: &mut crate::context::AppContext) -> Result<()>;

    fn handle_key(&mut self, key: &mut KeyEvent, app: &mut AppContext) -> Result<()>;

    fn handle_mouse_event(&mut self, event: MouseEvent, context: &mut AppContext) -> Result<()>;

    fn on_query_finished(
        &mut self,
        id: &'static str,
        data: &mut MpdQueryResult,
        context: &AppContext,
    ) -> Result<()> {
        Ok(())
    }
}

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
