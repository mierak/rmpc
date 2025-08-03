use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct Artists {
    pub album_display_mode: AlbumDisplayMode,
    pub album_sort_by: AlbumSortMode,
    pub album_date_tags: Vec<AlbumDateTag>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ArtistsFile {
    #[serde(default)]
    pub album_display_mode: AlbumDisplayMode,
    #[serde(default)]
    pub album_sort_by: AlbumSortMode,
    #[serde(default)]
    pub album_date_tags: Vec<AlbumDateTag>,
}

impl Default for ArtistsFile {
    fn default() -> Self {
        Self {
            album_display_mode: AlbumDisplayMode::default(),
            album_sort_by: AlbumSortMode::default(),
            album_date_tags: vec![AlbumDateTag::Date],
        }
    }
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

#[derive(
    Debug, Default, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, strum::IntoStaticStr,
)]
#[strum(serialize_all = "lowercase")]
pub enum AlbumDateTag {
    #[default]
    Date,
    OriginalDate,
}

impl Default for Artists {
    fn default() -> Self {
        ArtistsFile::default().into()
    }
}

impl From<ArtistsFile> for Artists {
    fn from(value: ArtistsFile) -> Self {
        Self {
            album_display_mode: value.album_display_mode,
            album_sort_by: value.album_sort_by,
            album_date_tags: value.album_date_tags,
        }
    }
}
