use std::{path::PathBuf, str::FromStr};

use anyhow::{Result, bail};

use crate::{
    config::cli_config::CliConfig,
    shared::{
        dependencies,
        ytdlp::{YtDlp, YtDlpHost, ytdlp_item::YtDlpContent},
    },
};

pub fn init_and_download(
    config: &CliConfig,
    url: &str,
    mut for_each: impl FnMut(PathBuf) -> Result<()>,
) -> Result<()> {
    let Some(cache_dir) = &config.cache_dir else {
        bail!("Youtube support requires 'cache_dir' to be configured")
    };

    if let Err(unsupported_list) = dependencies::is_youtube_supported(&config.address) {
        log::warn!(
            "Youtube support requires the following and may thus not work properly: {}",
            unsupported_list.join(", ")
        );
    } else {
        println!("Downloading '{url}'");
    }

    let ytdlp = YtDlp::new(cache_dir.clone(), &config.extra_yt_dlp_args);
    let resolved = YtDlpContent::from_str(url)?;

    match resolved {
        YtDlpContent::Single(host) => {
            match ytdlp.download_single(&host) {
                Ok(result) => {
                    eprintln!("Downloaded '{}'", result.file_path.display());
                    for_each(result.file_path)?;
                }
                Err(err) => {
                    eprintln!("Failed to download: {err}");
                }
            }

            Ok(())
        }
        YtDlpContent::Playlist(playlist) => {
            eprintln!("Resolving playlist '{}'", playlist.id);
            let resolved = ytdlp.resolve_playlist_urls(&playlist)?;
            for item in resolved {
                match ytdlp.download_single(&item) {
                    Ok(result) => {
                        eprintln!("Downloaded '{}' from playlist", result.file_path.display());
                        for_each(result.file_path)?;
                    }
                    Err(err) => {
                        eprintln!("Failed to download from playlist: {err}");
                    }
                }
            }

            Ok(())
        }
    }
}

pub fn search_pick_cli(kind: YtDlpHost, query: &str, limit: usize) -> anyhow::Result<String> {
    use dialoguer::{Select, theme::ColorfulTheme};

    let items = YtDlp::search(kind, query, limit)?;

    if items.is_empty() {
        anyhow::bail!("No results found for query '{query}'");
    }

    // Build labels + a trailing “Cancel”
    let mut labels: Vec<String> =
        items.iter().map(|it| it.title.as_deref().unwrap_or("<no title>").to_string()).collect();
    labels.push("⟲ Cancel".to_string());

    let sel = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Select a track")
        .items(&labels)
        .default(0)
        .interact_opt()?;

    match sel {
        Some(idx) if idx + 1 == labels.len() => anyhow::bail!("Selection canceled"),
        Some(idx) => Ok(items[idx].url.clone()),
        None => anyhow::bail!("Selection canceled"),
    }
}
