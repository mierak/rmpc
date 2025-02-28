use anyhow::{Context, Result};
use ratatui::{Frame, layout::Position, prelude::Rect, widgets::Widget};

use super::Pane;
use crate::{
    config::tabs::TabName,
    context::AppContext,
    shared::{events::AppEvent, key_event::KeyEvent, mouse_event::MouseEvent},
    ui::{UiAppEvent, UiEvent, widgets::tabs::Tabs},
};

#[derive(Debug)]
pub struct TabsPane<'a> {
    area: Rect,
    active_tab: TabName,
    tabs: Tabs<'a>,
}

impl TabsPane<'_> {
    pub fn new(context: &AppContext) -> Result<Self> {
        let active_tab =
            context.config.tabs.names.first().context("Expected at least one tab")?.clone();
        let tab_names = context
            .config
            .tabs
            .names
            .iter()
            .map(|e| format!("  {e: ^9}  "))
            .collect::<Vec<String>>();

        let tabs = Tabs::new(tab_names)
            .divider("")
            .block(context.config.as_tabs_block())
            .style(context.config.theme.tab_bar.inactive_style)
            .alignment(ratatui::prelude::Alignment::Center)
            .highlight_style(context.config.theme.tab_bar.active_style);

        Ok(Self { area: Rect::default(), active_tab, tabs })
    }

    pub fn get_tab_idx_at(&self, position: Position) -> Option<usize> {
        self.tabs.areas.iter().enumerate().find(|(_, area)| area.contains(position)).map(|v| v.0)
    }
}

impl Pane for TabsPane<'_> {
    fn render(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        context: &AppContext,
    ) -> anyhow::Result<()> {
        self.area = area;
        if self.area.height > 0 {
            let Some(selected_tab) = context
                .config
                .tabs
                .names
                .iter()
                .enumerate()
                .find(|(_, t)| **t == self.active_tab)
                .map(|(idx, _)| idx)
            else {
                return Ok(());
            };

            self.tabs.select(selected_tab);
            self.tabs.render(area, frame.buffer_mut());
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
                self.active_tab = tab.clone();
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

        let Some(tab_name) =
            self.get_tab_idx_at(event.into()).and_then(|idx| context.config.tabs.names.get(idx))
        else {
            return Ok(());
        };

        if &self.active_tab == tab_name {
            return Ok(());
        }

        context
            .app_event_sender
            .send(AppEvent::UiEvent(UiAppEvent::ChangeTab(tab_name.clone())))?;

        Ok(())
    }

    fn handle_action(&mut self, _event: &mut KeyEvent, _context: &mut AppContext) -> Result<()> {
        Ok(())
    }
}
