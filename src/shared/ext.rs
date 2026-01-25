pub mod span {
    use std::borrow::Cow;

    use ratatui::text::Span;
    use unicode_segmentation::UnicodeSegmentation;

    pub trait SpanExt {
        /// Truncate the end of the span's content to the specified number of
        /// characters. Returns how many characters were consumed form
        /// the specified remaining length.
        fn truncate_end(&mut self, remaining_length: usize) -> usize;

        /// Truncate the start of the span's content to the specified number of
        /// characters. Returns how many characters were consumed form
        /// the specified remaining length.
        fn truncate_start(&mut self, remaining_length: usize) -> usize;
    }

    impl SpanExt for String {
        fn truncate_end(&mut self, remaining_length: usize) -> usize {
            if remaining_length == 0 {
                self.clear();
                return 0;
            }

            if let Some((idx, s)) =
                self.grapheme_indices(true).nth(remaining_length.saturating_sub(1))
            {
                self.drain(idx + s.len()..);
            }
            remaining_length
        }

        fn truncate_start(&mut self, remaining_length: usize) -> usize {
            if remaining_length == 0 {
                self.clear();
                return 0;
            }

            if let Some((idx, _)) =
                self.grapheme_indices(true).rev().nth(remaining_length.saturating_sub(1))
            {
                self.drain(0..idx);
            }
            remaining_length
        }
    }

    impl SpanExt for Span<'_> {
        fn truncate_end(&mut self, remaining_length: usize) -> usize {
            let chars = self.content.graphemes(true).count();
            if chars <= remaining_length {
                return chars;
            }

            if remaining_length == 0 {
                self.content = Cow::Borrowed("");
                return 0;
            }

            match &mut self.content {
                Cow::Borrowed(content) => {
                    let mut strbuf = String::new();
                    for (i, c) in content.graphemes(true).enumerate() {
                        if i >= remaining_length {
                            break;
                        }
                        strbuf.push_str(c);
                    }

                    self.content = strbuf.into();
                    remaining_length
                }
                cow @ Cow::Owned(_) => {
                    cow.to_mut().truncate_end(remaining_length);
                    remaining_length
                }
            }
        }

        fn truncate_start(&mut self, remaining_length: usize) -> usize {
            let chars = self.content.graphemes(true).count();
            if chars <= remaining_length {
                return chars;
            }

            if remaining_length == 0 {
                self.content = Cow::Borrowed("");
                return 0;
            }

            match &mut self.content {
                Cow::Borrowed(content) => {
                    let mut strbuf = String::new();
                    for (i, c) in content.graphemes(true).rev().enumerate() {
                        if i >= remaining_length {
                            break;
                        }
                        strbuf.insert_str(0, c);
                    }

                    self.content = strbuf.into();
                    remaining_length
                }
                cow @ Cow::Owned(_) => {
                    cow.to_mut().truncate_start(remaining_length);
                    remaining_length
                }
            }
        }
    }

    #[cfg(test)]
    mod test {
        use std::borrow::Cow;

        use ratatui::{
            style::{Color, Style},
            text::Span,
        };

        use super::SpanExt;

        #[test]
        fn with_borrowed_content() {
            let content = Cow::Borrowed("Hello, world!");
            let mut input = Span::styled(content, Style::default().fg(Color::Red));

            let result = input.truncate_start(5);

            assert_eq!(result, 5);
            assert_eq!(input.content, "orld!");
            assert_eq!(input.style.fg, Some(Color::Red));
        }

        #[test]
        fn with_owned_content() {
            let content = Cow::Owned("Hello, world!".into());
            let mut input = Span::styled(content, Style::default().fg(Color::Red));

            let result = input.truncate_start(5);

            assert_eq!(result, 5);
            assert_eq!(input.content, "orld!");
            assert_eq!(input.style.fg, Some(Color::Red));
        }

        #[test]
        fn shorter_than_remaining() {
            let content = Cow::Owned("Hello, world!".into());
            let mut input = Span::styled(content, Style::default().fg(Color::Red));

            let result = input.truncate_start(99);

            assert_eq!(result, 13);
            assert_eq!(input.content, "Hello, world!");
            assert_eq!(input.style.fg, Some(Color::Red));
        }

        #[test]
        fn remaining_zero() {
            let content = Cow::Owned("Hello, world!".into());
            let mut input = Span::styled(content, Style::default().fg(Color::Red));

            let result = input.truncate_start(0);

            assert_eq!(result, 0);
            assert_eq!(input.content, "");
            assert_eq!(input.style.fg, Some(Color::Red));
        }
    }
}

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
                MpdError::UnknownCode(e) => format!("Unknown code: {e}"),
                MpdError::Generic(e) => format!("Generic error: {e}"),
                MpdError::ClientClosed => "Client closed".to_string(),
                MpdError::Mpd(e) => format!("MPD Error: {e}"),
                MpdError::ValueExpected(e) => {
                    format!("Expected Value but got '{e}'")
                }
                MpdError::UnsupportedMpdVersion(e) => {
                    format!("Unsupported MPD version: {e}")
                }
                MpdError::TimedOut(_) => "Request to MPD timed out".to_string(),
            }
        }
    }
}

