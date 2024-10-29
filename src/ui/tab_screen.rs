use std::collections::HashMap;

use anyhow::Result;
use crossterm::event::KeyEvent;
use ratatui::{
    layout::{Constraint, Layout, Rect},
    widgets::Block,
    Frame,
};

use crate::{
    config::{
        keys::CommonAction,
        tabs::{Pane, PaneOrSplitWithPosition, SubPaneWithPosition},
    },
    context::AppContext,
    mpd::mpd_client::MpdClient,
    shared::{
        geometry::Point,
        id::Id,
        mouse_event::{MouseEvent, MouseEventKind},
    },
    ui::KeyHandleResultInternal,
};

use super::{Pane as _, PaneContainer, Panes};

#[derive(Debug)]
pub struct TabScreen {
    focused: Option<Pane>, // can focused ever be none?
    panes: &'static crate::config::tabs::PaneOrSplitWithPosition,
    pane_areas: HashMap<Id, Rect>,
}

impl TabScreen {
    pub fn new(panes: &'static crate::config::tabs::PaneOrSplitWithPosition) -> Self {
        let focused = panes
            .panes_iter()
            .filter(|Pane { focusable, .. }| *focusable)
            .min_by(|a, b| {
                Point::default()
                    .distance(a.geometry.top_left())
                    .cmp(&Point::default().distance(b.geometry.top_left()))
            });

        Self {
            focused,
            panes,
            pane_areas: HashMap::default(),
        }
    }
}

macro_rules! screen_call {
    ($screen:ident, $fn:ident($($param:expr),+)) => {
        match $screen {
            Panes::Queue(s) => s.$fn($($param),+),
            #[cfg(debug_assertions)]
            Panes::Logs(s) => s.$fn($($param),+),
            Panes::Directories(s) => s.$fn($($param),+),
            Panes::Artists(s) => s.$fn($($param),+),
            Panes::AlbumArtists(s) => s.$fn($($param),+),
            Panes::Albums(s) => s.$fn($($param),+),
            Panes::Playlists(s) => s.$fn($($param),+),
            Panes::Search(s) => s.$fn($($param),+),
            Panes::AlbumArt(s) => s.$fn($($param),+),
        }
    }
}

impl TabScreen {
    pub fn render(
        &mut self,
        panes: &mut PaneContainer,
        frame: &mut Frame,
        area: Rect,
        context: &AppContext,
    ) -> Result<()> {
        self.render_recursive(panes, self.panes, frame, area, context)?;
        Ok(())
    }

    pub fn render_recursive(
        &mut self,
        panes: &mut PaneContainer,
        configured_panes: &PaneOrSplitWithPosition,
        frame: &mut Frame,
        area: Rect,
        context: &AppContext,
    ) -> Result<()> {
        match configured_panes {
            PaneOrSplitWithPosition::Pane(Pane { pane, border, id, .. }) => {
                let block = Block::default()
                    .border_style(if self.focused.is_some_and(|p| p.id == *id) {
                        context.config.as_focused_border_style()
                    } else {
                        context.config.as_border_style()
                    })
                    .borders(*border);
                let pane = panes.get_mut(*pane);
                let pane_area = block.inner(area);
                self.pane_areas.insert(*id, pane_area);
                screen_call!(pane, render(frame, pane_area, context))?;
                frame.render_widget(block, area);
            }
            PaneOrSplitWithPosition::Split {
                direction,
                panes: sub_panes,
            } => {
                let constraints = sub_panes
                    .iter()
                    .map(|SubPaneWithPosition { size, .. }| Constraint::Percentage((*size).into()));
                let areas = Layout::new(*direction, constraints).split(area);

                for (idx, area) in areas.iter().enumerate() {
                    self.render_recursive(panes, &sub_panes[idx].pane, frame, *area, context)?;
                }
            }
        };

        Ok(())
    }

