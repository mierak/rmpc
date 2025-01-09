use std::collections::HashMap;

use anyhow::Result;
use ratatui::{
    layout::{Constraint, Layout, Rect},
    widgets::Block,
    Frame,
};

use crate::{
    config::{
        keys::CommonAction,
        tabs::{Pane, SizedPaneOrSplit, SizedSubPane},
    },
    context::AppContext,
    shared::{
        id::Id,
        key_event::KeyEvent,
        mouse_event::{MouseEvent, MouseEventKind},
    },
};

use super::{Pane as _, PaneContainer, Panes};

#[derive(Debug)]
pub struct TabScreen {
    focused: Option<Pane>, // can focused ever be none?
    pub panes: &'static crate::config::tabs::SizedPaneOrSplit,
    pane_areas: HashMap<Id, (Rect, bool)>,
    initialized: bool,
}

impl TabScreen {
    pub fn new(panes: &'static crate::config::tabs::SizedPaneOrSplit) -> Self {
        Self {
            panes,
            initialized: false,
            pane_areas: HashMap::default(),
            focused: panes.panes_iter().next(),
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
            Panes::Lyrics(s) => s.$fn($($param),+),
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
        self.for_each_pane(
            panes,
            self.panes,
            area,
            context,
            &mut |pane, area, block, block_area| {
                screen_call!(pane, render(frame, area, context))?;
                frame.render_widget(block, block_area);
                Ok(())
            },
        )?;
        Ok(())
    }

    pub fn for_each_pane(
        &mut self,
        panes: &mut PaneContainer,
        configured_panes: &SizedPaneOrSplit,
        area: Rect,
        context: &AppContext,
        callback: &mut impl FnMut(&mut Panes<'_>, Rect, Block, Rect) -> Result<()>,
    ) -> Result<()> {
        match configured_panes {
            SizedPaneOrSplit::Pane(Pane {
                pane,
                border,
                id,
                focusable,
                ..
            }) => {
                let block = Block::default()
                    .border_style(if self.focused.is_some_and(|p| p.id == *id) {
                        context.config.as_focused_border_style()
                    } else {
                        context.config.as_border_style()
                    })
                    .borders(*border);
                let mut pane = panes.get_mut(*pane);
                let pane_area = block.inner(area);
                self.pane_areas.insert(*id, (pane_area, *focusable));
                callback(&mut pane, pane_area, block, area)?;
            }
            SizedPaneOrSplit::Split {
                direction,
                panes: sub_panes,
            } => {
                let constraints = sub_panes
                    .iter()
                    .map(|SizedSubPane { size, .. }| Into::<Constraint>::into(*size));
                let areas = Layout::new(*direction, constraints).split(area);

                for (idx, area) in areas.iter().enumerate() {
                    self.for_each_pane(panes, &sub_panes[idx].pane, *area, context, callback)?;
                }
            }
        };

        Ok(())
    }

    pub(in crate::ui) fn handle_action(
        &mut self,
        panes: &mut PaneContainer,
        event: &mut KeyEvent,
        context: &mut AppContext,
    ) -> Result<()> {
        let Some(focused) = self.focused else {
            return Ok(());
        };

        match event.as_common_action(context) {
            Some(CommonAction::PaneUp) => {
                let Some((focused_area, _)) = self.pane_areas.get(&focused.id) else {
                    log::warn!(focused:?, pane_areas:? = self.pane_areas; "Tried to find focused pane area but it does not exist");
                    return Ok(());
                };
                self.focused = self
                    .pane_areas
                    .iter()
                    .filter(|(_, (area, focusable))| *focusable && area.bottom() < focused_area.top())
                    .max_by(|(_, (area_a, _)), (_, (area_b, _))| area_a.top().cmp(&area_b.top()))
                    .and_then(|entry| self.panes.panes_iter().find(|pane| &pane.id == entry.0))
                    .or(Some(focused));
                context.render()?;
            }
            Some(CommonAction::PaneDown) => {
                let Some((focused_area, _)) = self.pane_areas.get(&focused.id) else {
                    log::warn!(focused:?, pane_areas:? = self.pane_areas; "Tried to find focused pane area but it does not exist");
                    return Ok(());
                };
                self.focused = self
                    .pane_areas
                    .iter()
                    .filter(|(_, (area, focusable))| *focusable && focused_area.bottom() < area.top())
                    .min_by(|(_, (area_a, _)), (_, (area_b, _))| area_a.top().cmp(&area_b.top()))
                    .and_then(|entry| self.panes.panes_iter().find(|pane| &pane.id == entry.0))
                    .or(Some(focused));
                context.render()?;
            }
            Some(CommonAction::PaneRight) => {
                let Some((focused_area, _)) = self.pane_areas.get(&focused.id) else {
                    log::warn!(focused:?, pane_areas:? = self.pane_areas; "Tried to find focused pane area but it does not exist");
                    return Ok(());
                };
                self.focused = self
                    .pane_areas
                    .iter()
                    .filter(|(_, (area, focusable))| *focusable && area.left() > focused_area.right())
                    .min_by(|(_, (area_a, _)), (_, (area_b, _))| area_a.left().cmp(&area_b.left()))
                    .and_then(|entry| self.panes.panes_iter().find(|pane| &pane.id == entry.0))
                    .or(Some(focused));
                context.render()?;
            }
            Some(CommonAction::PaneLeft) => {
                let (focused_area, _) = self.pane_areas.get(&focused.id).unwrap();
                let Some((focused_area, _)) = self.pane_areas.get(&focused.id) else {
                    log::warn!(focused:?, pane_areas:? = self.pane_areas; "Tried to find focused pane area but it does not exist");
                    return Ok(());
                };
                self.focused = self
                    .pane_areas
                    .iter()
                    .filter(|(_, (area, focusable))| *focusable && area.right() < focused_area.left())
                    .max_by(|(_, (area_a, _)), (_, (area_b, _))| area_a.left().cmp(&area_b.left()))
                    .and_then(|entry| self.panes.panes_iter().find(|pane| &pane.id == entry.0))
                    .or(Some(focused));
                context.render()?;
            }
            Some(_) | None => {
                event.abandon();
                let pane = panes.get_mut(focused.pane);
                screen_call!(pane, handle_action(event, context))?;
            }
        };

        Ok(())
    }

    pub(in crate::ui) fn handle_mouse_event(
        &mut self,
        panes: &mut PaneContainer,
        event: MouseEvent,
        context: &AppContext,
    ) -> Result<()> {
        if matches!(event.kind, MouseEventKind::LeftClick) {
            let Some(pane) = self
                .pane_areas
                .iter()
                .find(|(_, (area, _))| area.contains(event.into()))
                .and_then(|(pane_id, _)| {
                    self.panes
                        .panes_iter()
                        .find(|pane| &pane.id == pane_id && pane.focusable)
                })
            else {
                return Ok(());
            };
            self.focused = Some(pane);
            context.render()?;
        }

        let Some(focused) = self.focused else {
            return Ok(());
        };

        let pane = panes.get_mut(focused.pane);
        screen_call!(pane, handle_mouse_event(event, context))
    }

    pub fn on_hide(&mut self, panes: &mut PaneContainer, context: &AppContext) -> Result<()> {
        for pane in self.panes.panes_iter() {
            let screen = panes.get_mut(pane.pane);
            screen_call!(screen, on_hide(context))?;
        }
        Ok(())
    }

    pub fn before_show(&mut self, panes: &mut PaneContainer, area: Rect, context: &AppContext) -> Result<()> {
        self.for_each_pane(panes, self.panes, area, context, &mut |pane, rect, _, _| {
            screen_call!(pane, calculate_areas(rect, context));
            screen_call!(pane, before_show(context))?;
            Ok(())
        })?;
        if !self.initialized {
            self.focused = self
                .pane_areas
                .iter()
                .filter(|(_, (_, focusable))| *focusable)
                .min_by(|(_, (a, _)), (_, (b, _))| a.left().cmp(&b.left()).then(a.top().cmp(&b.top())))
                .and_then(|entry| self.panes.panes_iter().find(|pane| &pane.id == entry.0))
                .or(self.focused);
            self.initialized = true;
        };

        Ok(())
    }

    pub fn resize(&mut self, panes: &mut PaneContainer, area: Rect, context: &AppContext) -> Result<()> {
        self.for_each_pane(panes, self.panes, area, context, &mut |pane, rect, _, _| {
            screen_call!(pane, calculate_areas(rect, context));
            screen_call!(pane, resize(rect, context))?;
            Ok(())
        })
    }
}