pub mod duration {
    const SECONDS_IN_DAY: u64 = 60 * 60 * 24;
    const SECONDS_IN_HOUR: u64 = 60 * 60;
    const SECONDS_IN_MINUTE: u64 = 60;

    pub trait DurationExt {
        fn to_string(&self) -> String;
        fn format_to_duration(&self, unit_separator: &str) -> String;
    }

    impl DurationExt for std::time::Duration {
        fn to_string(&self) -> String {
            let secs = self.as_secs();

            let min = secs / 60;
            let frac_secs = secs - min * 60;

            let hours = min / 60;
            let frac_min = min - hours * 60;

            let days = hours / 24;
            let frac_hours = hours - days * 24;

            if hours == 0 {
                format!("{min}:{frac_secs:0>2}")
            } else if days == 0 {
                format!("{hours}:{frac_min:0>2}:{frac_secs:0>2}")
            } else {
                format!("{days}d {frac_hours:0>2}:{frac_min:0>2}:{frac_secs:0>2}")
            }
        }

        fn format_to_duration(&self, unit_separator: &str) -> String {
            let mut total_seconds = self.as_secs();
            if total_seconds == 0 {
                return "0s".to_string();
            }

            let mut buf = String::new();
            if total_seconds >= SECONDS_IN_DAY {
                let days = total_seconds / SECONDS_IN_DAY;
                total_seconds = total_seconds.saturating_sub(days * SECONDS_IN_DAY);
                buf.push_str(&days.to_string());
                buf.push('d');
                if total_seconds > 0 {
                    buf.push_str(unit_separator);
                }
            }

            if total_seconds >= SECONDS_IN_HOUR {
                let hours = total_seconds / SECONDS_IN_HOUR;
                total_seconds = total_seconds.saturating_sub(hours * SECONDS_IN_HOUR);
                buf.push_str(&hours.to_string());

                buf.push('h');
                if total_seconds > 0 {
                    buf.push_str(unit_separator);
                }
            }

            if total_seconds >= SECONDS_IN_MINUTE {
                let minutes = total_seconds / SECONDS_IN_MINUTE;
                total_seconds = total_seconds.saturating_sub(minutes * SECONDS_IN_MINUTE);
                buf.push_str(&minutes.to_string());

                buf.push('m');
                if total_seconds > 0 {
                    buf.push_str(unit_separator);
                }
            }

            if total_seconds > 0 {
                buf.push_str(&total_seconds.to_string());
                buf.push('s');
            }

            buf
        }
    }

    #[cfg(test)]
    mod test {
        use std::time::Duration;

        use test_case::test_case;

        use super::*;

        #[test_case(Duration::from_secs(0), "0:00")]
        #[test_case(Duration::from_secs(1), "0:01")]
        #[test_case(Duration::from_secs(30), "0:30")]
        #[test_case(Duration::from_secs(60), "1:00")]
        #[test_case(Duration::from_secs(95), "1:35")]
        #[test_case(Duration::from_secs(123), "2:03")]
        #[test_case(Duration::from_secs(3599), "59:59")]
        #[test_case(Duration::from_secs(3600), "1:00:00")]
        #[test_case(Duration::from_secs(3601), "1:00:01")]
        #[test_case(Duration::from_secs(3661), "1:01:01")]
        #[test_case(Duration::from_secs(7200), "2:00:00")]
        #[test_case(Duration::from_secs(86399), "23:59:59")]
        #[test_case(Duration::from_secs(86400), "1d 00:00:00")]
        #[test_case(Duration::from_secs(90061), "1d 01:01:01")]
        #[test_case(Duration::from_secs(99999), "1d 03:46:39")]
        #[test_case(Duration::from_secs(172_800), "2d 00:00:00")]
        fn duration_to_string(input: Duration, expected: &str) {
            assert_eq!(input.to_string(), expected);
        }

        #[test_case(Duration::from_secs(0), "0s")]
        #[test_case(Duration::from_secs(1), "1s")]
        #[test_case(Duration::from_secs(60), "1m")]
        #[test_case(Duration::from_secs(95), "1m, 35s")]
        #[test_case(Duration::from_secs(3600), "1h")]
        #[test_case(Duration::from_secs(3601), "1h, 1s")]
        #[test_case(Duration::from_secs(3661), "1h, 1m, 1s")]
        #[test_case(Duration::from_secs(3600 * 24), "1d")]
        #[test_case(Duration::from_secs(99999), "1d, 3h, 46m, 39s")]
        fn duration_format(input: Duration, expected: &str) {
            assert_eq!(input.format_to_duration(", "), expected);
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

        /// recv the last message in the channel in a non-blocking manner and
        /// drop all the other ones
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
            ZipLongest2 { iter_a: self.fuse(), iter_b: b.fuse(), iter_c: c.fuse() }
        }
    }
}

