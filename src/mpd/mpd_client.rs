use anyhow::Result;
use async_trait::async_trait;

use super::{
    client::Client,
    commands::{
        list::MpdList, list_playlist::FileList, status::OnOffOneshot, volume::Bound, IdleEvent, ListFiles, LsInfo,
        Playlist, Song, Status, Volume,
    },
    errors::{ErrorCode, MpdError, MpdFailureResponse},
};

type MpdResult<T> = Result<T, MpdError>;

#[async_trait]
pub trait MpdClient {
    async fn idle(&mut self) -> MpdResult<Vec<IdleEvent>>;
    async fn get_volume(&mut self) -> MpdResult<Volume>;
    async fn set_volume(&mut self, volume: &Volume) -> MpdResult<()>;
    async fn get_current_song(&mut self) -> MpdResult<Option<Song>>;
    async fn get_status(&mut self) -> MpdResult<Status>;
    // Playback control
    async fn pause_toggle(&mut self) -> MpdResult<()>;
    async fn next(&mut self) -> MpdResult<()>;
    async fn prev(&mut self) -> MpdResult<()>;
    async fn play_pos(&mut self, pos: u32) -> MpdResult<()>;
    async fn play(&mut self) -> MpdResult<()>;
    async fn play_id(&mut self, id: u32) -> MpdResult<()>;
    async fn stop(&mut self) -> MpdResult<()>;
    async fn seek_curr_forwards(&mut self, time_sec: u32) -> MpdResult<()>;
    async fn seek_curr_backwards(&mut self, time_sec: u32) -> MpdResult<()>;
    async fn repeat(&mut self, enabled: bool) -> MpdResult<()>;
    async fn random(&mut self, enabled: bool) -> MpdResult<()>;
    async fn single(&mut self, single: OnOffOneshot) -> MpdResult<()>;
    async fn consume(&mut self, consume: OnOffOneshot) -> MpdResult<()>;
    // Current queue
    async fn add(&mut self, path: &str) -> MpdResult<()>;
    async fn clear(&mut self) -> MpdResult<()>;
    async fn delete_id(&mut self, id: u32) -> MpdResult<()>;
    async fn playlist_info(&mut self) -> MpdResult<Option<Vec<Song>>>;
    async fn find(&mut self, filter: &[Filter<'_>]) -> MpdResult<Vec<Song>>;
    async fn find_add(&mut self, filter: &[Filter<'_>]) -> MpdResult<()>;
    async fn list_tag(&mut self, tag: &str, filter: Option<&[Filter<'_>]>) -> MpdResult<MpdList>;
    // Database
    async fn lsinfo(&mut self, path: Option<&str>) -> MpdResult<LsInfo>;
    async fn list_files(&mut self, path: Option<&str>) -> MpdResult<ListFiles>;
    async fn read_picture(&mut self, path: &str) -> MpdResult<Vec<u8>>;
    async fn albumart(&mut self, path: &str) -> MpdResult<Vec<u8>>;
    // Stored playlists
    async fn list_playlists(&mut self) -> MpdResult<Vec<Playlist>>;
    async fn list_playlist(&mut self, name: &str) -> MpdResult<FileList>;
    async fn list_playlist_info(&mut self, playlist: &str) -> MpdResult<Vec<Song>>;
    /// This function first invokes [albumart].
    /// If no album art is fonud it invokes [readpicture].
    /// If no art is still found, but no errors were encountered, None is returned.
    async fn find_album_art(&mut self, path: &str) -> MpdResult<Option<Vec<u8>>>;
}

#[async_trait]
impl MpdClient for Client<'_> {
    // Queries
    #[tracing::instrument(skip(self))]
    async fn idle(&mut self) -> MpdResult<Vec<IdleEvent>> {
        self.execute("idle").await
    }

    #[tracing::instrument(skip(self))]
    async fn get_volume(&mut self) -> MpdResult<Volume> {
        self.execute("getvol").await
    }

    #[tracing::instrument(skip(self))]
    async fn set_volume(&mut self, volume: &Volume) -> MpdResult<()> {
        self.execute_ok(&format!("setvol {}", volume.value())).await
    }

    #[tracing::instrument(skip(self))]
    async fn get_current_song(&mut self) -> MpdResult<Option<Song>> {
        self.execute_option("currentsong").await
    }

    #[tracing::instrument(skip(self))]
    async fn get_status(&mut self) -> MpdResult<Status> {
        self.execute("status").await
    }

    // Playback control
    #[tracing::instrument(skip(self))]
    async fn pause_toggle(&mut self) -> MpdResult<()> {
        self.execute_ok("pause").await
    }

    #[tracing::instrument(skip(self))]
    async fn next(&mut self) -> MpdResult<()> {
        self.execute_ok("next").await
    }

    #[tracing::instrument(skip(self))]
    async fn prev(&mut self) -> MpdResult<()> {
        self.execute_ok("previous").await
    }

    #[tracing::instrument(skip(self))]
    async fn play_pos(&mut self, pos: u32) -> MpdResult<()> {
        self.execute_ok(&format!("play {pos}")).await
    }

    #[tracing::instrument(skip(self))]
    async fn play(&mut self) -> MpdResult<()> {
        self.execute_ok("play").await
    }

    #[tracing::instrument(skip(self))]
    async fn play_id(&mut self, id: u32) -> MpdResult<()> {
        self.execute_ok(&format!("playid {id}")).await
    }

    #[tracing::instrument(skip(self))]
    async fn stop(&mut self) -> MpdResult<()> {
        self.execute_ok("stop").await
    }

    #[tracing::instrument(skip(self))]
    async fn seek_curr_forwards(&mut self, time_sec: u32) -> MpdResult<()> {
        self.execute_ok(&format!("seekcur +{time_sec}")).await
    }

    #[tracing::instrument(skip(self))]
    async fn seek_curr_backwards(&mut self, time_sec: u32) -> MpdResult<()> {
        self.execute_ok(&format!("seekcur -{time_sec}")).await
    }

    #[tracing::instrument(skip(self))]
    async fn repeat(&mut self, enabled: bool) -> MpdResult<()> {
        self.execute_ok(&format!("repeat {}", u8::from(enabled))).await
    }

    #[tracing::instrument(skip(self))]
    async fn random(&mut self, enabled: bool) -> MpdResult<()> {
        self.execute_ok(&format!("random {}", u8::from(enabled))).await
    }

    #[tracing::instrument(skip(self))]
    async fn single(&mut self, single: OnOffOneshot) -> MpdResult<()> {
        self.execute_ok(&format!("single {}", single.to_mpd_value())).await
    }

    #[tracing::instrument(skip(self))]
    async fn consume(&mut self, consume: OnOffOneshot) -> MpdResult<()> {
        self.execute_ok(&format!("consume {}", consume.to_mpd_value())).await
    }

    // Current queue
    #[tracing::instrument(skip(self))]
    async fn add(&mut self, path: &str) -> MpdResult<()> {
        self.execute_ok(&format!("add \"{path}\"")).await
    }

    #[tracing::instrument(skip(self))]
    async fn clear(&mut self) -> MpdResult<()> {
        self.execute_ok("clear").await
    }

    #[tracing::instrument(skip(self))]
    async fn delete_id(&mut self, id: u32) -> MpdResult<()> {
        self.execute_ok(&format!("deleteid \"{id}\"")).await
    }

    #[tracing::instrument(skip(self))]
    async fn playlist_info(&mut self) -> MpdResult<Option<Vec<Song>>> {
        self.execute_option("playlistinfo").await
    }

    #[tracing::instrument(skip(self))]
    async fn find(&mut self, filter: &[Filter<'_>]) -> MpdResult<Vec<Song>> {
        self.execute(&format!("find \"({})\"", filter.to_query_str())).await
    }

    #[tracing::instrument(skip(self))]
    async fn find_add(&mut self, filter: &[Filter<'_>]) -> MpdResult<()> {
        self.execute_ok(&format!("findadd \"({})\"", filter.to_query_str()))
            .await
    }

    #[tracing::instrument(skip(self))]
    async fn list_tag(&mut self, tag: &str, filter: Option<&[Filter<'_>]>) -> MpdResult<MpdList> {
        match filter {
            Some(filter) => {
                self.execute(&format!("list {tag} \"({})\"", filter.to_query_str()))
                    .await
            }
            None => self.execute(&format!("list {tag}")).await,
        }
    }

    // Database
    #[tracing::instrument(skip(self))]
    async fn lsinfo(&mut self, path: Option<&str>) -> MpdResult<LsInfo> {
        if let Some(path) = path {
            Ok(self
                .execute_option(&format!("lsinfo \"{path}\""))
                .await?
                .unwrap_or(LsInfo::default()))
        } else {
            Ok(self.execute_option("lsinfo").await?.unwrap_or(LsInfo::default()))
        }
    }

    #[tracing::instrument(skip(self))]
    async fn list_files(&mut self, path: Option<&str>) -> MpdResult<ListFiles> {
        if let Some(path) = path {
            Ok(self
                .execute_option(&format!("listfiles \"{path}\""))
                .await?
                .unwrap_or(ListFiles::default()))
        } else {
            Ok(self.execute_option("listfiles").await?.unwrap_or(ListFiles::default()))
        }
    }

    // Stored playlists
    async fn list_playlists(&mut self) -> MpdResult<Vec<Playlist>> {
        self.execute("listplaylists").await
    }
    async fn list_playlist(&mut self, name: &str) -> MpdResult<FileList> {
        self.execute(&format!("listplaylist \"{name}\"")).await
    }
    async fn list_playlist_info(&mut self, playlist: &str) -> MpdResult<Vec<Song>> {
        self.execute(&format!("listplaylistinfo \"{playlist}\"")).await
    }

    #[tracing::instrument(skip(self))]
    async fn read_picture(&mut self, path: &str) -> MpdResult<Vec<u8>> {
        self.execute_binary(&format!("readpicture \"{path}\"")).await
    }

    #[tracing::instrument(skip(self))]
    async fn albumart(&mut self, path: &str) -> MpdResult<Vec<u8>> {
        self.execute_binary(&format!("albumart \"{path}\"")).await
    }

    #[tracing::instrument(skip(self))]
    async fn find_album_art(&mut self, path: &str) -> MpdResult<Option<Vec<u8>>> {
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
}

trait StrExt {
    fn escape(self) -> String;
}
impl StrExt for &str {
    fn escape(self) -> String {
        self.replace('\\', "\\\\")
            .replace('(', "\\(")
            .replace(')', "\\)")
            .replace('\'', "\\\\'")
            .replace('\"', "\\\"")
    }
}

#[derive(Debug)]
pub struct Filter<'a> {
    pub tag: &'a str,
    pub value: &'a str,
}
trait FilterExt {
    fn to_query_str(&self) -> String;
}
impl FilterExt for &[Filter<'_>] {
    fn to_query_str(&self) -> String {
        self.iter()
            .enumerate()
            .fold(String::new(), |mut acc, (idx, Filter { tag, value })| {
                if idx > 0 {
                    acc.push_str(&format!(" AND ({tag} == '{}')", value.escape()));
                } else {
                    acc.push_str(&format!("({tag} == '{}')", value.escape()));
                }
                acc
            })
    }
}

#[cfg(test)]
mod strext_tests {
    use crate::mpd::mpd_client::StrExt;

    #[test]
    fn escapes_correctly() {
        let input: &'static str = r#"(Artist == "foo'bar")"#;

        assert_eq!(input.escape(), r#"\(Artist == \"foo\\'bar\"\)"#);
    }
}

#[cfg(test)]
mod filter_tests {
    use crate::mpd::mpd_client::FilterExt;

    use super::Filter;

    #[test]
    fn single_value() {
        let input: &[Filter<'_>] = &[Filter {
            tag: "artist",
            value: "mrs singer",
        }];

        assert_eq!(input.to_query_str(), "(artist == 'mrs singer')");
    }

    #[test]
    fn multiple_values() {
        let input: &[Filter<'_>] = &[
            Filter {
                tag: "album",
                value: "the greatest",
            },
            Filter {
                tag: "artist",
                value: "mrs singer",
            },
        ];

        assert_eq!(
            input.to_query_str(),
            "(album == 'the greatest') AND (artist == 'mrs singer')"
        );
    }
}
