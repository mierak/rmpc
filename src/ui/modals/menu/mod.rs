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
        key_event::KeyEvent,
        macros::status_error,
        mpd_client_ext::{Enqueue, MpdClientExt as _},
    },
    ui::modals::menu::select_section::SelectSection,
};

mod input_section;
mod list_section;
pub mod modal;
mod multi_action_section;
mod select_section;

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

    fn confirm(&mut self, ctx: &Ctx) -> Result<bool>;
    fn key_input(&mut self, _key: &mut KeyEvent, _ctx: &Ctx) -> Result<()> {
        Ok(())
    }

    fn len(&self) -> usize;
    fn render(&mut self, area: Rect, buf: &mut Buffer);

    fn left_click(&mut self, pos: ratatui::layout::Position);
    fn double_click(&mut self, pos: ratatui::layout::Position, ctx: &Ctx) -> Result<bool>;
}

#[derive(Debug)]
enum SectionType<'a> {
    Menu(ListSection),
    Select(SelectSection),
    Multi(MultiActionSection<'a>),
    Input(InputSection<'a>),
}

impl Section for SectionType<'_> {
    fn down(&mut self) -> bool {
        match self {
            SectionType::Menu(s) => s.down(),
            SectionType::Multi(s) => s.down(),
            SectionType::Input(s) => s.down(),
            SectionType::Select(s) => s.down(),
        }
    }

    fn up(&mut self) -> bool {
        match self {
            SectionType::Menu(s) => s.up(),
            SectionType::Multi(s) => s.up(),
            SectionType::Input(s) => s.up(),
            SectionType::Select(s) => s.up(),
        }
    }

    fn right(&mut self) -> bool {
        match self {
            SectionType::Menu(s) => s.right(),
            SectionType::Multi(s) => s.right(),
            SectionType::Input(s) => s.right(),
            SectionType::Select(s) => s.right(),
        }
    }

    fn left(&mut self) -> bool {
        match self {
            SectionType::Menu(s) => s.left(),
            SectionType::Multi(s) => s.left(),
            SectionType::Input(s) => s.left(),
            SectionType::Select(s) => s.left(),
        }
    }

    fn unselect(&mut self) {
        match self {
            SectionType::Menu(s) => s.unselect(),
            SectionType::Multi(s) => s.unselect(),
            SectionType::Input(s) => s.unselect(),
            SectionType::Select(s) => s.unselect(),
        }
    }

    fn unfocus(&mut self) {
        match self {
            SectionType::Menu(s) => s.unfocus(),
            SectionType::Multi(s) => s.unfocus(),
            SectionType::Input(s) => s.unfocus(),
            SectionType::Select(s) => s.unfocus(),
        }
    }

    fn confirm(&mut self, ctx: &Ctx) -> Result<bool> {
        match self {
            SectionType::Menu(s) => s.confirm(ctx),
            SectionType::Multi(s) => s.confirm(ctx),
            SectionType::Input(s) => s.confirm(ctx),
            SectionType::Select(s) => s.confirm(ctx),
        }
    }

    fn len(&self) -> usize {
        match self {
            SectionType::Menu(s) => s.len(),
            SectionType::Multi(s) => s.len(),
            SectionType::Input(s) => s.len(),
            SectionType::Select(s) => s.len(),
        }
    }

    fn render(&mut self, area: Rect, buf: &mut ratatui::buffer::Buffer) {
        match self {
            SectionType::Menu(s) => Widget::render(s, area, buf),
            SectionType::Multi(s) => Widget::render(s, area, buf),
            SectionType::Input(s) => Widget::render(s, area, buf),
            SectionType::Select(s) => Widget::render(s, area, buf),
        }
    }

    fn key_input(&mut self, key: &mut KeyEvent, ctx: &Ctx) -> Result<()> {
        match self {
            SectionType::Menu(s) => s.key_input(key, ctx),
            SectionType::Multi(s) => s.key_input(key, ctx),
            SectionType::Input(s) => s.key_input(key, ctx),
            SectionType::Select(s) => s.key_input(key, ctx),
        }
    }

    fn left_click(&mut self, pos: Position) {
        match self {
            SectionType::Menu(s) => s.left_click(pos),
            SectionType::Multi(s) => s.left_click(pos),
            SectionType::Input(s) => s.left_click(pos),
            SectionType::Select(s) => s.left_click(pos),
        }
    }

    fn double_click(&mut self, pos: Position, ctx: &Ctx) -> Result<bool> {
        match self {
            SectionType::Menu(s) => s.double_click(pos, ctx),
            SectionType::Multi(s) => s.double_click(pos, ctx),
            SectionType::Input(s) => s.double_click(pos, ctx),
            SectionType::Select(s) => s.double_click(pos, ctx),
        }
    }
}

pub fn create_rating_modal<'a>(
    items: Vec<Enqueue>,
    values: &[i32],
    custom: bool,
    ctx: &Ctx,
) -> MenuModal<'a> {
    let clone = items.clone();
    let clone2 = items.clone();

    MenuModal::new(ctx)
        .input_section(ctx, "Rating", move |section| {
            if !custom {
                return None;
            }

            let section = section.action(move |ctx, value| {
                let Ok(_) = value.trim().parse::<i32>() else {
                    status_error!("Rating must be a valid number");
                    return;
                };

                if !value.trim().is_empty() {
                    ctx.command(move |client| {
                        client.set_sticker_multiple("rating", value, clone2)?;
                        Ok(())
                    });
                }
            });

            Some(section)
        })
        .select_section(ctx, move |mut section| {
            if values.is_empty() {
                return None;
            }

            for i in values {
                section.add_item(i.to_string(), i.to_string());
            }

            section.action(move |ctx, value| {
                ctx.command(move |client| {
                    client.set_sticker_multiple("rating", value, clone)?;
                    Ok(())
                });
                Ok(())
            });

            Some(section)
        })
        .list_section(ctx, |section| {
            let section = section.item("Clear rating", |ctx| {
                ctx.command(move |client| {
                    client.delete_sticker_multiple("rating", items)?;
                    Ok(())
                });
                Ok(())
            });
            let section = section.item("Cancel", |_ctx| Ok(()));
            Some(section)
        })
        .build()
}

pub fn create_add_modal<'a>(
    opts: Vec<(String, AddOpts, (Vec<Enqueue>, Option<usize>))>,
    ctx: &Ctx,
) -> MenuModal<'a> {
    MenuModal::new(ctx)
        .list_section(ctx, |section| {
            let queue_len = ctx.queue.len();
            let current_song_idx = ctx.find_current_song_in_queue().map(|(i, _)| i);
            let mut section = section;

            for (label, options, (enqueue, hovered_idx)) in opts {
                section = section.item(label, move |ctx| {
                    if !enqueue.is_empty() {
                        ctx.command(move |client| {
                            let autoplay =
                                options.autoplay(queue_len, current_song_idx, hovered_idx);
                            client.enqueue_multiple(enqueue, options.position, autoplay)?;

                            Ok(())
                        });
                    }
                    Ok(())
                });
            }
            Some(section)
        })
        .list_section(ctx, |section| Some(section.item("Cancel", |_ctx| Ok(()))))
        .build()
}
