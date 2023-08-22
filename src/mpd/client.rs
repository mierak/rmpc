use anyhow::Result;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::{
        tcp::{OwnedReadHalf, OwnedWriteHalf},
        TcpStream,
    },
};
use tracing::{debug, trace};

use super::{
    commands::{volume::Bound, *},
    errors::{ErrorCode, MpdError, MpdFailureResponse},
    response::{BinaryMpdResponse, EmptyMpdResponse, MpdResponse},
};

type MpdResult<T> = Result<T, MpdError>;

pub struct Client<'a> {
    name: Option<&'a str>,
    rx: BufReader<OwnedReadHalf>,
    tx: OwnedWriteHalf,
    reconnect: bool,
    addr: String,
}

impl std::fmt::Debug for Client<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Client {{ name: {:?}, recconect: {}, addr: {} }}",
            self.name, self.reconnect, self.addr
        )
    }
}

#[allow(dead_code)]
impl<'a> Client<'a> {
    #[tracing::instrument]
    pub async fn init(addr: String, name: Option<&'a str>, reconnect: bool) -> MpdResult<Client<'a>> {
        let stream = TcpStream::connect(&addr).await?;
        let (rx, tx) = stream.into_split();
        let mut rx = BufReader::new(rx);

        let mut buf = String::new();
        rx.read_line(&mut buf).await?;
        if !buf.starts_with("OK") {
            return Err(MpdError::Generic(format!("Handshake validation failed. '{buf}'")));
        };

        debug!(message = "MPD client initiazed", handshake = buf.trim());
        Ok(Self {
            rx,
            tx,
            reconnect,
            addr,
            name,
        })
    }

    #[tracing::instrument]
    async fn reconnect(&mut self) -> MpdResult<&Client> {
        let stream = TcpStream::connect(&self.addr).await?;
        let (rx, tx) = stream.into_split();
        let mut rx = BufReader::new(rx);

        let mut buf = String::new();
        rx.read_line(&mut buf).await?;
        if !buf.starts_with("OK") {
            return Err(MpdError::Generic(format!("Handshake validation failed. '{buf}'")));
        };
        self.rx = rx;
        self.tx = tx;

        debug!(message = "MPD client reconnected", handshake = buf.trim(),);
        Ok(self)
    }

    // Queries
    #[tracing::instrument(skip(self))]
    pub async fn idle(&mut self) -> MpdResult<IdleEvents> {
        Ok(self.execute(IDLE_COMMAND).await?.unwrap())
    }

    #[tracing::instrument(skip(self))]
    pub async fn get_volume(&mut self) -> MpdResult<Volume> {
        Ok(self.execute(VOLUME_COMMAND).await?.unwrap())
    }

    #[tracing::instrument(skip(self))]
    pub async fn set_volume(&mut self, volume: &Volume) -> MpdResult<()> {
        self.execute_ok(format!("setvol {}", volume.value()).as_bytes()).await
    }

    #[tracing::instrument(skip(self))]
    pub async fn get_current_song(&mut self) -> MpdResult<Option<Song>> {
        self.execute(CURRENTSONG_COMMAND).await
    }

    #[tracing::instrument(skip(self))]
    pub async fn get_status(&mut self) -> MpdResult<Status> {
        Ok(self.execute(STATUS_COMMAND).await?.unwrap())
    }

    // Playback control
    #[tracing::instrument(skip(self))]
    pub async fn pause_toggle(&mut self) -> MpdResult<()> {
        self.execute_ok(b"pause").await
    }

    #[tracing::instrument(skip(self))]
    pub async fn next(&mut self) -> MpdResult<()> {
        self.execute_ok(b"next").await
    }

    #[tracing::instrument(skip(self))]
    pub async fn prev(&mut self) -> MpdResult<()> {
        self.execute_ok(b"previous").await
    }

    #[tracing::instrument(skip(self))]
    pub async fn play_pos(&mut self, pos: u32) -> MpdResult<()> {
        self.execute_ok(format!("play {pos}").as_bytes()).await
    }

    #[tracing::instrument(skip(self))]
    pub async fn play(&mut self) -> MpdResult<()> {
        self.execute_ok(b"play").await
    }

    #[tracing::instrument(skip(self))]
    pub async fn play_id(&mut self, id: u32) -> MpdResult<()> {
        self.execute_ok(format!("playid {id}").as_bytes()).await
    }

    #[tracing::instrument(skip(self))]
    pub async fn stop(&mut self) -> MpdResult<()> {
        self.execute_ok(b"stop").await
    }

    #[tracing::instrument(skip(self))]
    pub async fn seek_curr_forwards(&mut self, time_sec: u32) -> MpdResult<()> {
        self.execute_ok(format!("seekcur +{time_sec}").as_bytes()).await
    }

    #[tracing::instrument(skip(self))]
    pub async fn seek_curr_backwards(&mut self, time_sec: u32) -> MpdResult<()> {
        self.execute_ok(format!("seekcur -{time_sec}").as_bytes()).await
    }

    #[tracing::instrument(skip(self))]
    pub async fn repeat(&mut self, enabled: bool) -> MpdResult<()> {
        self.execute_ok(format!("repeat {}", enabled as u8).as_bytes()).await
    }

    #[tracing::instrument(skip(self))]
    pub async fn random(&mut self, enabled: bool) -> MpdResult<()> {
        self.execute_ok(format!("random {}", enabled as u8).as_bytes()).await
    }

    #[tracing::instrument(skip(self))]
    pub async fn single(&mut self, enabled: bool) -> MpdResult<()> {
        self.execute_ok(format!("single {}", enabled as u8).as_bytes()).await
    }

    // Current queue
    #[tracing::instrument(skip(self))]
    pub async fn add(&mut self, path: &str) -> MpdResult<()> {
        self.execute_ok(format!("add \"{path}\"").as_bytes()).await
    }

    #[tracing::instrument(skip(self))]
    pub async fn delete_id(&mut self, id: u32) -> MpdResult<()> {
        self.execute_ok(format!("deleteid \"{id}\"").as_bytes()).await
    }

    #[tracing::instrument(skip(self))]
    pub async fn playlist_info(&mut self) -> MpdResult<Option<PlayListInfo>> {
        self.execute(PLAYLIST_INFO_COMMAND).await
    }

    // Database
    #[tracing::instrument(skip(self))]
    pub async fn lsinfo(&mut self, path: Option<&str>) -> MpdResult<LsInfo> {
        if let Some(path) = path {
            Ok(self.execute(format!("lsinfo \"{}\"", path).as_bytes()).await?.unwrap())
        } else {
            Ok(self.execute(b"lsinfo").await?.unwrap())
        }
    }

    #[tracing::instrument(skip(self))]
    pub async fn list_files(&mut self, path: Option<&str>) -> MpdResult<ListFiles> {
        if let Some(path) = path {
            Ok(self
                .execute(format!("listfiles \"{}\"", path).as_bytes())
                .await?
                .unwrap())
        } else {
            Ok(self.execute(b"listfiles").await?.unwrap())
        }
    }

    #[tracing::instrument(skip(self))]
    pub async fn read_picture(&mut self, path: &str) -> MpdResult<Vec<u8>> {
        self.execute_binary(format!("readpicture \"{}\"", path).as_bytes())
            .await
    }

    #[tracing::instrument(skip(self))]
    pub async fn albumart(&mut self, path: &str) -> MpdResult<Vec<u8>> {
        self.execute_binary(format!("albumart \"{}\"", path).as_bytes()).await
    }

    /// This function first invokes [albumart].
    /// If no album art is fonud it invokes [readpicture].
    /// If no art is still found, but no errors were encountered, None is returned.
    #[tracing::instrument(skip(self))]
    pub async fn find_album_art(&mut self, path: &str) -> MpdResult<Option<Vec<u8>>> {
        match self.albumart(path).await {
            Ok(v) => Ok(Some(v)),
            Err(MpdError::Mpd(MpdFailureResponse {
                code: ErrorCode::NoExist,
                ..
            })) => match self.read_picture(path).await {
                Ok(p) => Ok(Some(p)),
                Err(MpdError::Mpd(MpdFailureResponse {
                    code: ErrorCode::NoExist,
                    ..
                })) => {
                    tracing::debug!(message = "No album art found, fallback to placeholder image here.");
                    Ok(None)
                }
                Err(e) => {
                    tracing::error!(message = "Failed to read picture", error = ?e);
                    Ok(None)
                }
            },
            Err(e) => {
                tracing::error!(message = "Failed to read picture", error = ?e);
                Ok(None)
            }
        }
    }

    #[tracing::instrument(skip(self), fields(command = ?String::from_utf8_lossy(command)))]
    async fn execute_binary(&mut self, command: &[u8]) -> MpdResult<Vec<u8>> {
        let mut buf = Vec::new();
        self.tx
            .write_all(&[command, b" ", buf.len().to_string().as_bytes(), b"\n"].concat())
            .await?;
        let _ = match BinaryMpdResponse::from_read(&mut self.rx, &mut buf).await {
            Ok(v) => Ok(v),
            Err(MpdError::ClientClosed) if self.reconnect => {
                self.reconnect().await?;
                self.tx
                    .write_all(&[command, b" ", buf.len().to_string().as_bytes(), b"\n"].concat())
                    .await?;
                BinaryMpdResponse::from_read(&mut self.rx, &mut buf).await
            }
            Err(e) => Err(e),
        };
        loop {
            self.tx
                .write_all(&[command, b" ", buf.len().to_string().as_bytes(), b"\n"].concat())
                .await?;
            let response = BinaryMpdResponse::from_read(&mut self.rx, &mut buf).await?;

            if buf.len() == response.size_total as usize || response.bytes_read == 0 {
                trace!(message = "Finshed reading binary response", len = buf.len());
                break;
            }
        }
        Ok(buf)
    }

    #[tracing::instrument(skip(self), fields(command = ?String::from_utf8_lossy(command)))]
    async fn execute_ok(&mut self, command: &[u8]) -> MpdResult<()> {
        self.tx.write_all(&[command, b"\n"].concat()).await?;
        match EmptyMpdResponse::is_ok(&mut self.rx).await {
            Ok(_) => Ok(()),
            Err(MpdError::ClientClosed) if self.reconnect => {
                self.reconnect().await?;
                self.tx.write_all(&[command, b"\n"].concat()).await?;
                EmptyMpdResponse::is_ok(&mut self.rx).await
            }
            Err(e) => Err(e),
        }
    }

    #[tracing::instrument(skip(self), fields(command = ?String::from_utf8_lossy(command)))]
    async fn execute<T>(&mut self, command: &[u8]) -> MpdResult<Option<T>>
    where
        T: std::str::FromStr + std::fmt::Debug,
        <T as std::str::FromStr>::Err: std::fmt::Debug,
    {
        self.tx.write_all(&[command, b"\n"].concat()).await?;
        match MpdResponse::from_read(&mut self.rx).await {
            Ok(res) => Ok(res.body),
            Err(MpdError::ClientClosed) if self.reconnect => {
                self.reconnect().await?;
                self.tx.write_all(&[command, b"\n"].concat()).await?;
                Ok(MpdResponse::from_read(&mut self.rx).await?.body)
            }
            Err(e) => Err(e),
        }
    }
}
