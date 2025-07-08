use anyhow::Result;
use input_section::InputSection;
use list_section::ListSection;
use modal::MenuModal;
use multi_action_section::MultiActionSection;
use ratatui::{
    buffer::Buffer,
    layout::{Position, Rect},
    widgets::Widget,
};

use crate::{
    config::keys::actions::AddOpts,
    ctx::Ctx,
    shared::{
        ext::mpd_client::{Enqueue, MpdClientExt as _},
        key_event::KeyEvent,
    },
};

mod input_section;
mod list_section;
pub mod modal;
mod multi_action_section;

trait Section {
    fn down(&mut self) -> bool;
    fn up(&mut self) -> bool;
    fn right(&mut self) -> bool {
        true
    }
    fn left(&mut self) -> bool {
        true
    }
    fn unselect(&mut self);
    fn unfocus(&mut self) {}

    fn confirm(&mut self, ctx: &Ctx) -> bool;
    fn key_input(&mut self, _key: &mut KeyEvent, _ctx: &Ctx) -> Result<()> {
        Ok(())
    }

    fn len(&self) -> usize;
    fn render(&mut self, area: Rect, buf: &mut Buffer);

    fn left_click(&mut self, pos: ratatui::layout::Position);
    fn double_click(&mut self, pos: ratatui::layout::Position, ctx: &Ctx) -> bool;
}

#[derive(Debug)]
enum SectionType<'a> {
    Menu(ListSection),
    Multi(MultiActionSection<'a>),
    Input(InputSection<'a>),
}

impl Section for SectionType<'_> {
    fn down(&mut self) -> bool {
        match self {
            SectionType::Menu(s) => s.down(),
            SectionType::Multi(s) => s.down(),
            SectionType::Input(s) => s.down(),
        }
    }

    fn up(&mut self) -> bool {
        match self {
            SectionType::Menu(s) => s.up(),
            SectionType::Multi(s) => s.up(),
            SectionType::Input(s) => s.up(),
        }
    }

    fn right(&mut self) -> bool {
        match self {
            SectionType::Menu(s) => s.right(),
            SectionType::Multi(s) => s.right(),
            SectionType::Input(s) => s.right(),
        }
    }

    fn left(&mut self) -> bool {
        match self {
            SectionType::Menu(s) => s.left(),
            SectionType::Multi(s) => s.left(),
            SectionType::Input(s) => s.left(),
        }
    }

    fn unselect(&mut self) {
        match self {
            SectionType::Menu(s) => s.unselect(),
            SectionType::Multi(s) => s.unselect(),
            SectionType::Input(s) => s.unselect(),
        }
    }

    fn unfocus(&mut self) {
        match self {
            SectionType::Menu(s) => s.unfocus(),
            SectionType::Multi(s) => s.unfocus(),
            SectionType::Input(s) => s.unfocus(),
        }
    }

    fn confirm(&mut self, ctx: &Ctx) -> bool {
        match self {
            SectionType::Menu(s) => s.confirm(ctx),
            SectionType::Multi(s) => s.confirm(ctx),
            SectionType::Input(s) => s.confirm(ctx),
        }
    }

    fn len(&self) -> usize {
        match self {
            SectionType::Menu(s) => s.len(),
            SectionType::Multi(s) => s.len(),
            SectionType::Input(s) => s.len(),
        }
    }

    fn render(&mut self, area: Rect, buf: &mut ratatui::buffer::Buffer) {
        match self {
            SectionType::Menu(s) => Widget::render(s, area, buf),
            SectionType::Multi(s) => Widget::render(s, area, buf),
            SectionType::Input(s) => Widget::render(s, area, buf),
        }
    }

    fn key_input(&mut self, key: &mut KeyEvent, ctx: &Ctx) -> Result<()> {
        match self {
            SectionType::Menu(s) => s.key_input(key, ctx),
            SectionType::Multi(s) => s.key_input(key, ctx),
            SectionType::Input(s) => s.key_input(key, ctx),
        }
    }

    fn left_click(&mut self, pos: Position) {
        match self {
            SectionType::Menu(s) => s.left_click(pos),
            SectionType::Multi(s) => s.left_click(pos),
            SectionType::Input(s) => s.left_click(pos),
        }
    }

    fn double_click(&mut self, pos: Position, ctx: &Ctx) -> bool {
        match self {
            SectionType::Menu(s) => s.double_click(pos, ctx),
            SectionType::Multi(s) => s.double_click(pos, ctx),
            SectionType::Input(s) => s.double_click(pos, ctx),
        }
    }
}

pub fn create_add_modal<'a>(
    opts: Vec<(String, AddOpts, (Vec<Enqueue>, Option<usize>))>,
    ctx: &Ctx,
) -> MenuModal<'a> {
    MenuModal::new(ctx)
        .add_list_section(ctx, |section| {
            let queue_len = ctx.queue.len();
            let current_song_idx = ctx.find_current_song_in_queue().map(|(i, _)| i);
            let mut section = section;

            for (label, options, (enqueue, hovered_idx)) in opts {
                section = section.add_item(label, move |ctx| {
                    if !enqueue.is_empty() {
                        ctx.command(move |client| {
                            let autoplay =
                                options.autoplay(queue_len, current_song_idx, hovered_idx);
                            client.enqueue_multiple(enqueue, options.position, autoplay)?;

                            Ok(())
                        });
                    }
                });
            }
            Some(section)
        })
        .add_list_section(ctx, |section| Some(section.add_item("Cancel", |_ctx| {})))
        .build()
}
