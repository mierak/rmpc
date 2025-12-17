use std::{borrow::Cow, ops::Range};

use ratatui::{
    style::{Style, Stylize},
    text::Span,
};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use crate::ui::input::{InputEvent, InputResultEvent};

#[derive(Debug, Default, Clone)]
pub(super) struct InputBuffer {
    value: String,
    cursor: usize,
    visible_slice: Range<usize>,
    available_columns: usize,
}

#[derive(Default)]
pub(super) struct Grapheme {
    offset: usize,
    len: usize,
}

impl InputBuffer {
    pub(super) fn new(initial_value: Option<&str>) -> Self {
        Self {
            value: initial_value.unwrap_or_default().to_owned(),
            cursor: initial_value.map_or(0, |s| s.len()),
            visible_slice: 0..initial_value.map_or(0, |s| s.len()),
            available_columns: 0,
        }
    }

    pub(super) fn value(&self) -> &str {
        &self.value
    }

    pub(super) fn set_value(&mut self, new_value: String) {
        self.cursor = new_value.len();
        self.value = new_value;
    }

    pub(super) fn clear(&mut self) {
        self.value.clear();
        self.cursor = 0;
    }

    pub fn as_spans(
        &mut self,
        prefix: Option<&'static str>,
        available_width: impl Into<usize>,
        style: Style,
        is_active: bool,
    ) -> Vec<Span<'static>> {
        let value = &self.value;
        let value_len = value.len();
        let cursor = self.cursor;
        let mut visible_slice = self.visible_slice.clone();

        self.available_columns = available_width.into();

        // make space for the prefix and the block symbol if active
        let mut space_left = self
            .available_columns
            .saturating_sub(is_active as usize)
            .saturating_sub(prefix.map_or(0, |p| p.width() + 1 /* +1 for space after */));

        if space_left == 0 {
            self.visible_slice = 0..0;
            return Vec::new();
        }

        if !visible_slice.contains(&cursor) {
            // Resize or initial render happened, simply snapping to end is fine as this
            // should not happen very often and a "jump" here is ok.
            visible_slice.end = cursor;
            visible_slice.start = cursor;

            let graphemes = value.grapheme_indices(true).rev();
            for (i, g) in graphemes {
                let width = g.width();
                if width <= space_left {
                    space_left = space_left.saturating_sub(width);
                    visible_slice.start = i;
                } else {
                    break;
                }
            }
        }

        let mut current_string = String::new();
        let mut result = vec![];
        if let Some(p) = prefix {
            result.push(Span::styled(p, style));
            result.push(Span::styled(" ", style));
        }
        for (idx, g) in value
            .grapheme_indices(true)
            .skip_while(|(i, _)| *i < visible_slice.start)
            .take_while(|(i, _)| *i < visible_slice.end)
        {
            if idx == cursor {
                if !current_string.is_empty() {
                    result.push(Span::styled(std::mem::take(&mut current_string), style));
                }
                result.push(Span::styled(Cow::Owned(g.to_owned()), style).reversed());
            } else {
                current_string.push_str(g);
            }
        }

        if !current_string.is_empty() {
            result.push(Span::styled(current_string, style));
        }

        if is_active {
            result.push(Span::styled(
                "█",
                if cursor == value_len { style } else { style.reversed() },
            ));
        }

        self.visible_slice = visible_slice;

