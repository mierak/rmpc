use anyhow::{anyhow, Result};
use itertools::Itertools;
use std::io::Write;

use crate::{
    config::{cli::Command, Config},
    mpd::{commands::volume::Bound, mpd_client::MpdClient},
    utils::macros::status_error,
    WorkRequest,
};
use anyhow::bail;

impl Command {
    pub fn execute<F, C>(
        self,
        client: &mut C,
        _config: &'static Config,
        mut request_work: F,
    ) -> Result<(), anyhow::Error>
    where
        C: MpdClient,
        F: FnMut(WorkRequest, &mut C),
    {
        match self {
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
                request_work(WorkRequest::DownloadYoutube { url }, client);
            }
            Command::Outputs => println!("{}", serde_json::ser::to_string(&client.outputs()?)?),
            Command::Config => bail!("Cannot use config command here."),
            Command::Theme => bail!("Cannot use theme command here."),
            Command::Version => bail!("Cannot use version command here."),
            Command::DebugInfo => bail!("Cannot use debuginfo command here."),
            Command::ToggleOutput { id } => client.toggle_output(id)?,
            Command::EnableOutput { id } => client.enable_output(id)?,
            Command::DisableOutput { id } => client.disable_output(id)?,
            Command::Status => println!("{}", serde_json::ser::to_string(&client.get_status()?)?),
            Command::Song => println!("{}", serde_json::ser::to_string(&client.get_current_song()?)?),
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
    E: IntoIterator<Item = (&'a str, &'a str)>,
{
    let [cmd, args @ ..] = command else { todo!() };

    let mut cmd = std::process::Command::new(cmd);
    cmd.args(args);

    for (key, val) in envs {
        cmd.env(key, val);
    }

    let out = match cmd.output() {
        Ok(out) => out,
        Err(err) => {
            return Err(anyhow!("Unexpected error when executing external command: {:?}", err));
        }
    };

    if !out.status.success() {
        return Err(anyhow!(
            "External command failed: exit code: '{}', stdout: '{}', stderr: '{}'",
            out.status.code().map_or_else(|| "-".to_string(), |v| v.to_string()),
            String::from_utf8_lossy(&out.stdout).trim(),
            String::from_utf8_lossy(&out.stderr).trim()
        ));
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
