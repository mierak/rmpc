use anyhow::{Context, Result};
use ratatui::{
    Frame,
    layout::Position,
    prelude::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::Widget,
};

use super::Pane;
use crate::{
    config::tabs::TabName,
    ctx::Ctx,
    shared::{
        events::AppEvent,
        keys::ActionEvent,
        mouse_event::{MouseEvent, MouseEventKind},
    },
    ui::{UiAppEvent, UiEvent, widgets::tabs::Tabs},
};

#[derive(Debug)]
pub struct TabsPane<'a> {
    area: Rect,
    active_tab: TabName,
    tabs: Tabs<'a>,
}

impl TabsPane<'_> {
    pub fn new(ctx: &Ctx) -> Result<Self> {
        let active_tab = Self::init_active_tab(ctx)?;
        let tab_names = Self::init_tab_names(ctx);
        let tabs = Self::init_tabs(tab_names, ctx);

        Ok(Self { area: Rect::default(), active_tab, tabs })
    }

    pub fn get_tab_idx_at(&self, position: Position) -> Option<usize> {
        self.tabs.areas.iter().enumerate().find(|(_, area)| area.contains(position)).map(|v| v.0)
    }

    fn init_active_tab(ctx: &Ctx) -> Result<TabName> {
        Ok(ctx.config.tabs.names.first().context("Expected at least one tab")?.clone())
    }

    fn init_tab_names(ctx: &Ctx) -> Vec<String> {
        ctx.config.tabs.names.iter().map(|e| format!("  {e: ^9}  ")).collect::<Vec<String>>()
    }

    fn init_tabs<'a>(tab_names: Vec<String>, ctx: &Ctx) -> Tabs<'a> {
        Tabs::new(tab_names)
            .divider("")
            .style(ctx.config.theme.tab_bar.inactive_style)
            .alignment(ratatui::prelude::Alignment::Center)
            // Active styling is baked into the per-tab pill Line in `build_titles`,
            // so the widget highlight must be a no-op (otherwise it would flatten
            // the rounded powerline caps with a solid accent background).
            .highlight_style(Style::default())
    }

    /// Build pill-shaped tab titles: the active tab is wrapped in rounded
    /// powerline caps (\u{e0b6}/\u{e0b4}) so it reads as a rounded pill,
    /// matching the design. Inactive tabs are plain muted labels.
    fn build_titles(ctx: &Ctx, active_idx: usize) -> Vec<Line<'static>> {
        let theme = &ctx.config.theme;
        let active = theme.tab_bar.active_style;
        let inactive = theme.tab_bar.inactive_style;
        let accent = active.bg.unwrap_or(Color::Cyan);
        ctx.config
            .tabs
            .names
            .iter()
            .enumerate()
            .map(|(i, name)| {
                let label = format!(" {name}  {} ", i + 1);
                if i == active_idx {
                    Line::from(vec![
                        Span::styled("\u{e0b6}", Style::default().fg(accent)),
                        Span::styled(label, active),
                        Span::styled("\u{e0b4}", Style::default().fg(accent)),
                    ])
                } else {
                    Line::from(vec![Span::styled(format!("  {label} "), inactive)])
                }
            })
            .collect()
    }
}

impl Pane for TabsPane<'_> {
    fn render(&mut self, frame: &mut Frame, area: Rect, ctx: &Ctx) -> anyhow::Result<()> {
        self.area = area;
        if self.area.height > 0 {
            let Some(selected_tab) = ctx
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

            self.tabs.titles(Self::build_titles(ctx, selected_tab));
            self.tabs.select(selected_tab);
            self.tabs.render(area, frame.buffer_mut());
        }
        Ok(())
    }

    fn before_show(&mut self, _ctx: &Ctx) -> Result<()> {
        Ok(())
    }

    fn on_event(&mut self, event: &mut UiEvent, _is_visible: bool, ctx: &Ctx) -> Result<()> {
        match event {
            UiEvent::TabChanged(tab) => {
                self.active_tab = tab.clone();
                ctx.render()?;
            }
            UiEvent::ConfigChanged => {
                let new_active_tab = ctx
                    .config
                    .tabs
                    .names
                    .iter()
                    .find(|tab| tab == &&self.active_tab)
                    .or(ctx.config.tabs.names.first())
                    .context("Expected at least one tab")
                    .cloned()?;

                let tab_names = Self::init_tab_names(ctx);
                self.tabs = Self::init_tabs(tab_names, ctx);

                self.active_tab = new_active_tab;
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_mouse_event(&mut self, event: MouseEvent, ctx: &Ctx) -> Result<()> {
        if !self.area.contains(event.into()) {
            return Ok(());
        }

        if !matches!(event.kind, MouseEventKind::LeftClick | MouseEventKind::DoubleClick) {
            return Ok(());
        }

        let Some(tab_name) =
            self.get_tab_idx_at(event.into()).and_then(|idx| ctx.config.tabs.names.get(idx))
        else {
            return Ok(());
        };

        if &self.active_tab == tab_name {
            return Ok(());
        }

        ctx.app_event_sender.send(AppEvent::UiAppEvent(UiAppEvent::ChangeTab(tab_name.clone())))?;

        Ok(())
    }

    fn handle_action(&mut self, _event: &mut ActionEvent, _ctx: &mut Ctx) -> Result<()> {
        Ok(())
    }
}
