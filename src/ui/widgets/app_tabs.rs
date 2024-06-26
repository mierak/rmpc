use ratatui::widgets::Widget;
use strum::{IntoEnumIterator, VariantNames};

use crate::config::Config;

use super::tabs::Tabs;

pub struct AppTabs<'a, T>
where
    T: PartialEq + IntoEnumIterator + VariantNames,
{
    active_tab: T,
    config: &'a Config,
}

impl<T> Widget for AppTabs<'_, T>
where
    T: PartialEq + IntoEnumIterator + VariantNames,
{
    fn render(self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer) {
        let tab_names = T::VARIANTS.iter().map(|e| format!("{e: ^13}")).collect::<Vec<String>>();

        let Some(selected_tab) = T::iter()
            .enumerate()
            .find(|(_, t)| *t == self.active_tab)
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

impl<'a, T: PartialEq + IntoEnumIterator + VariantNames> AppTabs<'a, T> {
    pub fn new(active_tab: T, config: &'a Config) -> Self {
        Self { active_tab, config }
    }
}
