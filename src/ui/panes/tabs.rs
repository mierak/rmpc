use anyhow::{Context, Result};
use ratatui::Frame;
use ratatui::prelude::Rect;

use super::Pane;
use crate::config::tabs::TabName;
use crate::context::AppContext;
use crate::shared::events::AppEvent;
use crate::shared::key_event::KeyEvent;
use crate::shared::mouse_event::MouseEvent;
use crate::ui::widgets::app_tabs::AppTabs;
use crate::ui::{UiAppEvent, UiEvent};

#[derive(Debug)]
pub struct TabsPane<'a> {
    area: Rect,
    active_tab: TabName,
    tab_bar: AppTabs<'a>,
}

impl TabsPane<'_> {
    pub fn new(context: &AppContext) -> Result<Self> {
        let active_tab = *context.config.tabs.names.first().context("Expected at least one tab")?;
        Ok(Self {
            area: Rect::default(),
            tab_bar: AppTabs::new(active_tab, context.config),
            active_tab,
        })
    }
}

impl Pane for TabsPane<'_> {
    fn render(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        _context: &AppContext,
    ) -> anyhow::Result<()> {
        self.area = area;
        if self.area.height > 0 {
            self.tab_bar.set_selected(self.active_tab);
            self.tab_bar.render(self.area, frame.buffer_mut());
        }
        Ok(())
    }

    fn before_show(&mut self, _context: &AppContext) -> Result<()> {
        Ok(())
    }

    fn on_event(
        &mut self,
        event: &mut UiEvent,
        _is_visible: bool,
        context: &AppContext,
    ) -> Result<()> {
        match event {
            UiEvent::TabChanged(tab) => {
                self.active_tab = *tab;
                context.render()?;
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_mouse_event(&mut self, event: MouseEvent, context: &AppContext) -> Result<()> {
        if !self.area.contains(event.into()) {
            return Ok(());
        }

        let Some(tab_name) = self
            .tab_bar
            .get_tab_idx_at(event.into())
            .and_then(|idx| context.config.tabs.names.get(idx))
        else {
            return Ok(());
        };

        if &self.active_tab == tab_name {
            return Ok(());
        }

        context.app_event_sender.send(AppEvent::UiEvent(UiAppEvent::ChangeTab(*tab_name)))?;

        Ok(())
    }

    fn handle_action(&mut self, _event: &mut KeyEvent, _context: &mut AppContext) -> Result<()> {
        Ok(())
    }
}
