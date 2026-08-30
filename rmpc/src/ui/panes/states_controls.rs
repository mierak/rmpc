use anyhow::Result;
use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
};
use rmpc_mpd::{commands::status::OnOffOneshot, mpd_client::MpdClient};

use super::Pane;
use crate::{
    ctx::Ctx,
    shared::{
        keys::ActionEvent,
        mouse_event::{MouseEvent, MouseEventKind},
    },
};

// Nerd Font glyphs (match the theme's previous static toggles)
const REPEAT: &str = "\u{f01e}";
const RANDOM: &str = "\u{f074}";
const CONSUME: &str = "\u{f014}";
const SINGLE: &str = "\u{f0e2}";

const GAP: u16 = 2;
const RIGHT_PAD: u16 = 2; // align toggles' right edge with the volume bar/% line above
const CLUSTER_W: u16 = 4 + 3 * GAP; // 4 glyphs + 3 gaps

/// Clickable repeat / random / consume / single toggles for the header.
///
/// Replaces the static `Property` toggles so each glyph is a real button:
/// left-clicking flips the corresponding MPD mode (repeat/random toggle,
/// consume/single cycle on → off → oneshot). Active modes use the theme's
/// highlight style; inactive ones are dimmed.
#[derive(Debug, Default)]
pub struct StatesControlsPane {
    area: Rect,
    rects: [Option<Rect>; 4],
}

impl StatesControlsPane {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Pane for StatesControlsPane {
    fn render(&mut self, frame: &mut Frame, area: Rect, ctx: &Ctx) -> Result<()> {
        self.area = area;
        self.rects = [None; 4];
        if area.width < CLUSTER_W + RIGHT_PAD || area.height == 0 {
            return Ok(());
        }
        let st = &ctx.status;
        let on = ctx.config.theme.highlighted_item_style;
        let off = Style::default()
            .fg(ctx.config.theme.text_color.unwrap_or_default())
            .add_modifier(Modifier::DIM);
        let glyphs = [
            (REPEAT, st.repeat),
            (RANDOM, st.random),
            (CONSUME, !matches!(st.consume, OnOffOneshot::Off)),
            (SINGLE, !matches!(st.single, OnOffOneshot::Off)),
        ];

        // right-align the cluster within the area, leaving a small right margin
        let start_x = area.x + area.width.saturating_sub(CLUSTER_W + RIGHT_PAD);
        let y = area.y + area.height / 2;
        let buf = frame.buffer_mut();
        for (i, (glyph, active)) in glyphs.iter().enumerate() {
            let x = start_x + (i as u16) * (1 + GAP);
            if x >= area.x.saturating_add(area.width) {
                break;
            }
            let style = if *active { on } else { off };
            buf.set_string(x, y, *glyph, style);
            self.rects[i] = Some(Rect::new(x, y, 1, 1));
        }
        Ok(())
    }

    fn handle_mouse_event(&mut self, event: MouseEvent, ctx: &Ctx) -> Result<()> {
        if !matches!(event.kind, MouseEventKind::LeftClick) {
            return Ok(());
        }
        let pos = event.into();
        let hit = self.rects.iter().position(|r| r.is_some_and(|r| r.contains(pos)));
        let Some(idx) = hit else {
            return Ok(());
        };
        let st = &ctx.status;
        match idx {
            0 => {
                let v = !st.repeat;
                ctx.command(move |_, c| {
                    c.repeat(v)?;
                    Ok(())
                });
            }
            1 => {
                let v = !st.random;
                ctx.command(move |_, c| {
                    c.random(v)?;
                    Ok(())
                });
            }
            2 => {
                let v = st.consume.cycle();
                ctx.command(move |_, c| {
                    c.consume(v)?;
                    Ok(())
                });
            }
            _ => {
                let v = st.single.cycle();
                ctx.command(move |_, c| {
                    c.single(v)?;
                    Ok(())
                });
            }
        }
        ctx.render()?;
        Ok(())
    }

    fn handle_action(&mut self, _event: &mut ActionEvent, _ctx: &mut Ctx) -> Result<()> {
        Ok(())
    }
}
