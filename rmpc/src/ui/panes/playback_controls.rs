use anyhow::Result;
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
};
use rmpc_mpd::{commands::State, mpd_client::MpdClient};

use super::Pane;
use crate::{
    ctx::Ctx,
    shared::{
        keys::ActionEvent,
        mouse_event::{MouseEvent, MouseEventKind},
    },
};

/// A crafted, clickable transport widget: ⏮  ▶/⏸  ⏭.
///
/// The play/pause control is rendered as a filled "pill" using the theme's
/// tab-bar active style (accent background, dark glyph) so it reads as a real
/// button; previous/next sit beside it in the muted inactive style. Clicking
/// any of the three issues the corresponding MPD command.
#[derive(Debug, Default)]
pub struct PlaybackControlsPane {
    area: Rect,
    prev: Option<Rect>,
    toggle: Option<Rect>,
    next: Option<Rect>,
}

impl PlaybackControlsPane {
    pub fn new() -> Self {
        Self::default()
    }
}

// glyph constants (Nerd Font / media controls)
const PREV: &str = "\u{f048}"; // step-backward
const NEXT: &str = "\u{f051}"; // step-forward
const PLAY: &str = "\u{f144}"; // play-circle
const PAUSE: &str = "\u{f28b}"; // pause-circle
const CLUSTER_W: u16 = 7; // prev(1) gap(2) circle(1) gap(2) next(1)

impl Pane for PlaybackControlsPane {
    fn render(&mut self, frame: &mut Frame, area: Rect, ctx: &Ctx) -> Result<()> {
        self.area = area;
        self.prev = None;
        self.toggle = None;
        self.next = None;
        if area.width < CLUSTER_W || area.height == 0 {
            return Ok(());
        }
        let toggle_glyph = if ctx.status.state == State::Play { PAUSE } else { PLAY };
        let muted: Style = ctx.config.theme.tab_bar.inactive_style;
        let accent = ctx.config.theme.tab_bar.active_style.bg.unwrap_or(Color::Cyan);
        let toggle_style = Style::default().fg(accent).add_modifier(Modifier::BOLD);
        let start_x = area.x + (area.width - CLUSTER_W) / 2;
        let y = area.y + area.height / 2;
        let circle_x = start_x + 3;
        let next_x = start_x + 6;
        // Circular play/pause (a single circled glyph) in bold accent — the focal
        // control beside the muted prev/next step glyphs.
        let buf = frame.buffer_mut();
        buf.set_string(start_x, y, PREV, muted);
        buf.set_string(circle_x, y, toggle_glyph, toggle_style);
        buf.set_string(next_x, y, NEXT, muted);
        self.prev = Some(Rect::new(start_x, y, 1, 1));
        self.toggle = Some(Rect::new(circle_x, y, 1, 1));
        self.next = Some(Rect::new(next_x, y, 1, 1));
        Ok(())
    }

    fn handle_mouse_event(&mut self, event: MouseEvent, ctx: &Ctx) -> Result<()> {
        if !matches!(event.kind, MouseEventKind::LeftClick | MouseEventKind::DoubleClick) {
            return Ok(());
        }
        let pos = event.into();
        if self.prev.is_some_and(|r| r.contains(pos)) {
            ctx.command(move |_, client| {
                client.prev()?;
                Ok(())
            });
            ctx.render()?;
        } else if self.toggle.is_some_and(|r| r.contains(pos)) {
            let state = ctx.status.state;
            ctx.command(move |_, client| {
                if state == State::Stop {
                    client.play()?;
                } else {
                    client.pause_toggle()?;
                }
                Ok(())
            });
            ctx.render()?;
        } else if self.next.is_some_and(|r| r.contains(pos)) {
            ctx.command(move |_, client| {
                client.next()?;
                Ok(())
            });
            ctx.render()?;
        }
        Ok(())
    }

    fn handle_action(&mut self, _event: &mut ActionEvent, _ctx: &mut Ctx) -> Result<()> {
        Ok(())
    }
}
