use anyhow::Result;
use itertools::Itertools;
use std::{io::Write, path::PathBuf};

use crate::{
    config::{cli::Command, Config},
    context::AppContext,
    mpd::{
        commands::{volume::Bound, IdleEvent},
        mpd_client::{Filter, MpdClient, Tag},
    },
    shared::{lrc::LrcIndex, macros::status_error, ytdlp::YtDlp},
};
use anyhow::bail;

impl Command {
    pub fn execute<C>(self, client: &mut C, config: &Config) -> Result<()>
    where
        C: MpdClient,
    {
        match self {
            ref cmd @ Command::Update { ref path, wait } | ref cmd @ Command::Rescan { ref path, wait } => {
                let crate::mpd::commands::Update { job_id } = if matches!(cmd, Command::Update { .. }) {
                    client.update(path.as_deref())?
                } else {
                    client.rescan(path.as_deref())?
                };

                if wait {
                    loop {
                        client.idle(Some(IdleEvent::Update))?;
                        let crate::mpd::commands::Status { updating_db, .. } = client.get_status()?;
                        match updating_db {
                            Some(current_id) if current_id > job_id => break,
                            Some(_id) => continue,
                            None => break,
                        }
                    }
                }
            }
            Command::LyricsIndex => {
                let Some(dir) = config.lyrics_dir else {
                    bail!("Lyrics dir is not configured");
                };
                println!(
                    "{}",
                    serde_json::to_string_pretty(&LrcIndex::index(&PathBuf::from(dir))?)?
                );
            }
            Command::Play { position: None } => client.play()?,
            Command::Play { position: Some(pos) } => client.play_pos(pos)?,
            Command::Pause => client.pause()?,
            Command::TogglePause => client.pause_toggle()?,
            Command::Unpause => client.unpause()?,
            Command::Stop => client.stop()?,
            Command::Volume { value: Some(value) } => client.volume(value.parse()?)?,
            Command::Volume { value: None } => println!("{}", client.get_status()?.volume.value()),
            Command::Next => client.next()?,
            Command::Prev => client.prev()?,
            Command::Repeat { value } => client.repeat((value).into())?,
            Command::Random { value } => client.random((value).into())?,
            Command::Single { value } => client.single((value).into())?,
            Command::Consume { value } => client.consume((value).into())?,
            Command::Seek { value } => client.seek_current(value.parse()?)?,
            Command::Clear => client.clear()?,
            Command::Add { file } => client.add(&file)?,
            Command::AddYt { url } => {
                YtDlp::download_and_add(config, &url, client)?;
            }
            Command::Decoders => println!("{}", serde_json::ser::to_string(&client.decoders()?)?),
            Command::Outputs => println!("{}", serde_json::ser::to_string(&client.outputs()?)?),
            Command::Config { .. } => bail!("Cannot use config command here."),
            Command::Theme { .. } => bail!("Cannot use theme command here."),
            Command::Version => bail!("Cannot use version command here."),
            Command::DebugInfo => bail!("Cannot use debuginfo command here."),
            Command::ToggleOutput { id } => client.toggle_output(id)?,
            Command::EnableOutput { id } => client.enable_output(id)?,
            Command::DisableOutput { id } => client.disable_output(id)?,
            Command::Status => println!("{}", serde_json::ser::to_string(&client.get_status()?)?),
            Command::Song { path: Some(paths) } if paths.len() == 1 => {
                let path = &paths[0];
                if let Some(song) = client.find_one(&[Filter::new(Tag::File, path.as_str())])? {
                    println!("{}", serde_json::ser::to_string(&song)?);
                } else {
                    println!("Song with path '{path}' not found.");
                    std::process::exit(1);
                }
            }
            Command::Song { path: Some(paths) } => {
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
            }
            Command::Song { path: None } => {
                let current_song = client.get_current_song()?;
                if let Some(song) = current_song {
                    println!("{}", serde_json::ser::to_string(&song)?);
                } else {
                    std::process::exit(1);
                }
            }
            Command::Mount { ref name, ref path } => client.mount(name, path)?,
            Command::Unmount { ref name } => client.unmount(name)?,
            Command::ListMounts => println!("{}", serde_json::ser::to_string(&client.list_mounts()?)?),
            Command::AlbumArt { output } => {
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
                } else {
                    std::fs::OpenOptions::new()
                        .write(true)
                        .create(true)
                        .truncate(true)
                        .open(output)?
                        .write_all(&album_art)?;
                }
            }
        };
        Ok(())
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
