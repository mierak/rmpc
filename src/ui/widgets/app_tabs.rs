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
        let tab_names = T::VARIANTS
            .iter()
            .map(|e| format!("{: ^13}", format!("{e}")))
            .collect::<Vec<String>>();

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
            .style(self.config.ui.inactive_tab_style)
            .highlight_style(self.config.ui.active_tab_style);

        tabs.render(area, buf);
    }
}

impl<'a, T: PartialEq + IntoEnumIterator + VariantNames> AppTabs<'a, T> {
    pub fn new(active_tab: T, config: &'a Config) -> Self {
        Self { active_tab, config }
    }
}
