use anyhow::Result;
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
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
const PLAY: &str = "\u{f04b}"; // play
const PAUSE: &str = "\u{f04c}"; // pause

const CLUSTER_W: u16 = 11; // prev(1) gap(2) [cap + " x " + cap = 5] gap(2) next(1)

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
        let active: Style = ctx.config.theme.tab_bar.active_style;
        let muted: Style = ctx.config.theme.tab_bar.inactive_style;
        let accent = active.bg.unwrap_or(Color::Cyan);
        let surround = ctx.config.theme.background_color.unwrap_or_default();
        let cap = Style::default().fg(accent).bg(surround);
        let start_x = area.x + (area.width - CLUSTER_W) / 2;
        let y = area.y + area.height / 2;
        let pill_x = start_x + 3;
        let next_x = start_x + 10;
        // Rounded pill (left cap + glyph + right cap), kept inside the box on a
        // single row. Larger than the prev/next glyphs beside it.
        let buf = frame.buffer_mut();
        buf.set_string(start_x, y, PREV, muted);
        buf.set_string(pill_x, y, "\u{e0b6}", cap);
        buf.set_string(pill_x + 1, y, format!(" {toggle_glyph} "), active);
        buf.set_string(pill_x + 4, y, "\u{e0b4}", cap);
        buf.set_string(next_x, y, NEXT, muted);
        self.prev = Some(Rect::new(start_x, y, 1, 1));
        self.toggle = Some(Rect::new(pill_x, y, 5, 1));
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
