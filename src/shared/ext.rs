pub mod error {
    use itertools::Itertools;

    use crate::mpd::errors::MpdError;

    pub trait ErrorExt {
        fn to_status(&self) -> String;
    }

    impl ErrorExt for anyhow::Error {
        fn to_status(&self) -> String {
            self.chain().map(|e| e.to_string().replace('\n', "")).join(" ")
        }
    }
    impl ErrorExt for MpdError {
        fn to_status(&self) -> String {
            match self {
                MpdError::Parse(e) => format!("Failed to parse: {e}"),
                MpdError::UnknownCode(e) => format!("Unkown code: {e}"),
                MpdError::Generic(e) => format!("Generic error: {e}"),
                MpdError::ClientClosed => "Client closed".to_string(),
                MpdError::Mpd(e) => format!("MPD Error: {e}"),
                MpdError::ValueExpected(e) => format!("Expected Value but got '{e}'"),
                MpdError::UnsupportedMpdVersion(e) => format!("Unsuported MPD version: {e}"),
            }
        }
    }
}

pub mod duration {
    pub trait DurationExt {
        fn to_string(&self) -> String;
    }

    impl DurationExt for std::time::Duration {
        fn to_string(&self) -> String {
            let secs = self.as_secs();
            let min = secs / 60;
            format!("{}:{:0>2}", min, secs - min * 60)
        }
    }
}

#[allow(unused)]
pub mod mpsc {
    use crossbeam::channel::{Receiver, RecvError, TryRecvError};

    pub trait RecvLast<T> {
        fn recv_last(&self) -> Result<T, RecvError>;
        fn try_recv_last(&self) -> Result<T, TryRecvError>;
    }

    impl<T> RecvLast<T> for Receiver<T> {
        /// recv the last message in the channel and drop all the other ones
        fn recv_last(&self) -> Result<T, RecvError> {
            self.recv().map(|data| {
                let mut result = data;
                while let Ok(newer_data) = self.try_recv() {
                    result = newer_data;
                }
                result
            })
        }

        /// recv the last message in the channel in a non-blocking manner and drop all the other ones
        fn try_recv_last(&self) -> Result<T, TryRecvError> {
            self.try_recv().map(|data| {
                let mut result = data;
                while let Ok(newer_data) = self.try_recv() {
                    result = newer_data;
                }
                result
            })
        }
    }
}

pub mod iter {
    use std::iter::Fuse;

    pub struct ZipLongest2<A, B, C>
    where
        A: Iterator,
        B: Iterator,
        C: Iterator,
    {
        iter_a: Fuse<A>,
        iter_b: Fuse<B>,
        iter_c: Fuse<C>,
    }

    impl<A, B, C> Iterator for ZipLongest2<A, B, C>
    where
        A: Iterator,
        B: Iterator,
        C: Iterator,
    {
        type Item = (
            Option<<A as Iterator>::Item>,
            Option<<B as Iterator>::Item>,
            Option<<C as Iterator>::Item>,
        );

        fn next(&mut self) -> Option<Self::Item> {
            match (self.iter_a.next(), self.iter_b.next(), self.iter_c.next()) {
                (None, None, None) => None,
                item => Some(item),
            }
        }
    }

    pub trait IntoZipLongest2: Iterator {
        fn zip_longest2<B: Iterator, C: Iterator>(self, b: B, c: C) -> ZipLongest2<Self, B, C>
        where
            Self: Sized;
    }

    impl<A: Iterator> IntoZipLongest2 for A {
        fn zip_longest2<B: Iterator, C: Iterator>(self, b: B, c: C) -> ZipLongest2<Self, B, C>
        where
            Self: Sized,
        {
            ZipLongest2 {
                iter_a: self.fuse(),
                iter_b: b.fuse(),
                iter_c: c.fuse(),
            }
        }
    }
}

pub mod mpd_client {
    use crate::mpd::{
        errors::{ErrorCode, MpdError, MpdFailureResponse},
        mpd_client::MpdClient,
    };

    pub trait MpdClientExt {
        fn play_last(&mut self, queue_len: usize) -> Result<(), MpdError>;
    }

    impl<T: MpdClient> MpdClientExt for T {
        fn play_last(&mut self, queue_len: usize) -> Result<(), MpdError> {
            match self.play_pos(queue_len) {
                Ok(()) => {}
                Err(MpdError::Mpd(MpdFailureResponse {
                    code: ErrorCode::Argument,
                    ..
                })) => {
                    // This can happen when multiple clients modify the queue at the same
                    // time. But a more robust solution would require refetching the whole
                    // queue and searching for the added song. This should be good enough.
                    log::warn!("Failed to autoplay song");
                }
                Err(err) => return Err(err),
            };
            Ok(())
        }
    }
}

