use anyhow::Result;
use itertools::Itertools;
use std::{io::Write, path::PathBuf};

use crate::{
    config::{
        cli::{Command, StickerCmd},
        Config,
    },
    context::AppContext,
    mpd::{
        client::Client,
        commands::{volume::Bound, IdleEvent},
        mpd_client::{Filter, MpdClient, Tag},
    },
    shared::{
        lrc::LrcIndex,
        macros::{status_error, status_info},
        ytdlp::YtDlp,
    },
};
use anyhow::bail;

impl Command {
    pub fn execute(
        mut self,
        config: &'static Config,
    ) -> Result<Box<dyn FnOnce(&mut Client<'_>) -> Result<()> + Send + 'static>> {
        match self {
            Command::Update { ref mut path, wait } | Command::Rescan { ref mut path, wait } => {
                let path = path.take();
                Ok(Box::new(move |client| {
                    let crate::mpd::commands::Update { job_id } = if matches!(self, Command::Update { .. }) {
                        client.update(path.as_deref())?
                    } else {
                        client.rescan(path.as_deref())?
                    };

                    if wait {
                        loop {
                            client.idle(Some(IdleEvent::Update))?;
                            log::trace!("issuing update");
                            let crate::mpd::commands::Status { updating_db, .. } = client.get_status()?;
                            log::trace!("update done");
                            match updating_db {
                                Some(current_id) if current_id > job_id => break,
                                Some(_id) => continue,
                                None => break,
                            }
                        }
                    }
                    Ok(())
                }))
            }
            Command::LyricsIndex => Ok(Box::new(|_| {
                let Some(dir) = config.lyrics_dir else {
                    bail!("Lyrics dir is not configured");
                };
                println!(
                    "{}",
                    serde_json::to_string_pretty(&LrcIndex::index(&PathBuf::from(dir)))?
                );
                Ok(())
            })),
            Command::Play { position: None } => Ok(Box::new(|client| Ok(client.play()?))),
            Command::Play { position: Some(pos) } => Ok(Box::new(move |client| Ok(client.play_pos(pos)?))),
            Command::Pause => Ok(Box::new(|client| Ok(client.pause()?))),
            Command::TogglePause => Ok(Box::new(|client| Ok(client.pause_toggle()?))),
            Command::Unpause => Ok(Box::new(|client| Ok(client.unpause()?))),
            Command::Stop => Ok(Box::new(|client| Ok(client.stop()?))),
            Command::Volume { value: Some(value) } => Ok(Box::new(move |client| Ok(client.volume(value.parse()?)?))),
            Command::Volume { value: None } => Ok(Box::new(|client| {
                println!("{}", client.get_status()?.volume.value());
                Ok(())
            })),
            Command::Next => Ok(Box::new(|client| Ok(client.next()?))),
            Command::Prev => Ok(Box::new(|client| Ok(client.prev()?))),
            Command::Repeat { value } => Ok(Box::new(move |client| Ok(client.repeat((value).into())?))),
            Command::Random { value } => Ok(Box::new(move |client| Ok(client.random((value).into())?))),
            Command::Single { value } => Ok(Box::new(move |client| Ok(client.single((value).into())?))),
            Command::Consume { value } => Ok(Box::new(move |client| Ok(client.consume((value).into())?))),
            Command::Seek { value } => Ok(Box::new(move |client| Ok(client.seek_current(value.parse()?)?))),
            Command::Clear => Ok(Box::new(|client| Ok(client.clear()?))),
            Command::Add { file } => Ok(Box::new(move |client| Ok(client.add(&file)?))),
            Command::AddYt { url } => {
                let file_path = YtDlp::init_and_download(config, &url)?;
                status_info!("file path {file_path}");
                Ok(Box::new(move |client| match client.add(&file_path) {
                    Ok(()) => {
                        status_info!("File '{file_path}' added to the queue");
                        Ok(())
                    }
                    Err(err) => {
                        status_error!(err:?; "Failed to add '{file_path}' to the queue");
                        Err(err.into())
                    }
                }))
            }
            Command::Decoders => Ok(Box::new(|client| {
                println!("{}", serde_json::ser::to_string(&client.decoders()?)?);
                Ok(())
            })),
            Command::Outputs => Ok(Box::new(|client| {
                println!("{}", serde_json::ser::to_string(&client.outputs()?)?);
                Ok(())
            })),
            Command::Config { .. } => bail!("Cannot use config command here."),
            Command::Theme { .. } => bail!("Cannot use theme command here."),
            Command::Version => bail!("Cannot use version command here."),
            Command::DebugInfo => bail!("Cannot use debuginfo command here."),
            Command::ToggleOutput { id } => Ok(Box::new(move |client| Ok(client.toggle_output(id)?))),
            Command::EnableOutput { id } => Ok(Box::new(move |client| Ok(client.enable_output(id)?))),
            Command::DisableOutput { id } => Ok(Box::new(move |client| Ok(client.disable_output(id)?))),
            Command::Status => Ok(Box::new(|client| {
                println!("{}", serde_json::ser::to_string(&client.get_status()?)?);
                Ok(())
            })),
            Command::Song { path: Some(paths) } if paths.len() == 1 => Ok(Box::new(move |client| {
                let path = &paths[0];
                if let Some(song) = client.find_one(&[Filter::new(Tag::File, path.as_str())])? {
                    println!("{}", serde_json::ser::to_string(&song)?);
                    Ok(())
                } else {
                    println!("Song with path '{path}' not found.");
                    std::process::exit(1);
                }
            })),
            Command::Song { path: Some(paths) } => Ok(Box::new(move |client| {
                let mut songs = Vec::new();
                for path in &paths {
                    if let Some(song) = client.find_one(&[Filter::new(Tag::File, path.as_str())])? {
                        songs.push(song);
                    } else {
                        println!("Song with path '{path}' not found.");
                        std::process::exit(1);
                    }
                }
                println!("{}", serde_json::ser::to_string(&songs)?);
                Ok(())
            })),
            Command::Song { path: None } => Ok(Box::new(|client| {
                let current_song = client.get_current_song()?;
                if let Some(song) = current_song {
                    println!("{}", serde_json::ser::to_string(&song)?);
                    Ok(())
                } else {
                    std::process::exit(1);
                }
            })),
            Command::Mount { name, path } => Ok(Box::new(move |client| Ok(client.mount(&name, &path)?))),
            Command::Unmount { name } => Ok(Box::new(move |client| Ok(client.unmount(&name)?))),
            Command::ListMounts => Ok(Box::new(|client| {
                println!("{}", serde_json::ser::to_string(&client.list_mounts()?)?);
                Ok(())
            })),
            Command::AlbumArt { output } => Ok(Box::new(move |client| {
                let Some(song) = client.get_current_song()? else {
                    std::process::exit(3);
                };

                let album_art = client.find_album_art(&song.file)?;

                let Some(album_art) = album_art else {
                    std::process::exit(2);
                };

                if &output == "-" {
                    std::io::stdout().write_all(&album_art)?;
                    std::io::stdout().flush()?;
                    Ok(())
                } else {
                    std::fs::OpenOptions::new()
                        .write(true)
                        .create(true)
                        .truncate(true)
                        .open(output)?
                        .write_all(&album_art)?;
                    Ok(())
                }
            })),
            Command::Sticker {
                cmd: StickerCmd::Set { uri, key, value },
            } => Ok(Box::new(move |client| {
                client.set_sticker(&uri, &key, &value)?;
                Ok(())
            })),
            Command::Sticker {
                cmd: StickerCmd::Get { uri, key },
            } => Ok(Box::new(move |client| {
                let sticker = client.sticker(&uri, &key)?;
                println!("{}", serde_json::ser::to_string(&sticker)?);
                Ok(())
            })),
            Command::Sticker {
                cmd: StickerCmd::Delete { uri, key },
            } => Ok(Box::new(move |client| {
                client.delete_sticker(&uri, &key)?;
                Ok(())
            })),
            Command::Sticker {
                cmd: StickerCmd::DeleteAll { uri },
            } => Ok(Box::new(move |client| {
                client.delete_all_stickers(&uri)?;
                Ok(())
            })),
            Command::Sticker {
                cmd: StickerCmd::List { uri },
            } => Ok(Box::new(move |client| {
                let stickers = client.list_stickers(&uri)?;
                println!("{}", serde_json::ser::to_string(&stickers)?);
                Ok(())
            })),
            Command::Sticker {
                cmd: StickerCmd::Find { uri, key },
            } => Ok(Box::new(move |client| {
                let stickers = client.find_stickers(&uri, &key)?;
                println!("{}", serde_json::ser::to_string(&stickers)?);
                Ok(())
            })),
        }
    }
}

pub fn run_external_blocking<'a, E>(command: &[&str], envs: E) -> Result<()>
where
    E: IntoIterator<Item = (&'a str, &'a str)> + std::fmt::Debug,
{
    let [cmd, args @ ..] = command else {
        bail!("Invalid command: {:?}", command);
    };

    let mut cmd = std::process::Command::new(cmd);
    cmd.args(args);

    for (key, val) in envs {
        cmd.env(key, val);
    }

    log::debug!(command:?; "Running external command");
    log::trace!(command:?, envs:? = cmd.get_envs(); "Running external command");

    let out = match cmd.output() {
        Ok(out) => out,
        Err(err) => {
            bail!("Unexpected error when executing external command: {:?}", err);
        }
    };

    if !out.status.success() {
        bail!(
            "External command failed: exit code: '{}', stdout: '{}', stderr: '{}'",
            out.status.code().map_or_else(|| "-".to_string(), |v| v.to_string()),
            String::from_utf8_lossy(&out.stdout).trim(),
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }

    Ok(())
}

pub fn run_external<'a: 'static, K: Into<String>, V: Into<String>>(command: &'a [&'a str], envs: Vec<(K, V)>) {
    let envs = envs.into_iter().map(|(k, v)| (k.into(), v.into())).collect_vec();

    std::thread::spawn(move || {
        if let Err(err) = run_external_blocking(command, envs.iter().map(|(k, v)| (k.as_str(), v.as_str()))) {
            status_error!("{}", err);
        }
    });
}

pub fn create_env<'a>(
    context: &AppContext,
    selected_songs_paths: impl IntoIterator<Item = &'a str>,
) -> Vec<(impl Into<String>, impl Into<String>)> {
    let mut result = Vec::new();

    if let Some((_, current)) = context.find_current_song_in_queue() {
        result.push(("CURRENT_SONG", current.file.clone()));
    }

    let songs = selected_songs_paths
        .into_iter()
        .enumerate()
        .fold(String::new(), |mut acc, (idx, val)| {
            if idx > 0 {
                acc.push('\n');
            }
            acc.push_str(val);
            acc
        });

    if !songs.is_empty() {
        result.push(("SELECTED_SONGS", songs));
    }

    result.push(("STATE", context.status.state.to_string()));

    result
}