        return result;
    }

    pub fn handle_input(&mut self, ev: Option<InputEvent>) -> InputResultEvent {
        let old_cursor = self.cursor;
        let result = match ev {
            Some(InputEvent::Push(c)) => {
                let g = self.current_grapheme();
                if g.len > 0 && g.offset < self.cursor && self.cursor < g.offset + g.len {
                    self.cursor = g.offset;
                }

                self.value.insert(self.cursor, c);
                self.cursor += c.len_utf8();

                InputResultEvent::Push
            }
            // Delete
            Some(InputEvent::PopLeft) => {
                if self.cursor == 0 {
                    return InputResultEvent::NoChange;
                }

                let grapheme = self.current_grapheme();
                self.value.drain(grapheme.offset..grapheme.offset + grapheme.len);
                self.cursor = grapheme.offset;

                InputResultEvent::Pop
            }
            Some(InputEvent::PopRight) => {
                if self.cursor == self.value.len() {
                    return InputResultEvent::NoChange;
                }

                let grapheme = self.next_grapheme();
                self.value.drain(grapheme.offset..grapheme.offset + grapheme.len);

                InputResultEvent::Pop
            }
            Some(InputEvent::PopWordLeft) => {
                if self.cursor == 0 {
                    return InputResultEvent::NoChange;
                }

                let deletion_start = self
                    .value
                    .unicode_word_indices()
                    .find(|(idx, w)| {
                        // -1 so that the if cursor is at the start of a word, the word itself is
                        // not counted and instead the word before is considered
                        (*idx..*idx + w.len()).contains(&self.cursor.saturating_sub(1))
                    })
                    .map(|(idx, _)| idx)
                    .or_else(|| {
                        self.value
                            .unicode_word_indices()
                            .take_while(|(idx, _)| *idx < self.cursor)
                            .last()
                            .map(|(idx, _)| idx)
                    })
                    .unwrap_or(0);

                if deletion_start >= self.cursor {
                    return InputResultEvent::NoChange;
                }

                self.value.drain(deletion_start..self.cursor);
                self.cursor = deletion_start;

                InputResultEvent::Pop
            }
            Some(InputEvent::PopWordRight) => {
                if self.cursor >= self.value.len() {
                    return InputResultEvent::NoChange;
                }

                let bytes_to_drain = self
                    .value
                    .unicode_word_indices()
                    .find(|(idx, w)| (*idx..*idx + w.len()).contains(&self.cursor))
                    .map(|(idx, w)| w.len().saturating_sub(self.cursor.saturating_sub(idx)))
                    .or_else(|| {
                        self.value
                            .unicode_word_indices()
                            .find(|(idx, _)| idx > &self.cursor)
                            .map(|(idx, w)| idx.saturating_sub(self.cursor) + w.len())
                    })
                    .unwrap_or_else(|| self.value.len().saturating_sub(self.cursor));

                self.value.drain(self.cursor..self.cursor + bytes_to_drain);

                InputResultEvent::Pop
            }
            Some(InputEvent::DeleteToStart) => {
                if self.cursor == 0 {
                    return InputResultEvent::NoChange;
                }
                self.value.drain(0..self.cursor);
                self.cursor = 0;

                InputResultEvent::Pop
            }
            Some(InputEvent::DeleteToEnd) => {
                if self.cursor == self.value.len() {
                    return InputResultEvent::NoChange;
                }
                let grapheme = self.next_grapheme();
                self.value.drain(grapheme.offset..);
                self.cursor = self.value.len();

                InputResultEvent::Pop
            }

            // Movement
            Some(InputEvent::Back) => {
                self.cursor = self.current_grapheme().offset;
                InputResultEvent::NoChange
            }
            Some(InputEvent::Forward) => {
                let g = self.next_grapheme();
                self.cursor = (g.offset + g.len).min(self.value.len());
                InputResultEvent::NoChange
            }
            Some(InputEvent::Start) => {
                self.cursor = 0;
                InputResultEvent::NoChange
            }
            Some(InputEvent::End) => {
                self.cursor = self.value.len();
                InputResultEvent::NoChange
            }
            Some(InputEvent::BackWord) => {
                let prev = self.prev_word_boundary();
                self.cursor = prev.max(0);
                InputResultEvent::NoChange
            }
            Some(InputEvent::ForwardWord) => {
                let next = self.next_word_boundary();
                self.cursor = next.min(self.value.len());
                InputResultEvent::NoChange
            }
            None => InputResultEvent::NoChange,
        };

        // If cursor moved outside the current window, recompute the window based on
        // existing width
        if !self.visible_slice.contains(&self.cursor) {
            if old_cursor > self.cursor {
                let start = self.cursor;
                let mut end = start;
                let mut remaining = self.available_columns;

                for (i, g) in self.value.grapheme_indices(true).skip_while(|(i, _)| *i < start) {
                    let w = g.width();
                    if w > remaining {
                        break;
                    }
                    remaining -= w;
                    end = i + g.len();
                }
                self.visible_slice = start..end.min(self.value.len());
            } else {
                let mut end = self.cursor;
                if let Some((i, g)) =
                    self.value.grapheme_indices(true).take_while(|(i, _)| *i <= self.cursor).last()
                {
                    end = i + g.len();
                }
                let mut start = end;
                let mut remaining = self.available_columns;
                for (i, g) in self.value.grapheme_indices(true).rev().skip_while(|(i, _)| *i >= end)
                {
                    let w = g.width();
                    if w > remaining {
                        break;
                    }
                    remaining -= w;
                    start = i;
                }
                self.visible_slice = start.min(self.value.len())..end.min(self.value.len());
            }
        }

        result
    }

    #[inline]
    pub fn next_word_boundary(&self) -> usize {
        self.value
            .unicode_word_indices()
            .find(|(idx, _)| idx > &self.cursor)
            .map_or(self.value.len(), |(idx, _)| idx)
    }

    #[inline]
    pub fn prev_word_boundary(&self) -> usize {
        self.value
            .unicode_word_indices()
            .take_while(|(idx, _)| idx < &self.cursor)
            .last()
            .map_or(0, |(idx, _)| idx)
    }

    #[inline]
    pub fn current_grapheme(&self) -> Grapheme {
        self.value
            .grapheme_indices(true)
            .take_while(|(idx, _)| idx < &self.cursor)
            .last()
            .map_or(Grapheme::default(), |(idx, g)| Grapheme { offset: idx, len: g.len() })
    }

    #[inline]
    pub fn next_grapheme(&self) -> Grapheme {
        self.value
            .grapheme_indices(true)
            .take_while(|(idx, _)| idx <= &self.cursor)
            .last()
            .map_or(Grapheme::default(), |(idx, g)| Grapheme { offset: idx, len: g.len() })
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn make_input(s: &str, pos: usize) -> InputBuffer {
        InputBuffer { value: s.to_owned(), cursor: pos, ..Default::default() }
    }

    mod pop_left {
        use super::*;

        #[test]
        fn pop_left_at_start_no_change() {
            let mut input = make_input("", 0);
            let res = input.handle_input(Some(InputEvent::PopLeft));
            match res {
                InputResultEvent::NoChange => {}
                _ => panic!("Expected NoChange"),
            }

            let mut input = make_input("hello", 0);
            let res = input.handle_input(Some(InputEvent::PopLeft));
            match res {
                InputResultEvent::NoChange => {}
                _ => panic!("Expected NoChange"),
            }
        }

        #[test]
        fn pop_left_simple_ascii_deletes_prev_char() {
            let mut input = make_input("hello", 3); // hel|lo
            let res = input.handle_input(Some(InputEvent::PopLeft));
            match res {
                InputResultEvent::Pop => {
                    assert_eq!(input.value, "helo");
                    assert_eq!(input.cursor, 2);
                }
                _ => panic!("Expected Pop"),
            }
        }

        #[test]
        fn pop_left_multiple_times_until_empty() {
            let mut input = make_input("ab", 2);

            let r1 = input.handle_input(Some(InputEvent::PopLeft));
            match r1 {
                InputResultEvent::Pop => {
                    assert_eq!(input.value, "a");
                    assert_eq!(input.cursor, 1);
                }
                _ => panic!("Expected Pop"),
            }

            let r2 = input.handle_input(Some(InputEvent::PopLeft));
            match r2 {
                InputResultEvent::Pop => {
                    assert_eq!(input.value, "");
                    assert_eq!(input.cursor, 0);
                }
                _ => panic!("Expected Pop"),
            }

            let r3 = input.handle_input(Some(InputEvent::PopLeft));
            match r3 {
                InputResultEvent::NoChange => {}
                _ => panic!("Expected NoChange once empty and at start"),
            }
        }

        #[test]
        fn pop_left_unicode_combining_cluster_is_atomic() {
            let s = "y̆es";
            let pos = "y̆".len();
            let mut input = make_input(s, pos);
            let res = input.handle_input(Some(InputEvent::PopLeft));
            match res {
                InputResultEvent::Pop => {
                    assert_eq!(input.value, "es"); // whole cluster removed
                    assert_eq!(input.cursor, 0);
                }
                _ => panic!("Expected Pop"),
            }
        }

        #[test]
        fn pop_left_emoji_with_skin_tone_is_atomic() {
            let s = "ok 👍🏼 done";
            let pos = "ok 👍🏼".len(); // cursor right after emoji
            let mut input = make_input(s, pos);
            let res = input.handle_input(Some(InputEvent::PopLeft));
            match res {
                InputResultEvent::Pop => {
                    assert_eq!(input.value, "ok  done");
                    assert_eq!(input.cursor, "ok ".len());
                }
                _ => panic!("Expected Pop"),
            }
        }

        #[test]
        fn pop_left_punctuation_boundary() {
            let mut input = make_input("foo, bar", 4); // after "foo,"
            let res = input.handle_input(Some(InputEvent::PopLeft));
            match res {
                InputResultEvent::Pop => {
                    assert_eq!(input.value, "foo bar");
                    assert_eq!(input.cursor, 3);
                }
                _ => panic!("Expected Pop"),
            }
        }

        #[test]
        fn pop_left_inside_word_middle_char() {
            let mut input = make_input("abcde", 3); // ab|cde
            let res = input.handle_input(Some(InputEvent::PopLeft));
            match res {
                InputResultEvent::Pop => {
                    assert_eq!(input.value, "abde");
                    assert_eq!(input.cursor, 2);
                }
                _ => panic!("Expected Pop"),
            }
        }

        #[test]
        fn pop_left_on_whitespace_removes_one_space_grapheme() {
            let mut input = make_input("hello   world", 7); // hello  | world
            let res = input.handle_input(Some(InputEvent::PopLeft));
            match res {
                InputResultEvent::Pop => {
                    assert_eq!(input.value, "hello  world"); // removed one space
                    assert_eq!(input.cursor, 6);
                }
                _ => panic!("Expected Pop"),
            }
        }
    }

    mod pop_right {
        use super::*;

        #[test]
        fn pop_right_at_end_no_change() {
            let s = "hello";
            let mut input = make_input(s, s.len());
            let res = input.handle_input(Some(InputEvent::PopRight));
            match res {
                InputResultEvent::NoChange => {}
                _ => panic!("Expected NoChange"),
            }
        }

        #[test]
        fn pop_right_on_empty_no_change() {
            let mut input = make_input("", 0);
            let res = input.handle_input(Some(InputEvent::PopRight));
            match res {
                InputResultEvent::NoChange => {}
                _ => panic!("Expected NoChange"),
            }
        }

        #[test]
        fn pop_right_at_start() {
            let mut input = make_input("hello", 0);
            let res = input.handle_input(Some(InputEvent::PopRight));
            match res {
                InputResultEvent::Pop => {
                    assert_eq!(input.value, "ello");
                    assert_eq!(input.cursor, 0);
                }
                _ => panic!("Expected NoChange"),
            }
        }

        #[test]
        fn pop_right_inside_word_deletes_current_grapheme() {
            // Cursor inside 'c' grapheme position: after 'ab'
            let mut input = make_input("abcde", 2); // ab|cde
            let res = input.handle_input(Some(InputEvent::PopRight));
            match res {
                InputResultEvent::Pop => {
                    assert_eq!(input.value, "abde");
                    assert_eq!(input.cursor, 2);
                }
                _ => panic!("Expected Pop"),
            }
        }

        #[test]
        fn pop_right_at_grapheme_boundary_deletes_next_grapheme() {
            let mut input = make_input("hello", 2); // he|llo
            let res = input.handle_input(Some(InputEvent::PopRight));
            match res {
                InputResultEvent::Pop => {
                    assert_eq!(input.value, "helo"); // removed 'l'
                    assert_eq!(input.cursor, 2);
                }
                _ => panic!("Expected Pop"),
            }
        }

        #[test]
        fn pop_right_unicode_combining_cluster_is_atomic() {
            let s = "y̆es";
            let mut input = make_input(s, "y̆".len() - 1); // Inside the cluster bytes
            let res = input.handle_input(Some(InputEvent::PopRight));
            match res {
                InputResultEvent::Pop => {
                    assert_eq!(input.value, "es");
                    assert_eq!(input.cursor, "y̆".len() - 1);
                }
                _ => panic!("Expected Pop"),
            }
        }

        #[test]
        fn pop_right_emoji_with_skin_tone_is_atomic() {
            let s = "👍🏼 done";
            let mut input = make_input(s, 1); // inside emoji bytes; simulate being within cluster
            let res = input.handle_input(Some(InputEvent::PopRight));
            match res {
                InputResultEvent::Pop => {
                    assert!(input.value == " done");
                    assert_eq!(input.cursor, 1);
                }
                _ => panic!("Expected Pop"),
            }
        }

        #[test]
        fn pop_right_on_whitespace_deletes_space_grapheme() {
            let mut input = make_input("hello   world", 5); // hello|   world
            let res = input.handle_input(Some(InputEvent::PopRight));
            match res {
                InputResultEvent::Pop => {
                    assert_eq!(input.value, "hello  world"); // one space removed
                    assert_eq!(input.cursor, 5);
                }
                _ => panic!("Expected Pop"),
            }
        }

        #[test]
        fn pop_right_on_punctuation_deletes_punctuation() {
            let mut input = make_input("foo, bar", 3); // foo|, bar
            let res = input.handle_input(Some(InputEvent::PopRight));
            match res {
                InputResultEvent::Pop => {
                    assert_eq!(input.value, "foo bar"); // comma removed
                    assert_eq!(input.cursor, 3);
                }
                _ => panic!("Expected Pop"),
            }
        }

        #[test]
        fn pop_right_multiple_calls_progressively_delete() {
            let mut input = make_input("abc", 1); // a|bc

            let r1 = input.handle_input(Some(InputEvent::PopRight)); // delete 'b'
            match r1 {
                InputResultEvent::Pop => {
                    assert_eq!(input.value, "ac");
                    assert_eq!(input.cursor, 1);
                }
                _ => panic!("Expected Pop"),
            }
            let r2 = input.handle_input(Some(InputEvent::PopRight)); // delete 'c'
            match r2 {
                InputResultEvent::Pop => {
                    assert_eq!(input.value, "a");
                    assert_eq!(input.cursor, 1);
                }
                _ => panic!("Expected Pop"),
            }
            let r3 = input.handle_input(Some(InputEvent::PopRight)); // at end -> NoChange
            match r3 {
                InputResultEvent::NoChange => {}
                _ => panic!("Expected NoChange"),
            }
        }
    }

    mod pop_word_right {
        use super::*;

        #[test]
        fn pop_word_right_inside_word_deletes_to_word_end() {
            let mut input = make_input("hello world", 2); // cursor inside "hello"
            let ev = Some(InputEvent::PopWordRight);
            let res = input.handle_input(ev);

            match res {
                InputResultEvent::Pop => {
                    assert_eq!(input.value, "he world");
                    assert_eq!(input.cursor, 2);
                }
                _ => panic!("Expected Pop event"),
            }
        }

        #[test]
        fn pop_word_right_on_whitespace_deletes_next_word() {
            let mut input = make_input("hello   world", 6); // cursor on whitespace before "world"
            let ev = Some(InputEvent::PopWordRight);
            let res = input.handle_input(ev);

            match res {
                InputResultEvent::Pop => {
                    assert_eq!(input.value, "hello ");
                    assert_eq!(input.cursor, 6);
                }
                _ => panic!("Expected Pop event"),
            }
        }

        #[test]
        fn pop_word_right_at_word_boundary_deletes_next_word() {
            let mut input = make_input("hello world test", 5); // cursor at end of "hello"
            let ev = Some(InputEvent::PopWordRight);
            let res = input.handle_input(ev);

            match res {
                InputResultEvent::Pop => {
                    assert_eq!(input.value, "hello test");
                    assert_eq!(input.cursor, 5);
                }
                _ => panic!("Expected Pop event"),
            }
        }

        #[test]
        fn pop_word_right_at_end_no_change() {
            let s = "hello";
            let mut input = make_input(s, s.len()); // cursor at end
            let ev = Some(InputEvent::PopWordRight);
            let res = input.handle_input(ev);

            match res {
                InputResultEvent::NoChange => {}
                _ => panic!("Expected NoChange"),
            }
        }

        #[test]
        fn pop_word_right_only_whitespace_deletes_to_end() {
            let mut input = make_input("   ", 1); // cursor on whitespace
            let ev = Some(InputEvent::PopWordRight);
            let res = input.handle_input(ev);

            match res {
                InputResultEvent::Pop => {
                    assert_eq!(input.value, " ");
                    assert_eq!(input.cursor, 1);
                }
                _ => panic!("Expected Pop event"),
            }
        }

        #[test]
        fn pop_word_right_unicode_combining_and_emojis() {
            let mut input = make_input("y̆es 👍🏼 done", 0);
            let ev = Some(InputEvent::PopWordRight);
            let res = input.handle_input(ev);

            match res {
                InputResultEvent::Pop => {
                    assert!(input.value.starts_with(" 👍🏼 done"));
                    assert_eq!(input.cursor, 0);
                }
                _ => panic!("Expected Pop event"),
            }
        }

        #[test]
        fn pop_word_right_between_words_deletes_next_word() {
            let mut input = make_input("foo, bar!", 4); // cursor after "foo" and comma (likely punctuation boundary)
            let ev = Some(InputEvent::PopWordRight);
            let res = input.handle_input(ev);

            match res {
                InputResultEvent::Pop => {
                    assert!(input.value == "foo,!");
                    assert_eq!(input.cursor, 4);
                }
                _ => panic!("Expected Pop event"),
            }
        }
    }

    mod pop_word_left {
        use super::*;
        #[test]
        fn pop_word_left_inside_word_deletes_to_word_start() {
            let mut input = make_input("hello world", 3); // cursor inside "hello" after 'l'
            let ev = Some(InputEvent::PopWordLeft);
            let res = input.handle_input(ev);

            match res {
                InputResultEvent::Pop => {
                    assert_eq!(input.value, "lo world");
                    assert_eq!(input.cursor, 0);
                }
                _ => panic!("Expected Pop event"),
            }
        }

        #[test]
        fn pop_word_left_on_whitespace_deletes_prev_word() {
            let mut input = make_input("hello   world", 6); // cursor on whitespace after "hello"
            let ev = Some(InputEvent::PopWordLeft);
            let res = input.handle_input(ev);

            match res {
                InputResultEvent::Pop => {
                    assert_eq!(input.value, "  world");
                    assert_eq!(input.cursor, 0);
                }
                _ => panic!("Expected Pop event"),
            }
        }

        #[test]
        fn pop_word_left_at_word_boundary_deletes_prev_word() {
            let mut input = make_input("hello world test", 12); // cursor right after space following "hello"
            let ev = Some(InputEvent::PopWordLeft);
            let res = input.handle_input(ev);

            match res {
                InputResultEvent::Pop => {
                    assert_eq!(input.value, "hello test");
                    assert_eq!(input.cursor, 6);
                }
                _ => panic!("Expected Pop event"),
            }
        }

        #[test]
        fn pop_word_left_at_start_no_change() {
            let mut input = make_input("hello", 0); // cursor at start
            let ev = Some(InputEvent::PopWordLeft);
            let res = input.handle_input(ev);

            match res {
                InputResultEvent::NoChange => {}
                _ => panic!("Expected NoChange"),
            }
        }

        #[test]
        fn pop_word_left_only_whitespace_deletes_to_start() {
            let mut input = make_input("   ", 2); // cursor on whitespace
            let ev = Some(InputEvent::PopWordLeft);
            let res = input.handle_input(ev);

            match res {
                InputResultEvent::Pop => {
                    assert_eq!(input.value, " ");
                    assert_eq!(input.cursor, 0);
                }
                _ => panic!("Expected Pop event"),
            }
        }

        #[test]
        fn pop_word_left_unicode_combining_and_emojis() {
            let s = "y̆es 👍🏼 done";
            let cursor = "y̆es ".len(); // position right after first word and space
            let mut input = make_input(s, cursor);
            let ev = Some(InputEvent::PopWordLeft);
            let res = input.handle_input(ev);

            match res {
                InputResultEvent::Pop => {
                    assert!(input.value.starts_with("👍🏼 done"));
                    assert_eq!(input.cursor, 0);
                }
                _ => panic!("Expected Pop event"),
            }
        }

        #[test]
        fn pop_word_left_between_words_deletes_prev_word() {
            let mut input = make_input("foo, bar!", 4); // cursor after "foo,"
            let ev = Some(InputEvent::PopWordLeft);
            let res = input.handle_input(ev);

            match res {
                InputResultEvent::Pop => {
                    assert_eq!(input.value, " bar!");
                    assert_eq!(input.cursor, 0);
                }
                _ => panic!("Expected Pop event"),
            }
        }
    }

    mod delete_to_start {
        use super::*;

        #[test]
        fn delete_to_start_on_empty_no_change() {
            let mut input = make_input("", 0);
            let res = input.handle_input(Some(InputEvent::DeleteToStart));
            match res {
                InputResultEvent::NoChange => {}
                _ => panic!("Expected Pop with empty string unchanged"),
            }
        }

        #[test]
        fn delete_to_start_at_start_no_change() {
            let mut input = make_input("hello", 0);
            let res = input.handle_input(Some(InputEvent::DeleteToStart));
            match res {
                InputResultEvent::NoChange => {}
                _ => panic!("Expected Pop"),
            }
        }

        #[test]
        fn delete_to_start_mid_ascii_deletes_prefix() {
            let mut input = make_input("hello world", 6); // "hello " | "world"
            let res = input.handle_input(Some(InputEvent::DeleteToStart));
            match res {
                InputResultEvent::Pop => {
                    assert_eq!(input.value, "world");
                    assert_eq!(input.cursor, 0);
                }
                _ => panic!("Expected Pop"),
            }
        }

        #[test]
        fn delete_to_start_unicode_combining_cluster_partial_prefix() {
            // y̆ is a single grapheme made of 'y' + combining diacritic
            let s = "y̆es test";
            let pos = "y̆es".len(); // after first word
            let mut input = make_input(s, pos);
            let res = input.handle_input(Some(InputEvent::DeleteToStart));
            match res {
                InputResultEvent::Pop => {
                    assert_eq!(input.value, " test");
                    assert_eq!(input.cursor, 0);
                }
                _ => panic!("Expected Pop"),
            }
        }

        #[test]
        fn delete_to_start_emoji_prefix() {
            let s = "👍🏼 ok";
            let pos = s.len(); // end
            let mut input = make_input(s, pos);
            let res = input.handle_input(Some(InputEvent::DeleteToStart));
            match res {
                InputResultEvent::Pop => {
                    assert_eq!(input.value, "");
                    assert_eq!(input.cursor, 0);
                }
                _ => panic!("Expected Pop"),
            }
        }

        #[test]
        fn delete_to_start_on_whitespace_prefix() {
            let mut input = make_input("   abc", 3); // after spaces
            let res = input.handle_input(Some(InputEvent::DeleteToStart));
            match res {
                InputResultEvent::Pop => {
                    assert_eq!(input.value, "abc");
                    assert_eq!(input.cursor, 0);
                }
                _ => panic!("Expected Pop"),
            }
        }
    }

    mod delete_to_end {
        use super::*;

        #[test]
        fn delete_to_end_on_empty_no_change() {
            let mut input = make_input("", 0);
            let res = input.handle_input(Some(InputEvent::DeleteToEnd));
            match res {
                InputResultEvent::NoChange => {}
                _ => panic!("Expected Pop with empty string unchanged"),
            }
        }

        #[test]
        fn delete_to_end_at_start_deletes_all() {
            let mut input = make_input("hello", 0);
            let res = input.handle_input(Some(InputEvent::DeleteToEnd));
            match res {
                InputResultEvent::Pop => {
                    assert_eq!(input.value, "");
                    assert_eq!(input.cursor, 0);
                }
                _ => panic!("Expected Pop"),
            }
        }

        #[test]
        fn delete_to_end_mid_ascii_deletes_from_current_grapheme_start_to_end() {
            let mut input = make_input("hello world", 7); // "hello w|orld"
            let res = input.handle_input(Some(InputEvent::DeleteToEnd));
            match res {
                InputResultEvent::Pop => {
                    assert_eq!(input.value, "hello w");
                    assert_eq!(input.cursor, "hello w".len());
                }
                _ => panic!("Expected Pop"),
            }
        }

        #[test]
        fn delete_to_end_inside_grapheme_deletes_from_cluster_start() {
            let s = "y̆es done";
            let mut input = make_input(s, 1);
            let res = input.handle_input(Some(InputEvent::DeleteToEnd));
            match res {
                InputResultEvent::Pop => {
                    assert_eq!(input.value, "");
                    assert_eq!(input.cursor, 0);
                }
                _ => panic!("Expected Pop"),
            }
        }

        #[test]
        fn delete_to_end_on_whitespace_deletes_trailing_content() {
            let mut input = make_input("hello   world", 5); // "hello|   world"
            let res = input.handle_input(Some(InputEvent::DeleteToEnd));
            match res {
                InputResultEvent::Pop => {
                    assert_eq!(input.value, "hello");
                    assert_eq!(input.cursor, "hello".len());
                }
                _ => panic!("Expected Pop"),
            }
        }

        #[test]
        fn delete_to_end_at_end_deletes_last_grapheme_due_to_current_behavior() {
            let s = "hello";
            let mut input = make_input(s, s.len()); // cursor at end
            let res = input.handle_input(Some(InputEvent::DeleteToEnd));
            match res {
                InputResultEvent::NoChange => {}
                _ => panic!("Expected Pop"),
            }
        }

        #[test]
        fn delete_to_end_emoji_cluster_behavior() {
            let s = "ok 👍🏼 done";
            let mut input = make_input(s, "ok ".len());
            let res = input.handle_input(Some(InputEvent::DeleteToEnd));
            match res {
                InputResultEvent::Pop => {
                    assert_eq!(input.value, "ok ");
                    assert_eq!(input.cursor, "ok ".len());
                }
                _ => panic!("Expected Pop"),
            }
        }
    }

    mod move_left_right {
        use super::*;

        #[test]
        fn left_at_start_no_change() {
            let mut input = make_input("hello", 0);
            let res = input.handle_input(Some(InputEvent::Back));
            match res {
                InputResultEvent::NoChange => {}
                _ => panic!("Expected NoChange"),
            }
            assert_eq!(input.cursor, 0);
        }

        #[test]
        fn right_at_end_no_change() {
            let s = "hello";
            let mut input = make_input(s, s.len());
            let res = input.handle_input(Some(InputEvent::Forward));
            match res {
                InputResultEvent::NoChange => {}
                _ => panic!("Expected NoChange"),
            }
            assert_eq!(input.cursor, s.len());
        }

        #[test]
        fn left_moves_one_grapheme_ascii() {
            let mut input = make_input("hello", 3); // hel|lo
            let res = input.handle_input(Some(InputEvent::Back));
            match res {
                InputResultEvent::NoChange => {}
                _ => panic!("Expected NoChange"),
            }
            assert_eq!(input.cursor, 2);
        }

        #[test]
        fn right_moves_one_grapheme_ascii() {
            let mut input = make_input("hello", 2); // he|llo
            let res = input.handle_input(Some(InputEvent::Forward));
            match res {
                InputResultEvent::NoChange => {}
                _ => panic!("Expected NoChange"),
            }
            assert_eq!(input.cursor, 3);
        }

        #[test]
        fn left_moves_one_grapheme_combining_cluster() {
            let s = "y̆es";
            let pos = s.len();
            let mut input = make_input(s, pos);
            let _ = input.handle_input(Some(InputEvent::Back));
            assert_eq!(input.cursor, "y̆e".len());
            let _ = input.handle_input(Some(InputEvent::Back));
            assert_eq!(input.cursor, "y̆".len());
            let _ = input.handle_input(Some(InputEvent::Back));
            assert_eq!(input.cursor, 0);
            let _ = input.handle_input(Some(InputEvent::Back));
            assert_eq!(input.cursor, 0);
        }

        #[test]
        fn right_moves_one_grapheme_combining_cluster() {
            let s = "y̆es";
            let mut input = make_input(s, 0);
            let _ = input.handle_input(Some(InputEvent::Forward));
            assert_eq!(input.cursor, "y̆".len());
            let _ = input.handle_input(Some(InputEvent::Forward));
            assert_eq!(input.cursor, "y̆e".len());
            let _ = input.handle_input(Some(InputEvent::Forward));
            assert_eq!(input.cursor, s.len());
            let _ = input.handle_input(Some(InputEvent::Forward));
            assert_eq!(input.cursor, s.len());
        }

        #[test]
        fn left_right_with_emoji_cluster() {
            let s = "a👍🏼b";
            let mut input = make_input(s, s.len());
            let _ = input.handle_input(Some(InputEvent::Back));
            let after_emoji_pos = "a👍🏼".len();
            assert_eq!(input.cursor, after_emoji_pos);
            let _ = input.handle_input(Some(InputEvent::Back));
            assert_eq!(input.cursor, "a".len());
            let _ = input.handle_input(Some(InputEvent::Forward));
            assert_eq!(input.cursor, after_emoji_pos);
            let _ = input.handle_input(Some(InputEvent::Forward));
            assert_eq!(input.cursor, s.len());
        }

        #[test]
        fn left_from_mid_grapheme_moves_by_left_grapheme_len() {
            let s = "y̆es";
            let pos_inside_cluster = "y̆".len() - 1;
            let mut input = make_input(s, pos_inside_cluster);
            let _ = input.handle_input(Some(InputEvent::Back));
            assert_eq!(input.cursor, 0);
        }
    }

    mod move_word_left_right {
        use super::*;

        #[test]
        fn right_word_from_start_moves_to_first_word_start_after_cursor() {
            let mut input = make_input("hello  world", 0);
            let res = input.handle_input(Some(InputEvent::ForwardWord));
            match res {
                InputResultEvent::NoChange => {}
                _ => panic!("Expected NoChange"),
            }
            assert_eq!(input.cursor, "hello  ".len());
        }

        #[test]
        fn right_word_skips_whitespace_and_punctuation() {
            let mut input = make_input("foo,  bar!", 0);
            let _ = input.handle_input(Some(InputEvent::ForwardWord));
            assert_eq!(input.cursor, "foo,  ".len());
            let _ = input.handle_input(Some(InputEvent::ForwardWord));
            assert_eq!(input.cursor, "foo,  bar!".len());
        }

        #[test]
        fn right_word_at_end_stays_at_end() {
            let s = "hello";
            let mut input = make_input(s, s.len());
            let res = input.handle_input(Some(InputEvent::ForwardWord));
            match res {
                InputResultEvent::NoChange => {}
                _ => panic!("Expected NoChange"),
            }
            assert_eq!(input.cursor, s.len());
        }

        #[test]
        fn left_word_from_middle_moves_to_prev_word_start() {
            let mut input = make_input("hello   world test", 14);
            let res = input.handle_input(Some(InputEvent::BackWord));
            match res {
                InputResultEvent::NoChange => {}
                _ => panic!("Expected NoChange"),
            }
            assert_eq!(input.cursor, "hello   ".len());
        }

        #[test]
        fn left_word_from_whitespace_moves_to_prev_word_start() {
            let mut input = make_input("hello   world", 6);
            let res = input.handle_input(Some(InputEvent::BackWord));
            match res {
                InputResultEvent::NoChange => {}
                _ => panic!("Expected NoChange"),
            }
            assert_eq!(input.cursor, 0);
        }

        #[test]
        fn left_word_at_start_stays() {
            let mut input = make_input("hello world", 0);
            let res = input.handle_input(Some(InputEvent::BackWord));
            match res {
                InputResultEvent::NoChange => {}
                _ => panic!("Expected NoChange"),
            }
            assert_eq!(input.cursor, 0);
        }

        #[test]
        fn right_word_handles_unicode_combining_and_emojis() {
            let s = "y̆es 👍🏼 done";
            let mut input = make_input(s, 0);
            let _ = input.handle_input(Some(InputEvent::ForwardWord));
            assert!(input.cursor >= "y̆es".len());
        }

        #[test]
        fn left_word_handles_unicode_combining_and_emojis() {
            let s = "y̆es 👍🏼 done";
            let mut input = make_input(s, s.len());
            let _ = input.handle_input(Some(InputEvent::BackWord));
            assert_eq!(input.cursor, "y̆es 👍🏼 ".len());
            let _ = input.handle_input(Some(InputEvent::BackWord));
            assert_eq!(input.cursor, 0);
        }
    }

    mod move_start_end {
        use super::*;

        #[test]
        fn start_on_empty_stays_at_zero() {
            let mut input = make_input("", 0);
            let res = input.handle_input(Some(InputEvent::Start));
            match res {
                InputResultEvent::NoChange => {}
                _ => panic!("Expected NoChange"),
            }
            assert_eq!(input.cursor, 0);
        }

        #[test]
        fn end_on_empty_stays_at_zero() {
            let mut input = make_input("", 0);
            let res = input.handle_input(Some(InputEvent::End));
            match res {
                InputResultEvent::NoChange => {}
                _ => panic!("Expected NoChange"),
            }
            assert_eq!(input.cursor, 0);
        }

        #[test]
        fn start_moves_to_zero_from_middle_ascii() {
            let mut input = make_input("hello world", 6);
            let res = input.handle_input(Some(InputEvent::Start));
            match res {
                InputResultEvent::NoChange => {}
                _ => panic!("Expected NoChange"),
            }
            assert_eq!(input.cursor, 0);
        }

        #[test]
        fn end_moves_to_len_from_middle_ascii() {
            let s = "hello world";
            let mut input = make_input(s, 5);
            let res = input.handle_input(Some(InputEvent::End));
            match res {
                InputResultEvent::NoChange => {}
                _ => panic!("Expected NoChange"),
            }
            assert_eq!(input.cursor, s.len());
        }

        #[test]
        fn start_from_end_moves_to_zero() {
            let s = "hello";
            let mut input = make_input(s, s.len());
            let res = input.handle_input(Some(InputEvent::Start));
            match res {
                InputResultEvent::NoChange => {}
                _ => panic!("Expected NoChange"),
            }
            assert_eq!(input.cursor, 0);
        }

        #[test]
        fn end_from_start_moves_to_len() {
            let s = "hello";
            let mut input = make_input(s, 0);
            let res = input.handle_input(Some(InputEvent::End));
            match res {
                InputResultEvent::NoChange => {}
                _ => panic!("Expected NoChange"),
            }
            assert_eq!(input.cursor, s.len());
        }

        #[test]
        fn start_with_unicode_combining_cluster() {
            let s = "y̆es 👍🏼 done";
            let mut input = make_input(s, s.len()); // at end
            let res = input.handle_input(Some(InputEvent::Start));
            match res {
                InputResultEvent::NoChange => {}
                _ => panic!("Expected NoChange"),
            }
            assert_eq!(input.cursor, 0);
        }

        #[test]
        fn end_with_unicode_combining_cluster() {
            let s = "y̆es 👍🏼 done";
            let mut input = make_input(s, 0); // at start
            let res = input.handle_input(Some(InputEvent::End));
            match res {
                InputResultEvent::NoChange => {}
                _ => panic!("Expected NoChange"),
            }
            assert_eq!(input.cursor, s.len());
        }

        #[test]
        fn start_then_end_roundtrip_positions() {
            let s = "abc👍🏼def";
            let mut input = make_input(s, 3);
            let _ = input.handle_input(Some(InputEvent::Start));
            assert_eq!(input.cursor, 0);
            let _ = input.handle_input(Some(InputEvent::End));
            assert_eq!(input.cursor, s.len());
        }

        #[test]
        fn end_then_start_roundtrip_positions() {
            let s = "abc👍🏼def";
            let mut input = make_input(s, 0);
            let _ = input.handle_input(Some(InputEvent::End));
            assert_eq!(input.cursor, s.len());
            let _ = input.handle_input(Some(InputEvent::Start));
            assert_eq!(input.cursor, 0);
        }
    }
}
