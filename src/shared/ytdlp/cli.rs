use std::{path::PathBuf, str::FromStr};

use anyhow::{Result, bail};
use itertools::Itertools;

use crate::{
    config::cli_config::CliConfig,
    shared::{
        dependencies,
        ytdlp::{YtDlp, YtDlpHost, ytdlp_item::YtDlpContent},
    },
};

pub fn init_and_download(config: &CliConfig, url: &str) -> Result<Vec<PathBuf>> {
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

    let ytdlp = YtDlp::new(cache_dir.clone());
    let resolved = YtDlpContent::from_str(url)?;

    match resolved {
        YtDlpContent::Single(host) => {
            let result = ytdlp
                .download_single(&host)
                .inspect(|ok| {
                    eprintln!("Downloaded '{}'", ok.file_path.display());
                })
                .inspect_err(|err| {
                    eprintln!("Failed to download: {err}");
                })?;

            Ok(vec![result.file_path])
        }
        YtDlpContent::Playlist(playlist) => {
            let resolved = ytdlp.resolve_playlist_urls(&playlist)?;
            let success = resolved
                .iter()
                .map(|item| {
                    ytdlp
                        .download_single(item)
                        .inspect(|ok| {
                            eprintln!("Downloaded '{}' from playlist", ok.file_path.display());
                        })
                        .inspect_err(|err| {
                            eprintln!("Failed to download item from playlist: {err}");
                        })
                        .map(|res| res.file_path)
                })
                // Drop all the unsuccessful ones, errors have been printed out to stderr already
                .filter_map(|item| item.ok())
                .collect_vec();

            Ok(success)
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
