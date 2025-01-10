use std::{collections::HashMap, time::Instant};

use anyhow::Result;
use itertools::Itertools;
use ratatui::{
    layout::{Constraint, Layout, Rect},
    widgets::Block,
    Frame,
};

use crate::{
    config::{
        keys::CommonAction,
        tabs::{Pane, SizedPaneOrSplit},
    },
    context::AppContext,
    shared::{
        ext::{rect::RectExt, vec::VecExt},
        id::Id,
        key_event::KeyEvent,
        mouse_event::{MouseEvent, MouseEventKind},
    },
};

use super::{Pane as _, PaneContainer, Panes};

#[derive(Debug)]
pub struct PaneData {
    area: Rect,
    block_area: Rect,
    focusable: bool,
    active: Instant,
}

impl PaneData {
    fn new(focusable: bool) -> Self {
        Self {
            focusable,
            active: Instant::now(),
            area: Rect::default(),
            block_area: Rect::default(),
        }
    }
}

#[derive(Debug)]
pub struct TabScreen {
    focused: Option<Pane>, // can focused ever be none?
    pub panes: SizedPaneOrSplit,
    pane_data: HashMap<Id, PaneData>,
    initialized: bool,
}

impl TabScreen {
    pub fn new(panes: SizedPaneOrSplit) -> Self {
        let focused = panes.panes_iter().next();
        Self {
            panes,
            focused,
            initialized: false,
            pane_data: HashMap::default(),
        }
    }

