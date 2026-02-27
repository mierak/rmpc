use std::{
    io::{BufRead, Cursor, Read},
    path::PathBuf,
    sync::Arc,
};

use anyhow::Result;
use crossbeam::channel::{Receiver, Sender};

use crate::{
    config::{Config, cli_config::CliConfig},
    shared::{
        events::{AppEvent, ClientRequest, LoadAlbumArtResult, WorkDone, WorkRequest},
        lrc::LrcIndex,
        macros::try_skip,
        mpd_query::MpdCommand as QueryCmd,
        ytdlp::{YtDlp, YtDlpDownloadError},
    },
};

pub fn init(
    work_rx: Receiver<WorkRequest>,
    client_tx: Sender<ClientRequest>,
    event_tx: Sender<AppEvent>,
    config: Arc<Config>,
) -> std::io::Result<std::thread::JoinHandle<()>> {
    std::thread::Builder::new().name("work".to_owned()).spawn(move || {
        let ytdlp =
            config.cache_dir.as_ref().map(|dir| YtDlp::new(dir.clone(), &config.extra_yt_dlp_args));
        let cli_config = config.as_ref().into();
        while let Ok(req) = work_rx.recv() {
            let result = handle_work_request(req, &client_tx, &cli_config, ytdlp.as_ref());
            try_skip!(
                event_tx.send(AppEvent::WorkDone(result)),
                "Failed to send work done notification"
            );
        }
    })
}

fn handle_work_request(
    request: WorkRequest,
    client_tx: &Sender<ClientRequest>,
    config: &CliConfig,
    ytdlp: Option<&YtDlp>,
) -> Result<WorkDone> {
    match request {
        WorkRequest::Command(command) => {
            let callback = command.execute(config)?; // TODO log
            try_skip!(
                client_tx.send(ClientRequest::Command(QueryCmd { callback })),
                "Failed to send client request to complete command"
            );
            Ok(WorkDone::None)
        }
        WorkRequest::IndexLyrics { lyrics_dir } => {
            let index = LrcIndex::index(&PathBuf::from(lyrics_dir));
            Ok(WorkDone::LyricsIndexed { index })
        }
        WorkRequest::IndexSingleLrc { path } => {
            let metadata = LrcIndex::index_single(&path)?;
            Ok(WorkDone::SingleLrcIndexed { path, metadata })
        }
        WorkRequest::ResizeImage(fn_once) => Ok(WorkDone::ImageResized { data: fn_once() }),
        WorkRequest::SearchYt { query, kind, limit, interactive, position } => {
            if ytdlp.is_none() {
                anyhow::bail!("Youtube support requires 'cache_dir' to be configured")
            }

            let limit = if interactive { limit } else { 1 };
            let items = YtDlp::search(kind, &query, limit)?;

            Ok(WorkDone::SearchYtResults { items, position, interactive })
        }
        WorkRequest::YtDlpDownload { id, url } => {
            let Some(ytdlp) = ytdlp else {
                return Ok(WorkDone::YtDlpDownloaded {
                    id,
                    result: Err(YtDlpDownloadError::InvalidConfig(
                        "Youtube support requires 'cache_dir' to be configured",
                    )),
                });
            };

            let result = ytdlp.download_single(&url);
            Ok(WorkDone::YtDlpDownloaded { id, result })
        }
        WorkRequest::YtDlpResolvePlaylist { playlist } => {
            let Some(ytdlp) = ytdlp else {
                anyhow::bail!("Youtube support requires 'cache_dir' to be configured")
            };

            let result = ytdlp.resolve_playlist_urls(&playlist)?;
            Ok(WorkDone::YtDlpPlaylistResolved { urls: result })
        }
        WorkRequest::LoadAlbumArt { file, loader } => {
            let Some((program, args)) = loader.split_first() else {
                return Ok(WorkDone::AlbumArtLoaded {
                    result: LoadAlbumArtResult::Failure {
                        file,
                        message: "no album art loader specified".to_string(),
                    },
                });
            };

            let mut cmd = std::process::Command::new(program);
            cmd.args(args);
            cmd.env("FILE", &file);

            let result = match cmd.output() {
                Ok(result) => result,
                Err(err) => {
                    return Ok(WorkDone::AlbumArtLoaded {
                        result: LoadAlbumArtResult::Failure {
                            file,
                            message: format!("failed to execute album art loader: {err}"),
                        },
                    });
                }
            };

            if !result.status.success() {
                return Ok(WorkDone::AlbumArtLoaded {
                    result: LoadAlbumArtResult::Failure {
                        file,
                        message: format!(
                            "exited with a non-zero exit code: {:?}",
                            result.status.code()
                        ),
                    },
                });
            }

            let mut cursor = Cursor::new(result.stdout);
            let mut buf = String::new();
            let mut bytes_to_read = None;

            let result = loop {
                buf.clear();
                let bytes_read = cursor.read_line(&mut buf)?;
                if bytes_read == 0 {
                    break LoadAlbumArtResult::Failure {
                        file,
                        message: "invalid album art loader output".to_string(),
                    };
                }
                match buf.split_once(':').map(|(a, b)| (a.trim(), b.trim())) {
                    Some(("size", size)) => match size.parse::<usize>() {
                        Ok(size) => {
                            bytes_to_read = Some(size);
                        }
                        Err(err) => {
                            break LoadAlbumArtResult::Failure {
                                file,
                                message: format!(
                                    "invalid album art loader size value: {size}, error: {err}"
                                ),
                            };
                        }
                    },
                    Some(("action", action)) => match action {
                        "display" => {
                            let Some(bytes) = bytes_to_read else {
                                break LoadAlbumArtResult::Failure {
                                    file,
                                    message: "missing album art loader size before display action"
                                        .to_string(),
                                };
                            };
                            let mut data = vec![0u8; bytes];
                            if let Err(e) = cursor.read_exact(&mut data) {
                                break LoadAlbumArtResult::Failure {
                                    file,
                                    message: format!(
                                        "failed to read album art loader binary data: {e}"
                                    ),
                                };
                            }
                            break LoadAlbumArtResult::Loaded { file, data };
                        }
                        "displaydefault" => {
                            break LoadAlbumArtResult::DisplayDefault { file };
                        }
                        "fallback" => {
                            break LoadAlbumArtResult::Fallback { file };
                        }
                        action => {
                            break LoadAlbumArtResult::Failure {
                                file,
                                message: format!(
                                    "unknown album art loader action: {action}, possible actions are: display, displaydefault, fallback"
                                ),
                            };
                        }
                    },
                    Some((k, v)) => {
                        log::warn!("Unknown album art loader output key: {k} with value: {v}");
                    }
                    None => {
                        break LoadAlbumArtResult::Failure {
                            file,
                            message: "missing album art loader action".to_string(),
                        };
                    }
                }
            };

            Ok(WorkDone::AlbumArtLoaded { result })
        }
    }
}
