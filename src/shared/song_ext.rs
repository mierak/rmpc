use std::ops::Range;

use crate::mpd::commands::Song;

pub trait SongsExt {
    fn to_album_ranges(self) -> Vec<Range<usize>>;
}

impl SongsExt for &[Song] {
    fn to_album_ranges(self) -> Vec<Range<usize>> {
        let mut out = Vec::new();
        let mut i = 0;
        while i < self.len() {
            let a = self[i].metadata.get("album");
            let aa = self[i].metadata.get("album_artist");
            let mut j = i + 1;
            while j < self.len()
                && self[j].metadata.get("album") == a
                && self[j].metadata.get("album_artist") == aa
            {
                j += 1;
            }
            out.push(i..j);
            i = j;
        }
        out
    }
}
