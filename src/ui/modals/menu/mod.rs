use std::{borrow::Cow, collections::HashSet};

use anyhow::Result;
use input_section::InputSection;
use itertools::Itertools;
use list_section::ListSection;
use modal::MenuModal;
use multi_action_section::MultiActionSection;
use ratatui::{
    buffer::Buffer,
    layout::{Position, Rect},
};

use crate::{
    config::keys::actions::{AddOpts, DuplicateStrategy},
    ctx::{Ctx, LIKE_STICKER, RATING_STICKER},
    mpd::{
        client::Client,
        errors::{ErrorCode, MpdError, MpdFailureResponse},
        mpd_client::{MpdClient, MpdCommand, SingleOrRange},
        proto_client::ProtoClient,
    },
    shared::{
        cmp::StringCompare,
        key_event::KeyEvent,
        macros::{modal, status_error, status_info, status_warn},
        mpd_client_ext::{Enqueue, MpdClientExt as _},
    },
    ui::modals::{
        confirm_modal::{Action, ConfirmModal},
        menu::select_section::SelectSection,
    },
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
    fn selected(&self) -> Option<usize>;
    fn select(&mut self, idx: usize);
    fn unselect(&mut self);
    fn unfocus(&mut self) {}

    fn confirm(&mut self, ctx: &Ctx) -> Result<bool>;
    fn key_input(&mut self, _key: &mut KeyEvent, _ctx: &Ctx) -> Result<()> {
        Ok(())
    }

    fn len(&self) -> usize;
    fn preferred_height(&self) -> u16;
    fn render(&mut self, area: Rect, buf: &mut Buffer, filter: Option<&str>, ctx: &Ctx);

    fn left_click(&mut self, pos: ratatui::layout::Position);
    fn double_click(&mut self, pos: ratatui::layout::Position, ctx: &Ctx) -> Result<bool>;

    fn item_labels_iter(&self) -> Box<dyn Iterator<Item = &str> + '_>;
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

    fn selected(&self) -> Option<usize> {
        match self {
            SectionType::Menu(s) => s.selected(),
            SectionType::Multi(s) => s.selected(),
            SectionType::Input(s) => s.selected(),
            SectionType::Select(s) => s.selected(),
        }
    }

    fn select(&mut self, idx: usize) {
        match self {
            SectionType::Menu(s) => s.select(idx),
            SectionType::Multi(s) => s.select(idx),
            SectionType::Input(s) => s.select(idx),
            SectionType::Select(s) => s.select(idx),
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

    fn preferred_height(&self) -> u16 {
        match self {
            SectionType::Menu(s) => s.preferred_height(),
            SectionType::Multi(s) => s.preferred_height(),
            SectionType::Input(s) => s.preferred_height(),
            SectionType::Select(s) => s.preferred_height(),
        }
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, filter: Option<&str>, ctx: &Ctx) {
        match self {
            SectionType::Menu(s) => s.render(area, buf, filter, ctx),
            SectionType::Multi(s) => s.render(area, buf, filter, ctx),
            SectionType::Input(s) => s.render(area, buf, filter, ctx),
            SectionType::Select(s) => s.render(area, buf, filter, ctx),
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

    fn item_labels_iter(&self) -> Box<dyn Iterator<Item = &str> + '_> {
        match self {
            SectionType::Menu(s) => s.item_labels_iter(),
            SectionType::Multi(s) => s.item_labels_iter(),
            SectionType::Input(s) => s.item_labels_iter(),
            SectionType::Select(s) => s.item_labels_iter(),
        }
    }
}

pub fn create_rating_modal<'a>(
    items: Vec<Enqueue>,
    values: &[i32],
    min_rating: i32,
    max_rating: i32,
    custom: bool,
    like: bool,
    ctx: &Ctx,
) -> MenuModal<'a> {
    let clone = items.clone();
    let clone2 = items.clone();
    let clone3 = items.clone();

    MenuModal::new(ctx)
        .input_section(ctx, "Rating", move |section| {
            if !custom {
                return None;
            }

            let section = section.action(move |ctx, value| {
                let Ok(v) = value.trim().parse::<i32>() else {
                    status_error!("Rating must be a valid number");
                    return;
                };

                if v < min_rating {
                    status_error!("Rating must be at least {min_rating}");
                    return;
                }

                if v > max_rating {
                    status_error!("Rating must be at most {max_rating}");
                    return;
                }

                if !value.trim().is_empty() {
                    ctx.command(move |client| {
                        client.set_sticker_multiple(RATING_STICKER, value, clone2)?;
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
                    client.set_sticker_multiple(RATING_STICKER, value, clone)?;
                    Ok(())
                });
                Ok(())
            });

            Some(section)
        })
        .list_section(ctx, |section| {
            if !like {
                return None;
            }
            let clone = items.clone();
            let section = section.item("Like", |ctx| {
                ctx.command(move |client| {
                    client.set_sticker_multiple(LIKE_STICKER, "2".to_string(), clone)?;
                    Ok(())
                });
                Ok(())
            });
            let clone = items.clone();
            let section = section.item("Neutral", |ctx| {
                ctx.command(move |client| {
                    client.set_sticker_multiple(LIKE_STICKER, "1".to_string(), clone)?;
                    Ok(())
                });
                Ok(())
            });
            let clone = items.clone();
            let section = section.item("Dislike", |ctx| {
                ctx.command(move |client| {
                    client.set_sticker_multiple(LIKE_STICKER, "0".to_string(), clone)?;
                    Ok(())
                });
                Ok(())
            });
            Some(section)
        })
        .list_section(ctx, |mut section| {
            if custom || !values.is_empty() {
                section.add_item("Clear rating", |ctx| {
                    ctx.command(move |client| {
                        client.delete_sticker_multiple(RATING_STICKER, clone3)?;
                        Ok(())
                    });
                    Ok(())
                });
            }
            if like {
                section.add_item("Clear like state", |ctx| {
                    ctx.command(move |client| {
                        client.delete_sticker_multiple(LIKE_STICKER, items)?;
                        Ok(())
                    });
                    Ok(())
                });
            }

            section.add_item("Cancel", |_ctx| Ok(()));

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
            let current_song_idx = ctx.find_current_song_in_queue().map(|(i, _)| i);
            let mut section = section;

            for (label, options, (enqueue, hovered_idx)) in opts {
                section = section.item(label, move |ctx| {
                    if !enqueue.is_empty() {
                        Client::resolve_and_enqueue(
                            ctx,
                            enqueue,
                            options.position,
                            options.autoplay,
                            current_song_idx,
                            hovered_idx,
                        );
                    }
                    Ok(())
                });
            }
            Some(section)
        })
        .list_section(ctx, |section| Some(section.item("Cancel", |_ctx| Ok(()))))
        .build()
}

pub fn create_save_modal<'a>(
    song_paths: Vec<String>,
    initial_playlist_name: Option<String>,
    duplicate_strategy: DuplicateStrategy,
    ctx: &Ctx,
) -> Result<MenuModal<'a>> {
    let playlists =
        ctx.query_sync(|client| Ok(client.list_playlists()?))?.into_iter().sorted_by(|a, b| {
            StringCompare::builder().fold_case(true).build().compare(&a.name, &b.name)
        });

    Ok(MenuModal::new(ctx)
        .width(80)
        .input_section(ctx, "New playlist", |mut sect| {
            sect.add_initial_value(initial_playlist_name.unwrap_or_default());
            let song_paths = song_paths.clone();
            sect.add_action(|ctx, value| {
                if !value.is_empty() {
                    ctx.command(move |client| {
                        client.create_playlist(&value, song_paths)?;
                        Ok(())
                    });
                }
            });
            Some(sect)
        })
        .select_section(ctx, move |mut sect| {
            sect.action(move |ctx, playlist_name| {
                add_to_playlist_or_show_modal(playlist_name, song_paths, duplicate_strategy, ctx);
                Ok(())
            });
            for mut playlist in playlists {
                let playlist_name = std::mem::take(&mut playlist.name);
                sect.add_item(playlist_name.clone(), playlist_name);
                sect.add_max_height(12);
            }
            Some(sect)
        })
        .list_section(ctx, |mut section| {
            section.add_item("Cancel", |_ctx| Ok(()));
            Some(section)
        })
        .build())
}