pub mod btreeset_ranges {
    use std::{
        collections::{BTreeSet, btree_set},
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
            Ranges { iter: self.iter(), current_range: None }
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

    #[allow(unused)]
    pub trait RectExt {
        fn shrink_from_top(self, amount: u16) -> Rect;
        fn shrink_horizontally(self, amount: u16) -> Rect;
        fn overlaps_in_y(&self, other: &Self) -> bool;
        fn overlaps_in_x(&self, other: &Self) -> bool;
    }

    impl RectExt for Rect {
        fn shrink_from_top(mut self, amount: u16) -> Rect {
            self.height = self.height.saturating_sub(amount);
            self.y = self.y.saturating_add(amount);
            self
        }

        fn shrink_horizontally(mut self, amount: u16) -> Rect {
            self.width = self.width.saturating_sub(amount * 2);
            self.x = self.x.saturating_add(amount);
            self
        }

        fn overlaps_in_y(&self, other: &Self) -> bool {
            !(self.bottom() <= other.top() || self.top() >= other.bottom())
        }

        fn overlaps_in_x(&self, other: &Self) -> bool {
            !(self.right() <= other.left() || self.left() >= other.right())
        }
    }

    #[cfg(test)]
    mod tests {
        use ratatui::layout::Rect;
        use test_case::test_case;

        use crate::shared::ext::rect::RectExt;

        #[test_case(Rect::new(0, 0, 5, 1), Rect::new(5, 0, 5, 1), false; "self on the left, no overlap")]
        #[test_case(Rect::new(0, 0, 6, 1), Rect::new(5, 0, 5, 1), true; "self on the left, overlap")]
        #[test_case(Rect::new(10, 0, 5, 1), Rect::new(5, 0, 5, 1), false; "self on the right, no overlap")]
        #[test_case(Rect::new(10, 0, 6, 1), Rect::new(5, 0, 6, 1), true; "self on the right, overlap")]
        #[test_case(Rect::new(0, 0, 5, 5), Rect::new(0, 0, 5, 5), true; "perfect overlap")]
        fn overlap_x(a: Rect, b: Rect, expected_overlap: bool) {
            assert_eq!(a.overlaps_in_x(&b), expected_overlap);
        }

        #[test_case(Rect::new(0, 0, 5, 5), Rect::new(0, 5, 5, 5), false; "self above, no overlap")]
        #[test_case(Rect::new(0, 0, 5, 6), Rect::new(0, 5, 5, 5), true; "self above, overlap")]
        #[test_case(Rect::new(0, 10, 5, 5), Rect::new(0, 5, 5, 5), false; "self below, no overlap")]
        #[test_case(Rect::new(0, 10, 5, 5), Rect::new(0, 5, 5, 6), true; "self below, overlap")]
        #[test_case(Rect::new(0, 0, 5, 5), Rect::new(0, 0, 5, 5), true; "perfect overlap")]
        fn overlap_y(a: Rect, b: Rect, expected_overlap: bool) {
            assert_eq!(a.overlaps_in_y(&b), expected_overlap);
        }
    }
}

pub mod vec {
    pub trait VecExt<T> {
        fn or_else_if_empty(self, cb: impl Fn() -> Vec<T>) -> Vec<T>;
        fn get_or_last(&self, idx: usize) -> Option<&T>;
    }

    impl<T> VecExt<T> for Vec<T> {
        fn or_else_if_empty(self, cb: impl Fn() -> Vec<T>) -> Vec<T> {
            if self.is_empty() { cb() } else { self }
        }

        fn get_or_last(&self, idx: usize) -> Option<&T> {
            self.get(idx).or_else(|| self.last())
        }
    }
}

pub mod num {
    pub trait NumExt {
        fn with_thousands_separator(self, separator: &str) -> String;
    }

    impl NumExt for usize {
        fn with_thousands_separator(self, separator: &str) -> String {
            let mut buf = String::new();
            for (idx, c) in self.to_string().chars().rev().enumerate() {
                if idx % 3 == 0 && idx != 0 {
                    buf.insert_str(0, separator);
                }
                buf.insert(0, c);
            }
            buf
        }
    }

    #[cfg(test)]
    mod test {
        use test_case::test_case;

        use super::*;

        #[test_case(123_456_789, "123,456,789")]
        #[test_case(789, "789")]
        #[test_case(6789, "6,789")]
        #[test_case(1, "1")]
        #[test_case(0, "0")]
        #[test_case(4_294_967_295, "4,294,967,295")] // equivalent to u32::MAX, as not to break 32 bit architectures
        fn usize_format(input: usize, expected: &str) {
            let result = input.with_thousands_separator(",");

            assert_eq!(result, expected);
        }
    }
}
