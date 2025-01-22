use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone)]
pub struct Artists {
    pub album_display_mode: AlbumDisplayMode,
    pub album_sort_by: AlbumSortMode,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ArtistsFile {
    #[serde(default)]
    pub album_display_mode: AlbumDisplayMode,
    #[serde(default)]
    pub album_sort_by: AlbumSortMode,
}

#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum AlbumDisplayMode {
    #[default]
    SplitByDate,
    NameOnly,
}

#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum AlbumSortMode {
    Name,
    #[default]
    Date,
}

impl From<ArtistsFile> for Artists {
    fn from(value: ArtistsFile) -> Self {
        Self { album_display_mode: value.album_display_mode, album_sort_by: value.album_sort_by }
    }
}