pub mod btreeset_ranges {
    use std::{
        collections::{btree_set, BTreeSet},
        ops::{Range, RangeInclusive},
    };

    pub trait BTreeSetRanges<'a, T: 'a> {
        fn ranges(&'a self) -> Ranges<'a, T, std::collections::btree_set::Iter<'a, T>>;
    }

    pub struct Ranges<'a, T: 'a, I: Iterator<Item = &'a T>> {
        iter: I,
        current_range: Option<Range<T>>,
    }

    impl<'a, T: Default + 'a> BTreeSetRanges<'a, T> for BTreeSet<T> {
        fn ranges(&'a self) -> Ranges<'a, T, btree_set::Iter<'a, T>> {
            Ranges {
                iter: self.iter(),
                current_range: None,
            }
        }
    }

    impl<'a, I: DoubleEndedIterator<Item = &'a usize>> DoubleEndedIterator for Ranges<'a, usize, I> {
        fn next_back(&mut self) -> Option<Self::Item> {
            match (self.iter.next_back(), self.current_range.take()) {
                (Some(current), None) => {
                    self.current_range = Some(*current..*current);
                    self.next_back()
                }
                (None, Some(current_range)) => {
                    self.current_range = None;
                    Some(current_range.start..=current_range.end)
                }
                (Some(current), Some(mut current_range)) if *current == current_range.start - 1 => {
                    current_range.start = *current;
                    self.current_range = Some(current_range);
                    self.next_back()
                }
                (Some(current), Some(current_range)) => {
                    self.current_range = Some(*current..*current);
                    Some(current_range.start..=current_range.end)
                }
                (None, None) => None,
            }
        }
    }

    impl<'a, I: Iterator<Item = &'a usize>> Iterator for Ranges<'a, usize, I> {
        type Item = RangeInclusive<usize>;

        fn next(&mut self) -> Option<Self::Item> {
            match (self.iter.next(), self.current_range.take()) {
                (Some(current), None) => {
                    self.current_range = Some(*current..*current);
                    self.next()
                }
                (None, Some(current_range)) => {
                    self.current_range = None;
                    Some(current_range.start..=current_range.end)
                }
                (Some(current), Some(mut current_range)) if *current == current_range.end + 1 => {
                    current_range.end = *current;
                    self.current_range = Some(current_range);
                    self.next()
                }
                (Some(current), Some(current_range)) => {
                    self.current_range = Some(*current..*current);
                    Some(current_range.start..=current_range.end)
                }
                (None, None) => None,
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use std::collections::BTreeSet;

        use itertools::Itertools;

        use super::BTreeSetRanges;

        #[test]
        fn ranges() {
            let input: BTreeSet<usize> = [1, 2, 3, 6, 7, 12, 16, 17, 18, 19].into();

            let ranges = input.ranges().collect_vec();

            assert_eq!(ranges[0].clone().count(), 3);
            assert_eq!(ranges[1].clone().count(), 2);
            assert_eq!(ranges[2].clone().count(), 1);
            assert_eq!(ranges[3].clone().count(), 4);
            assert_eq!(ranges, vec![1..=3, 6..=7, 12..=12, 16..=19]);
        }

        #[test]
        fn ranges_rev() {
            let input: BTreeSet<usize> = [1, 2, 3, 6, 7, 12, 16, 17, 18, 19].into();

            let ranges = input.ranges().rev().collect_vec();

            dbg!(&ranges);
            assert_eq!(ranges[0].clone().count(), 4);
            assert_eq!(ranges[1].clone().count(), 1);
            assert_eq!(ranges[2].clone().count(), 2);
            assert_eq!(ranges[3].clone().count(), 3);
            assert_eq!(
                ranges,
                vec![1..=3, 6..=7, 12..=12, 16..=19].into_iter().rev().collect_vec()
            );
        }
    }
}

pub mod rect {
    use ratatui::layout::Rect;

    pub trait ShrinkExt {
        fn shrink_from_top(self, amount: u16) -> Rect;
    }

    impl ShrinkExt for Rect {
        fn shrink_from_top(mut self, amount: u16) -> Rect {
            self.height = self.height.saturating_sub(amount);
            self.y = self.y.saturating_add(amount);
            self
        }
    }
}
