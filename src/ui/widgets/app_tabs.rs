use ratatui::{
    layout::{Position, Rect},
    widgets::Widget,
};

use super::tabs::Tabs;
use crate::config::{Config, tabs::TabName};

#[derive(Debug)]
pub struct AppTabs<'a> {
    active_tab: TabName,
    config: &'a Config,
    tabs: Tabs<'a>,
}

impl AppTabs<'_> {
    pub fn render(&mut self, area: Rect, buf: &mut ratatui::prelude::Buffer) {
        let Some(selected_tab) = self
            .config
            .tabs
            .names
            .iter()
            .enumerate()
            .find(|(_, t)| **t == self.active_tab)
            .map(|(idx, _)| idx)
        else {
            return;
        };

        self.tabs.select(selected_tab);
        self.tabs.render(area, buf);
    }

    pub fn set_selected(&mut self, tab: TabName) -> &mut Self {
        self.active_tab = tab;
        self
    }

    pub fn get_tab_idx_at(&self, position: Position) -> Option<usize> {
        self.tabs.areas.iter().enumerate().find(|(_, area)| area.contains(position)).map(|v| v.0)
    }
}

impl<'a> AppTabs<'a> {
    pub fn new(active_tab: TabName, config: &'a Config) -> Self {
        let tab_names =
            config.tabs.names.iter().map(|e| format!("  {e: ^9}  ")).collect::<Vec<String>>();

        let tabs = Tabs::new(tab_names)
            .divider("")
            .block(config.as_tabs_block())
            .style(config.theme.tab_bar.inactive_style)
            .alignment(ratatui::prelude::Alignment::Center)
            .highlight_style(config.theme.tab_bar.active_style);

        Self { active_tab, config, tabs }
    }
}
