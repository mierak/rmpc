use crate::mpd::{errors::MpdError, FromMpd, LineHandled};

use super::Song;
use anyhow::anyhow;
use anyhow::Context;
use derive_more::{AsMut, AsRef, Into, IntoIterator};

#[derive(Debug, Default, IntoIterator, AsRef, AsMut, Into)]
pub struct LsInfo(pub Vec<LsInfoEntry>);

#[derive(Debug, PartialEq, Eq)]
pub enum LsInfoEntry {
    Dir(Dir),
    File(Song),
    Playlist(Playlist),
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Dir {
    /// Last segment of the part, the dir name
    pub path: String,
    /// this is the full path from mpd root
    pub full_path: String,
    pub last_modified: String,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Playlist {
    name: String,
}

impl FromMpd for Dir {
    fn next_internal(&mut self, key: &str, value: String) -> Result<LineHandled, MpdError> {
        match key {
            "directory" => {
                value
                    .split('/')
                    .last()
                    .context(anyhow!("Failed to parse dir name. Key: '{}' Value: '{}'", key, value))?
                    .clone_into(&mut self.path);
                self.full_path = value;
            }
            "last-modified" => self.last_modified = value,
            "playlist" => {} // ignore, deprecated
            _ => return Ok(LineHandled::No { value }),
        }
        Ok(LineHandled::Yes)
    }
}

impl FromMpd for Playlist {
    fn next_internal(&mut self, key: &str, value: String) -> Result<LineHandled, MpdError> {
        match key {
            "playlist" => self.name = value,
            _ => return Ok(LineHandled::No { value }),
        }
        Ok(LineHandled::Yes)
    }
}

impl FromMpd for LsInfo {
    fn next_internal(&mut self, key: &str, value: String) -> Result<LineHandled, MpdError> {
        if key == "file" {
            self.0.push(LsInfoEntry::File(Song::default()));
        }
        if key == "directory" {
            self.0.push(LsInfoEntry::Dir(Dir::default()));
        }
        if key == "playlist" {
            self.0.push(LsInfoEntry::Playlist(Playlist::default()));
        }

        match self.0.last_mut().context(anyhow!(
            "No element in accumulator while parsing LsInfo. Key '{}' Value :'{}'",
            key,
            value
        ))? {
            LsInfoEntry::Dir(dir) => dir.next_internal(key, value),
            LsInfoEntry::File(song) => song.next_internal(key, value),
            LsInfoEntry::Playlist(playlist) => playlist.next_internal(key, value),
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use crate::mpd::commands::lsinfo::{Dir, LsInfoEntry, Playlist};

    use super::{FromMpd, LsInfo};

    #[test]
    fn lsinfoplaylist() {
        let input = r"playlist: autechre.m3u
Last-Modified: 2024-10-30T00:04:26Z
directory: .cue
Last-Modified: 2024-11-02T02:55:40Z
directory: .win
Last-Modified: 2024-09-15T19:39:47Z
directory: flac
Last-Modified: 2024-12-23T00:11:38Z
directory: wav
Last-Modified: 2024-08-12T03:03:40Z";

        let mut result = LsInfo::default();
        for line in input.lines() {
            let (key, value) = line.split_once(": ").unwrap();
            result
                .next_internal(key.to_lowercase().as_str(), value.to_owned())
                .unwrap();
        }

        let result = result.0;
        assert_eq!(result.len(), 5);
        assert_eq!(
            result[0],
            LsInfoEntry::Playlist(Playlist {
                name: "autechre.m3u".to_owned(),
            })
        );
        assert_eq!(
            result[1],
            LsInfoEntry::Dir(Dir {
                path: ".cue".to_owned(),
                full_path: ".cue".to_owned(),
                last_modified: "2024-11-02T02:55:40Z".to_owned()
            })
        );
        assert_eq!(
            result[2],
            LsInfoEntry::Dir(Dir {
                path: ".win".to_owned(),
                full_path: ".win".to_owned(),
                last_modified: "2024-09-15T19:39:47Z".to_owned()
            })
        );
        assert_eq!(
            result[3],
            LsInfoEntry::Dir(Dir {
                path: "flac".to_owned(),
                full_path: "flac".to_owned(),
                last_modified: "2024-12-23T00:11:38Z".to_owned()
            })
        );
        assert_eq!(
            result[4],
            LsInfoEntry::Dir(Dir {
                path: "wav".to_owned(),
                full_path: "wav".to_owned(),
                last_modified: "2024-08-12T03:03:40Z".to_owned()
            })
        );
    }
}