pub fn add_to_playlist_or_show_modal(
    playlist_name: String,
    all_songs: Vec<String>,
    duplicate_strategy: DuplicateStrategy,
    ctx: &Ctx,
) {
    let pl_name = playlist_name.clone();
    let songs_in_playlist = match ctx.query_sync(move |client| {
        let pl: HashSet<_> =
            client.list_playlist_info(&pl_name, None)?.into_iter().map(|s| s.file).collect();
        Ok(pl)
    }) {
        Ok(v) => v,
        Err(err) => {
            status_error!("Failed to fetch playlist info: {err}");
            return;
        }
    };

    let (duplicate_songs, non_duplicate_songs): (Vec<_>, Vec<_>) =
        all_songs.iter().cloned().partition(|s| songs_in_playlist.contains(s));

    match duplicate_strategy {
        DuplicateStrategy::None if !duplicate_songs.is_empty() => {}
        DuplicateStrategy::NonDuplicate if !duplicate_songs.is_empty() => {
            // add only non duplicate songs
            ctx.command(move |client| {
                client.add_to_playlist_multiple(&playlist_name, non_duplicate_songs)?;
                Ok(())
            });
        }
        DuplicateStrategy::Ask if !duplicate_songs.is_empty() => {
            // show modal window
            let modal = create_duplicate_songs_modal(
                playlist_name,
                all_songs,
                &duplicate_songs,
                non_duplicate_songs,
                ctx,
            );
            modal!(ctx, modal);
        }
        DuplicateStrategy::All
        | DuplicateStrategy::None
        | DuplicateStrategy::NonDuplicate
        | DuplicateStrategy::Ask => {
            // add all songs
            ctx.command(move |client| {
                client.add_to_playlist_multiple(&playlist_name, all_songs)?;
                Ok(())
            });
        }
    }
}

