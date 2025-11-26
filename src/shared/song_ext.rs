use std::ops::Range;

use crate::mpd::commands::Song;

pub trait SongsExt {
    fn to_album_ranges(self) -> impl Iterator<Item = Range<usize>>;
}

impl SongsExt for &[Song] {
    fn to_album_ranges(self) -> impl Iterator<Item = Range<usize>> {
        let mut i = 0;

        std::iter::from_fn(move || {
            if self.is_empty() || i >= self.len() {
                return None;
            }

            let a = self[i].metadata.get("album");
            let aa = self[i].metadata.get("album_artist");
            let mut j = i + 1;
            while j < self.len()
                && self[j].metadata.get("album") == a
                && self[j].metadata.get("album_artist") == aa
            {
                j += 1;
            }
            let range = i..j;
            i = j;

            Some(range)
        })
    }
}
