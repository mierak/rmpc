use ratatui::widgets::Widget;

use crate::config::{tabs::TabName, Config};

use super::tabs::Tabs;

pub struct AppTabs<'a> {
    active_tab: TabName,
    config: &'a Config,
}

impl Widget for AppTabs<'_> {
    fn render(self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer) {
        let tab_names = self
            .config
            .tabs
            .names
            .iter()
            .map(|e| format!("  {e: ^9}  "))
            .collect::<Vec<String>>();

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

        let tabs = Tabs::new(tab_names)
            .select(selected_tab)
            .divider("")
            .block(self.config.as_tabs_block())
            .style(self.config.theme.tab_bar.inactive_style)
            .alignment(ratatui::prelude::Alignment::Center)
            .highlight_style(self.config.theme.tab_bar.active_style);

        tabs.render(area, buf);
    }
}

impl<'a> AppTabs<'a> {
    pub fn new(active_tab: TabName, config: &'a Config) -> Self {
        Self { active_tab, config }
    }
}
