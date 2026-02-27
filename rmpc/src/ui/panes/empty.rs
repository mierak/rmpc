use anyhow::Result;
use ratatui::{Frame, layout::Rect};

use crate::{ctx::Ctx, shared::keys::ActionEvent, ui::panes::Pane};

#[derive(Debug)]
pub struct EmptyPane;

impl Pane for EmptyPane {
    fn render(&mut self, _frame: &mut Frame, _area: Rect, _ctx: &Ctx) -> Result<()> {
        Ok(())
    }

    fn handle_action(&mut self, _event: &mut ActionEvent, _ctx: &mut Ctx) -> Result<()> {
        Ok(())
    }
}