    pub(in crate::ui) fn handle_action(
        &mut self,
        panes: &mut PaneContainer,
        event: KeyEvent,
        client: &mut impl MpdClient,
        context: &AppContext,
    ) -> Result<KeyHandleResultInternal> {
        let Some(focused) = self.focused else {
            return Ok(KeyHandleResultInternal::KeyNotHandled);
        };

        match context.config.keybinds.navigation.get(&event.into()) {
            Some(CommonAction::PaneUp) => {
                self.focused = Some(
                    self.panes
                        .panes_iter()
                        .filter(|p| p.id != focused.id && p.focusable)
                        .fold((focused, u16::MAX), |acc, pane| {
                            if !pane.geometry.is_directly_above(focused.geometry) {
                                return acc;
                            }

                            let dist = pane.geometry.top_left_dist(focused.geometry);
                            if dist > acc.1 {
                                return acc;
                            }

                            (pane, dist)
                        })
                        .0,
                );
                Ok(KeyHandleResultInternal::RenderRequested)
            }
            Some(CommonAction::PaneDown) => {
                self.focused = Some(
                    self.panes
                        .panes_iter()
                        .filter(|p| p.id != focused.id && p.focusable)
                        .fold((focused, u16::MAX), |acc, pane| {
                            if !pane.geometry.is_directly_below(focused.geometry) {
                                return acc;
                            }

                            let dist = pane.geometry.top_left_dist(focused.geometry);
                            if dist > acc.1 {
                                return acc;
                            }

                            (pane, dist)
                        })
                        .0,
                );
                Ok(KeyHandleResultInternal::RenderRequested)
            }
            Some(CommonAction::PaneRight) => {
                self.focused = Some(
                    self.panes
                        .panes_iter()
                        .filter(|p| p.id != focused.id && p.focusable)
                        .fold((focused, u16::MAX), |acc, pane| {
                            if !pane.geometry.is_directly_right(focused.geometry) {
                                return acc;
                            }

                            let dist = pane.geometry.top_left_dist(focused.geometry);
                            if dist > acc.1 {
                                return acc;
                            }

                            (pane, dist)
                        })
                        .0,
                );
                Ok(KeyHandleResultInternal::RenderRequested)
            }
            Some(CommonAction::PaneLeft) => {
                self.focused = Some(
                    self.panes
                        .panes_iter()
                        .filter(|p| p.id != focused.id && p.focusable)
                        .fold((focused, u16::MAX), |acc, pane| {
                            if !pane.geometry.is_directly_left(focused.geometry) {
                                return acc;
                            }

                            let dist = pane.geometry.top_left_dist(focused.geometry);
                            if dist > acc.1 {
                                return acc;
                            }

                            (pane, dist)
                        })
                        .0,
                );
                Ok(KeyHandleResultInternal::RenderRequested)
            }
            Some(_) | None => {
                let pane = panes.get_mut(focused.pane);
                screen_call!(pane, handle_action(event, client, context))
            }
        }
    }

    pub(in crate::ui) fn handle_mouse_event(
        &mut self,
        panes: &mut PaneContainer,
        event: MouseEvent,
        client: &mut impl MpdClient,
        context: &mut AppContext,
    ) -> Result<KeyHandleResultInternal> {
        let mut result = KeyHandleResultInternal::KeyNotHandled;
        if matches!(event.kind, MouseEventKind::LeftClick) {
            let Some(pane) = self
                .pane_areas
                .iter()
                .find(|(_, area)| area.contains(event.into()))
                .and_then(|(pane_id, _)| {
                    self.panes
                        .panes_iter()
                        .find(|pane| &pane.id == pane_id && pane.focusable)
                })
            else {
                return Ok(KeyHandleResultInternal::KeyNotHandled);
            };
            self.focused = Some(pane);
            result = KeyHandleResultInternal::RenderRequested;
        }

        let Some(focused) = self.focused else {
            return Ok(result);
        };

        let pane = panes.get_mut(focused.pane);
        match screen_call!(pane, handle_mouse_event(event, client, context))? {
            KeyHandleResultInternal::Modal(modal) => Ok(KeyHandleResultInternal::Modal(modal)),
            KeyHandleResultInternal::RenderRequested => Ok(KeyHandleResultInternal::RenderRequested),
            KeyHandleResultInternal::FullRenderRequested => Ok(KeyHandleResultInternal::FullRenderRequested),
            _ => Ok(result),
        }
    }

    pub fn post_render(&mut self, panes: &mut PaneContainer, frame: &mut Frame, context: &AppContext) -> Result<()> {
        for pane in self.panes.panes_iter() {
            let screen = panes.get_mut(pane.pane);
            screen_call!(screen, post_render(frame, context))?;
        }
        Ok(())
    }

    pub fn on_hide(
        &mut self,
        panes: &mut PaneContainer,
        client: &mut impl MpdClient,
        context: &AppContext,
    ) -> Result<()> {
        for pane in self.panes.panes_iter() {
            let screen = panes.get_mut(pane.pane);
            screen_call!(screen, on_hide(client, context))?;
        }
        Ok(())
    }

    pub fn before_show(
        &mut self,
        panes: &mut PaneContainer,
        client: &mut impl MpdClient,
        context: &AppContext,
    ) -> Result<()> {
        for pane in self.panes.panes_iter() {
            let screen = panes.get_mut(pane.pane);
            screen_call!(screen, before_show(client, context))?;
        }
        Ok(())
    }
}