fn create_duplicate_songs_modal<'a>(
    playlist_name: String,
    all_songs: Vec<String>,
    duplicate_songs: &[String],
    non_duplicate_songs: Vec<String>,
    ctx: &Ctx,
) -> ConfirmModal<'a> {
    let max = 5;
    let mut message: Vec<Cow<_>> = vec![
        format!("You are trying to add songs that are already in the playlist '{playlist_name}':")
            .into(),
        "\n".into(),
    ];

    for d in duplicate_songs.iter().take(max) {
        message.push(format!("  - {d}").into());
    }

    if duplicate_songs.len() > max {
        let count = duplicate_songs.len() - max;
        if count == 1 {
            message.push("  ... and 1 other".into());
        } else {
            message.push(format!("  ... and {count} others").into());
        }
    }

    let playlist_name2 = playlist_name.clone();
    ConfirmModal::builder()
        .ctx(ctx)
        .message(message)
        .action(Action::CustomButtons {
            buttons: vec![
                (
                    "Add anyway",
                    Box::new(|ctx| {
                        ctx.command(move |client| {
                            client.add_to_playlist_multiple(&playlist_name2, all_songs)?;
                            Ok(())
                        });
                        Ok(())
                    }),
                ),
                (
                    "Add non duplicates",
                    Box::new(|ctx| {
                        ctx.command(move |client| {
                            client.add_to_playlist_multiple(&playlist_name, non_duplicate_songs)?;
                            Ok(())
                        });
                        Ok(())
                    }),
                ),
                ("Cancel", Box::new(|_ctx| Ok(()))),
            ],
        })
        .build()
}

pub fn create_delete_modal<'a>(
    song_paths: HashSet<String>,
    confirmation: bool,
    ctx: &Ctx,
) -> Result<MenuModal<'a>> {
    let playlists =
        ctx.query_sync(|client| Ok(client.list_playlists()?))?.into_iter().sorted_by(|a, b| {
            StringCompare::builder().fold_case(true).build().compare(&a.name, &b.name)
        });

    Ok(MenuModal::new(ctx)
        .select_section(ctx, move |mut sect| {
            for playlist in playlists {
                sect.add_item(playlist.name.clone(), playlist.name);
            }
            sect.add_max_height(12);
            sect.action(move |ctx, playlist| {
                delete_from_playlist_or_show_confirmation(
                    playlist,
                    &song_paths,
                    confirmation,
                    ctx,
                )?;
                Ok(())
            });
            Some(sect)
        })
        .list_section(ctx, |mut sect| {
            sect.add_item("Cancel", |_| Ok(()));
            Some(sect)
        })
        .build())
}

pub fn delete_from_playlist_or_show_confirmation(
    playlist_name: String,
    song_paths: &HashSet<String>,
    confirmation: bool,
    ctx: &Ctx,
) -> Result<()> {
    let pl_name = playlist_name.clone();
    let Some(songs_in_playlist) =
        ctx.query_sync(move |client| match client.list_playlist_info(&pl_name, None) {
            Ok(val) => Ok(Some(val.into_iter().map(|s| s.file).collect_vec())),
            Err(MpdError::Mpd(MpdFailureResponse { code: ErrorCode::NoExist, .. })) => {
                status_warn!("Cannot remove song(s) from playlist, playlist does not exist");
                Ok(None)
            }
            Err(err) => Err(err.into()),
        })?
    else {
        return Ok(());
    };

    let mut songs_to_remove_in_playlist = Vec::new();
    for (idx, song) in songs_in_playlist.into_iter().enumerate() {
        if song_paths.contains(&song) {
            songs_to_remove_in_playlist.push((idx, song));
        }
    }
    let songs_to_remove = songs_to_remove_in_playlist.len();

    if songs_to_remove == 0 {
        status_warn!("No matching songs found in playlist");
        return Ok(());
    }

    let confirmation_message =
        format!("Remove {songs_to_remove} song(s) from playlist \"{playlist_name}\"?");

    let delete_songs = move |ctx: &Ctx| {
        ctx.command(move |client| {
            client.send_start_cmd_list()?;
            for (idx, _path) in songs_to_remove_in_playlist.iter().rev() {
                client.send_delete_from_playlist(&playlist_name, &SingleOrRange::single(*idx))?;
            }
            client.send_execute_cmd_list()?;
            client.read_ok()?;
            status_info!("Removed {songs_to_remove} song(s) from playlist \"{playlist_name}\"",);
            Ok(())
        });
    };

    if confirmation {
        let modal = ConfirmModal::builder()
            .ctx(ctx)
            .message(vec![confirmation_message])
            .action(Action::Single {
                confirm_label: Some("Delete"),
                cancel_label: None,
                on_confirm: Box::new(|ctx| {
                    delete_songs(ctx);
                    Ok(())
                }),
            })
            .build();
        modal!(ctx, modal);
    } else {
        delete_songs(ctx);
    }

    Ok(())
}