    fn set_focused(&mut self, pane: Option<Pane>) {
        self.focused = pane.or(self.focused);
        if let Some(data) = pane.and_then(|pane| self.pane_data.get_mut(&pane.id)) {
            data.active = Instant::now();
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
        self.for_each_pane(panes, area, context, &mut |pane, area, block, block_area| {
            screen_call!(pane, render(frame, area, context))?;
            frame.render_widget(block, block_area);
            Ok(())
        })?;
        Ok(())
    }

    pub fn for_each_pane(
        &mut self,
        panes: &mut PaneContainer,
        area: Rect,
        context: &AppContext,
        callback: &mut impl FnMut(&mut Panes<'_>, Rect, Block, Rect) -> Result<()>,
    ) -> Result<()> {
        let mut stack = vec![(&self.panes, area)];

        while let Some((configured_panes, area)) = stack.pop() {
            match configured_panes {
                SizedPaneOrSplit::Pane(pane) => {
                    let block = Block::default()
                        .border_style(if self.focused.is_some_and(|p| p.id == pane.id) {
                            context.config.as_focused_border_style()
                        } else {
                            context.config.as_border_style()
                        })
                        .borders(pane.border);
                    let pane_area = block.inner(area);

                    let pane_data = self
                        .pane_data
                        .entry(pane.id)
                        .or_insert_with(|| PaneData::new(pane.focusable));
                    pane_data.area = pane_area;
                    pane_data.block_area = area;

                    let mut pane = panes.get_mut(pane.pane);
                    callback(&mut pane, pane_area, block, area)?;
                }
                SizedPaneOrSplit::Split { direction, panes } => {
                    let constraints = panes.iter().map(|pane| Into::<Constraint>::into(pane.size));
                    let areas = Layout::new(*direction, constraints).split(area);
                    stack.extend(areas.iter().enumerate().map(|(idx, area)| (&panes[idx].pane, *area)));
                }
            }
        }

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

        let Some(focused_pane_data) = self.pane_data.get(&focused.id) else {
            log::warn!(focused:?, pane_areas:? = self.pane_data; "Tried to find focused pane area but it does not exist");
            return Ok(());
        };
        let focused_area = focused_pane_data.area;

        match event.as_common_action(context) {
            Some(CommonAction::PaneUp) => {
                let pane_to_focus = self
                    .panes_directly_above(focused_area)
                    .collect_vec()
                    .or_if_empty(|| self.all_panes_above(focused_area))
                    .into_iter()
                    .max_by_key(|(_, data)| data.active)
                    .and_then(|(id, _)| self.panes.panes_iter().find(|pane| pane.id == *id));

                self.set_focused(pane_to_focus);
                context.render()?;
            }
            Some(CommonAction::PaneDown) => {
                let pane_to_focus = self
                    .panes_directly_below(focused_area)
                    .collect_vec()
                    .or_if_empty(|| self.all_panes_below(focused_area))
                    .into_iter()
                    .max_by_key(|(_, data)| data.active)
                    .and_then(|(id, _)| self.panes.panes_iter().find(|pane| pane.id == *id));

                self.set_focused(pane_to_focus);
                context.render()?;
            }
            Some(CommonAction::PaneRight) => {
                let pane_to_focus = self
                    .panes_directly_right(focused_area)
                    .collect_vec()
                    .or_if_empty(|| self.all_panes_right(focused_area))
                    .into_iter()
                    .max_by_key(|(_, data)| data.active)
                    .and_then(|(id, _)| self.panes.panes_iter().find(|pane| pane.id == *id));

                self.set_focused(pane_to_focus);
                context.render()?;
            }
            Some(CommonAction::PaneLeft) => {
                let pane_to_focus = self
                    .panes_directly_left(focused_area)
                    .collect_vec()
                    .or_if_empty(|| self.all_panes_left(focused_area))
                    .into_iter()
                    .max_by_key(|(_, data)| data.active)
                    .and_then(|(id, _)| self.panes.panes_iter().find(|pane| pane.id == *id));

                self.set_focused(pane_to_focus);
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
                .pane_data
                .iter()
                .find(|(_, PaneData { area, .. })| area.contains(event.into()))
                .and_then(|(pane_id, _)| {
                    self.panes
                        .panes_iter()
                        .find(|pane| &pane.id == pane_id && pane.focusable)
                })
            else {
                return Ok(());
            };
            self.set_focused(Some(pane));
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
        self.for_each_pane(panes, area, context, &mut |pane, rect, _, _| {
            screen_call!(pane, calculate_areas(rect, context));
            screen_call!(pane, before_show(context))?;
            Ok(())
        })?;
        if !self.initialized {
            self.set_focused(
                self.pane_data
                    .iter()
                    .filter(|(_, PaneData { focusable, .. })| *focusable)
                    .min_by(|(_, PaneData { area: a, .. }), (_, PaneData { area: b, .. })| {
                        a.left().cmp(&b.left()).then(a.top().cmp(&b.top()))
                    })
                    .and_then(|entry| self.panes.panes_iter().find(|pane| &pane.id == entry.0)),
            );
            self.initialized = true;
        };

        Ok(())
    }

    pub fn resize(&mut self, panes: &mut PaneContainer, area: Rect, context: &AppContext) -> Result<()> {
        self.for_each_pane(panes, area, context, &mut |pane, rect, _, _| {
            screen_call!(pane, calculate_areas(rect, context));
            screen_call!(pane, resize(rect, context))?;
            Ok(())
        })
    }

    fn panes_directly_above(&self, focused_area: Rect) -> impl Iterator<Item = (&Id, &PaneData)> {
        self.pane_data.iter().filter(move |data| {
            data.1.focusable
                && focused_area.top() == data.1.block_area.bottom()
                && data.1.block_area.overlaps_in_x(&focused_area)
        })
    }

    fn all_panes_above(&self, focused_area: Rect) -> Vec<(&Id, &PaneData)> {
        self.pane_data
            .iter()
            .filter(|data| {
                data.1.focusable
                    && focused_area.top() > data.1.block_area.bottom()
                    && data.1.block_area.overlaps_in_x(&focused_area)
            })
            .max_set_by(|a, b| a.1.area.bottom().cmp(&b.1.area.bottom()))
    }

    fn panes_directly_below(&self, focused_area: Rect) -> impl Iterator<Item = (&Id, &PaneData)> {
        self.pane_data.iter().filter(move |data| {
            data.1.focusable
                && focused_area.bottom() == data.1.block_area.top()
                && data.1.block_area.overlaps_in_x(&focused_area)
        })
    }

    fn all_panes_below(&self, focused_area: Rect) -> Vec<(&Id, &PaneData)> {
        self.pane_data
            .iter()
            .filter(|data| {
                data.1.focusable
                    && focused_area.bottom() < data.1.block_area.top()
                    && data.1.block_area.overlaps_in_x(&focused_area)
            })
            .min_set_by(|a, b| a.1.area.top().cmp(&b.1.area.top()))
    }

    fn panes_directly_left(&self, focused_area: Rect) -> impl Iterator<Item = (&Id, &PaneData)> {
        self.pane_data.iter().filter(move |data| {
            data.1.focusable
                && focused_area.left() == data.1.block_area.right()
                && data.1.block_area.overlaps_in_y(&focused_area)
        })
    }

    fn all_panes_left(&self, focused_area: Rect) -> Vec<(&Id, &PaneData)> {
        self.pane_data
            .iter()
            .filter(|data| {
                data.1.focusable
                    && focused_area.left() > data.1.block_area.right()
                    && data.1.block_area.overlaps_in_y(&focused_area)
            })
            .max_set_by(|a, b| a.1.area.left().cmp(&b.1.area.left()))
    }

    fn panes_directly_right(&self, focused_area: Rect) -> impl Iterator<Item = (&Id, &PaneData)> {
        self.pane_data.iter().filter(move |data| {
            data.1.focusable
                && focused_area.right() == data.1.block_area.left()
                && data.1.block_area.overlaps_in_y(&focused_area)
        })
    }

    fn all_panes_right(&self, focused_area: Rect) -> Vec<(&Id, &PaneData)> {
        self.pane_data
            .iter()
            .filter(|data| {
                data.1.focusable
                    && focused_area.right() < data.1.block_area.left()
                    && data.1.block_area.overlaps_in_y(&focused_area)
            })
            .min_set_by(|a, b| a.1.area.left().cmp(&b.1.area.left()))
    }
}
