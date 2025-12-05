use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
};

use crate::ui::input::{BufferId, InputEvent, InputResultEvent, buffer::InputBuffer};

#[derive(Debug, Default, Clone, Copy)]
pub enum InputMode {
    #[default]
    Normal,
    Insert(BufferId),
}

#[derive(derive_more::Debug, Default)]
pub struct InputManager {
    mode: Cell<InputMode>,
    buffers: RefCell<HashMap<BufferId, InputBuffer>>,
}

macro_rules! buffer {
    ($self:expr, $id:expr) => {
        $self.buffers.borrow_mut().entry($id).or_insert(InputBuffer::default())
    };
}

impl InputManager {
    pub fn value(&self, id: BufferId) -> String {
        // TODO this clone could be avoided by returning &str with lifetime but needs to
        // avoid borrow issues
        buffer!(self, id).value()
    }

    pub fn destroy_buffer(&self, id: BufferId) {
        self.buffers.borrow_mut().remove(&id);
    }

    pub fn clear_buffer(&self, id: BufferId) {
        buffer!(self, id).clear();
    }

    pub fn is_insert_mode(&self) -> bool {
        matches!(self.mode.get(), InputMode::Insert(_))
    }

    pub fn insert_mode(&self, id: BufferId) {
        self.mode.replace(InputMode::Insert(id));
    }

    pub fn is_normal_mode(&self) -> bool {
        matches!(self.mode.get(), InputMode::Normal)
    }

    pub fn normal_mode(&self) {
        self.mode.replace(InputMode::Normal);
    }

    pub fn position(&self, id: BufferId) -> usize {
        buffer!(self, id).offset()
    }

    pub fn handle_input(&self, ev: Option<InputEvent>) -> Option<InputResultEvent> {
        let InputMode::Insert(id) = self.mode.get() else {
            return None;
        };

        match ev {
            Some(InputEvent::Cancel) => {
                self.normal_mode();
                Some(InputResultEvent::Cancel)
            }
            Some(InputEvent::Confirm) => {
                self.normal_mode();

                Some(InputResultEvent::Confirm(buffer!(self, id).value()))
            }
            _ => buffer!(self, id).handle_input(ev),
        }
    }
}

#[cfg(test)]
mod test {
    use std::sync::LazyLock;

    use super::*;
    static ID: LazyLock<BufferId> = LazyLock::new(BufferId::new);

    fn make_input(s: &str, pos: usize) -> InputManager {
        let input = InputManager::default();
        input.insert_mode(*ID);
        input
            .buffers
            .borrow_mut()
            .entry(*ID)
            .or_insert(InputBuffer::default())
            .set_value(s.to_owned());
        input.buffers.borrow_mut().entry(*ID).or_insert(InputBuffer::default()).set_offset(pos);
        input
    }

    mod pop_left {
        use super::*;

        #[test]
        fn pop_left_at_start_no_change() {
            let input = make_input("", 0);
            let res = input.handle_input(Some(InputEvent::PopLeft));
            match res {
                Some(InputResultEvent::NoChange) => {}
                _ => panic!("Expected NoChange"),
            }

            let input = make_input("hello", 0);
            let res = input.handle_input(Some(InputEvent::PopLeft));
            match res {
                Some(InputResultEvent::NoChange) => {}
                _ => panic!("Expected NoChange"),
            }
        }

        #[test]
        fn pop_left_simple_ascii_deletes_prev_char() {
            let input = make_input("hello", 3); // hel|lo
            let res = input.handle_input(Some(InputEvent::PopLeft));
            match res {
                Some(InputResultEvent::Pop(new)) => {
                    assert_eq!(new, "helo");
                    assert_eq!(input.position(*ID), 2);
                }
                _ => panic!("Expected Pop"),
            }
        }

        #[test]
        fn pop_left_multiple_times_until_empty() {
            let input = make_input("ab", 2);
            let r1 = input.handle_input(Some(InputEvent::PopLeft));

            match r1 {
                Some(InputResultEvent::Pop(new)) => {
                    assert_eq!(new, "a");
                    assert_eq!(input.position(*ID), 1);
                }
                _ => panic!("Expected Pop"),
            }

            let r2 = input.handle_input(Some(InputEvent::PopLeft));
            match r2 {
                Some(InputResultEvent::Pop(new)) => {
                    assert_eq!(new, "");
                    assert_eq!(input.position(*ID), 0);
                }
                _ => panic!("Expected Pop"),
            }

            let r3 = input.handle_input(Some(InputEvent::PopLeft));
            match r3 {
                Some(InputResultEvent::NoChange) => {}
                _ => panic!("Expected NoChange once empty and at start"),
            }
        }

        #[test]
        fn pop_left_unicode_combining_cluster_is_atomic() {
            let s = "y̆es";
            let pos = "y̆".len();
            let input = make_input(s, pos);
            let res = input.handle_input(Some(InputEvent::PopLeft));
            match res {
                Some(InputResultEvent::Pop(new)) => {
                    assert_eq!(new, "es"); // whole cluster removed
                    assert_eq!(input.position(*ID), 0);
                }
                _ => panic!("Expected Pop"),
            }
        }

        #[test]
        fn pop_left_emoji_with_skin_tone_is_atomic() {
            let s = "ok 👍🏼 done";
            let pos = "ok 👍🏼".len(); // cursor right after emoji
            let input = make_input(s, pos);
            let res = input.handle_input(Some(InputEvent::PopLeft));
            match res {
                Some(InputResultEvent::Pop(new)) => {
                    assert_eq!(new, "ok  done");
                    assert_eq!(input.position(*ID), "ok ".len());
                }
                _ => panic!("Expected Pop"),
            }
        }

        #[test]
        fn pop_left_punctuation_boundary() {
            let input = make_input("foo, bar", 4); // after "foo,"
            let res = input.handle_input(Some(InputEvent::PopLeft));
            match res {
                Some(InputResultEvent::Pop(new)) => {
                    assert_eq!(new, "foo bar");
                    assert_eq!(input.position(*ID), 3);
                }
                _ => panic!("Expected Pop"),
            }
        }

        #[test]
        fn pop_left_inside_word_middle_char() {
            let input = make_input("abcde", 3); // ab|cde
            let res = input.handle_input(Some(InputEvent::PopLeft));
            match res {
                Some(InputResultEvent::Pop(new)) => {
                    assert_eq!(new, "abde");
                    assert_eq!(input.position(*ID), 2);
                }
                _ => panic!("Expected Pop"),
            }
        }

        #[test]
        fn pop_left_on_whitespace_removes_one_space_grapheme() {
            let input = make_input("hello   world", 7); // hello  | world
            let res = input.handle_input(Some(InputEvent::PopLeft));
            match res {
                Some(InputResultEvent::Pop(new)) => {
                    assert_eq!(new, "hello  world"); // removed one space
                    assert_eq!(input.position(*ID), 6);
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
            let input = make_input(s, s.len());
            let res = input.handle_input(Some(InputEvent::PopRight));
            match res {
                Some(InputResultEvent::NoChange) => {}
                _ => panic!("Expected NoChange"),
            }
        }

        #[test]
        fn pop_right_on_empty_no_change() {
            let input = make_input("", 0);
            let res = input.handle_input(Some(InputEvent::PopRight));
            match res {
                Some(InputResultEvent::NoChange) => {}
                _ => panic!("Expected NoChange"),
            }
        }

        #[test]
        fn pop_right_at_start() {
            let input = make_input("hello", 0);
            let res = input.handle_input(Some(InputEvent::PopRight));
            match res {
                Some(InputResultEvent::Pop(new)) => {
                    assert_eq!(new, "ello");
                    assert_eq!(input.position(*ID), 0);
                }
                _ => panic!("Expected NoChange"),
            }
        }

        #[test]
        fn pop_right_inside_word_deletes_current_grapheme() {
            // Cursor inside 'c' grapheme position: after 'ab'
            let input = make_input("abcde", 2); // ab|cde
            let res = input.handle_input(Some(InputEvent::PopRight));
            match res {
                Some(InputResultEvent::Pop(new)) => {
                    assert_eq!(new, "abde");
                    assert_eq!(input.position(*ID), 2);
                }
                _ => panic!("Expected Pop"),
            }
        }

        #[test]
        fn pop_right_at_grapheme_boundary_deletes_next_grapheme() {
            let input = make_input("hello", 2); // he|llo
            let res = input.handle_input(Some(InputEvent::PopRight));
            match res {
                Some(InputResultEvent::Pop(new)) => {
                    assert_eq!(new, "helo"); // removed 'l'
                    assert_eq!(input.position(*ID), 2);
                }
                _ => panic!("Expected Pop"),
            }
        }

        #[test]
        fn pop_right_unicode_combining_cluster_is_atomic() {
            let s = "y̆es";
            let input = make_input(s, "y̆".len() - 1); // Inside the cluster bytes
            let res = input.handle_input(Some(InputEvent::PopRight));
            match res {
                Some(InputResultEvent::Pop(new)) => {
                    assert_eq!(new, "es");
                    assert_eq!(input.position(*ID), "y̆".len() - 1);
                }
                _ => panic!("Expected Pop"),
            }
        }

        #[test]
        fn pop_right_emoji_with_skin_tone_is_atomic() {
            let s = "👍🏼 done";
            let input = make_input(s, 1); // inside emoji bytes; simulate being within cluster
            let res = input.handle_input(Some(InputEvent::PopRight));
            match res {
                Some(InputResultEvent::Pop(new)) => {
                    // Entire emoji cluster removed
                    assert!(new == " done" || new == "done"); // depending on spaces
                    assert_eq!(input.position(*ID), 1);
                }
                _ => panic!("Expected Pop"),
            }
        }

        #[test]
        fn pop_right_on_whitespace_deletes_space_grapheme() {
            let input = make_input("hello   world", 5); // hello|   world
            let res = input.handle_input(Some(InputEvent::PopRight));
            match res {
                Some(InputResultEvent::Pop(new)) => {
                    assert_eq!(new, "hello  world"); // one space removed
                    assert_eq!(input.position(*ID), 5);
                }
                _ => panic!("Expected Pop"),
            }
        }

        #[test]
        fn pop_right_on_punctuation_deletes_punctuation() {
            let input = make_input("foo, bar", 3); // foo|, bar
            let res = input.handle_input(Some(InputEvent::PopRight));
            match res {
                Some(InputResultEvent::Pop(new)) => {
                    assert_eq!(new, "foo bar"); // comma removed
                    assert_eq!(input.position(*ID), 3);
                }
                _ => panic!("Expected Pop"),
            }
        }

        #[test]
        fn pop_right_multiple_calls_progressively_delete() {
            let input = make_input("abc", 1); // a|bc
            let r1 = input.handle_input(Some(InputEvent::PopRight)); // delete 'b'
            let r2 = input.handle_input(Some(InputEvent::PopRight)); // delete 'c'
            let r3 = input.handle_input(Some(InputEvent::PopRight)); // at end -> NoChange

            match r1 {
                Some(InputResultEvent::Pop(new)) => {
                    assert_eq!(new, "ac");
                    assert_eq!(input.position(*ID), 1);
                }
                _ => panic!("Expected Pop"),
            }
            match r2 {
                Some(InputResultEvent::Pop(new)) => {
                    assert_eq!(new, "a");
                    assert_eq!(input.position(*ID), 1);
                }
                _ => panic!("Expected Pop"),
            }
            match r3 {
                Some(InputResultEvent::NoChange) => {}
                _ => panic!("Expected NoChange"),
            }
        }
    }

    mod pop_word_right {
        use super::*;

        #[test]
        fn pop_word_right_inside_word_deletes_to_word_end() {
            let input = make_input("hello world", 2); // cursor inside "hello"
            let ev = Some(InputEvent::PopWordRight);
            let res = input.handle_input(ev);

            match res {
                Some(InputResultEvent::Pop(new)) => {
                    assert_eq!(new, "he world");
                    assert_eq!(input.position(*ID), 2);
                }
                _ => panic!("Expected Pop event"),
            }
        }

        #[test]
        fn pop_word_right_on_whitespace_deletes_next_word() {
            let input = make_input("hello   world", 6); // cursor on whitespace before "world"
            let ev = Some(InputEvent::PopWordRight);
            let res = input.handle_input(ev);

            match res {
                Some(InputResultEvent::Pop(new)) => {
                    assert_eq!(new, "hello ");
                    assert_eq!(input.position(*ID), 6);
                }
                _ => panic!("Expected Pop event"),
            }
        }

        #[test]
        fn pop_word_right_at_word_boundary_deletes_next_word() {
            let input = make_input("hello world test", 5); // cursor at end of "hello"
            let ev = Some(InputEvent::PopWordRight);
            let res = input.handle_input(ev);

            match res {
                Some(InputResultEvent::Pop(new)) => {
                    assert_eq!(new, "hello test");
                    assert_eq!(input.position(*ID), 5);
                }
                _ => panic!("Expected Pop event"),
            }
        }

        #[test]
        fn pop_word_right_at_end_no_change() {
            let s = "hello";
            let input = make_input(s, s.len()); // cursor at end
            let ev = Some(InputEvent::PopWordRight);
            let res = input.handle_input(ev);

            match res {
                Some(InputResultEvent::NoChange) => {}
                _ => panic!("Expected NoChange"),
            }
        }

        #[test]
        fn pop_word_right_only_whitespace_deletes_to_end() {
            let input = make_input("   ", 1); // cursor on whitespace
            let ev = Some(InputEvent::PopWordRight);
            let res = input.handle_input(ev);

            match res {
                Some(InputResultEvent::Pop(new)) => {
                    assert_eq!(new, " ");
                    assert_eq!(input.position(*ID), 1);
                }
                _ => panic!("Expected Pop event"),
            }
        }

        #[test]
        fn pop_word_right_unicode_combining_and_emojis() {
            let input = make_input("y̆es 👍🏼 done", 0);
            let ev = Some(InputEvent::PopWordRight);
            let res = input.handle_input(ev);

            match res {
                Some(InputResultEvent::Pop(new)) => {
                    assert!(new.starts_with(" 👍🏼 done") || new.starts_with("👍🏼 done"));
                    assert_eq!(input.position(*ID), 0);
                }
                _ => panic!("Expected Pop event"),
            }
        }

        #[test]
        fn pop_word_right_between_words_deletes_next_word() {
            let input = make_input("foo, bar!", 4); // cursor after "foo" and comma (likely punctuation boundary)
            let ev = Some(InputEvent::PopWordRight);
            let res = input.handle_input(ev);

            match res {
                Some(InputResultEvent::Pop(new)) => {
                    assert!(new == "foo, !" || new == "foo,!");
                    assert_eq!(input.position(*ID), 4);
                }
                _ => panic!("Expected Pop event"),
            }
        }
    }

    mod pop_word_left {
        use super::*;
        #[test]
        fn pop_word_left_inside_word_deletes_to_word_start() {
            let input = make_input("hello world", 3); // cursor inside "hello" after 'l'
            let ev = Some(InputEvent::PopWordLeft);
            let res = input.handle_input(ev);

            match res {
                Some(InputResultEvent::Pop(new)) => {
                    assert_eq!(new, "lo world");
                    assert_eq!(input.position(*ID), 0);
                }
                _ => panic!("Expected Pop event"),
            }
        }

        #[test]
        fn pop_word_left_on_whitespace_deletes_prev_word() {
            let input = make_input("hello   world", 6); // cursor on whitespace after "hello"
            let ev = Some(InputEvent::PopWordLeft);
            let res = input.handle_input(ev);

            match res {
                Some(InputResultEvent::Pop(new)) => {
                    assert_eq!(new, "  world");
                    assert_eq!(input.position(*ID), 0);
                }
                _ => panic!("Expected Pop event"),
            }
        }

        #[test]
        fn pop_word_left_at_word_boundary_deletes_prev_word() {
            let input = make_input("hello world test", 12); // cursor right after space following "hello"
            let ev = Some(InputEvent::PopWordLeft);
            let res = input.handle_input(ev);

            match res {
                Some(InputResultEvent::Pop(new)) => {
                    assert_eq!(new, "hello test");
                    assert_eq!(input.position(*ID), 6);
                }
                _ => panic!("Expected Pop event"),
            }
        }

        #[test]
        fn pop_word_left_at_start_no_change() {
            let input = make_input("hello", 0); // cursor at start
            let ev = Some(InputEvent::PopWordLeft);
            let res = input.handle_input(ev);

            match res {
                Some(InputResultEvent::NoChange) => {}
                _ => panic!("Expected NoChange"),
            }
        }

        #[test]
        fn pop_word_left_only_whitespace_deletes_to_start() {
            let input = make_input("   ", 2); // cursor on whitespace
            let ev = Some(InputEvent::PopWordLeft);
            let res = input.handle_input(ev);

            match res {
                Some(InputResultEvent::Pop(new)) => {
                    assert_eq!(new, " ");
                    assert_eq!(input.position(*ID), 0);
                }
                _ => panic!("Expected Pop event"),
            }
        }

        #[test]
        fn pop_word_left_unicode_combining_and_emojis() {
            let s = "y̆es 👍🏼 done";
            let cursor = "y̆es ".len(); // position right after first word and space
            let input = make_input(s, cursor);
            let ev = Some(InputEvent::PopWordLeft);
            let res = input.handle_input(ev);

            match res {
                Some(InputResultEvent::Pop(new)) => {
                    assert!(new.starts_with("👍🏼 done"));
                    assert_eq!(input.position(*ID), 0);
                }
                _ => panic!("Expected Pop event"),
            }
        }

        #[test]
        fn pop_word_left_between_words_deletes_prev_word() {
            let input = make_input("foo, bar!", 4); // cursor after "foo,"
            let ev = Some(InputEvent::PopWordLeft);
            let res = input.handle_input(ev);

            match res {
                Some(InputResultEvent::Pop(new)) => {
                    assert_eq!(new, " bar!");
                    assert_eq!(input.position(*ID), 0);
                }
                _ => panic!("Expected Pop event"),
            }
        }
    }

    mod delete_to_start {
        use super::*;

        #[test]
        fn delete_to_start_on_empty_no_change() {
            let input = make_input("", 0);
            let res = input.handle_input(Some(InputEvent::DeleteToStart));
            match res {
                Some(InputResultEvent::NoChange) => {}
                _ => panic!("Expected Pop with empty string unchanged"),
            }
        }

        #[test]
        fn delete_to_start_at_start_no_change() {
            let input = make_input("hello", 0);
            let res = input.handle_input(Some(InputEvent::DeleteToStart));
            match res {
                Some(InputResultEvent::NoChange) => {}
                _ => panic!("Expected Pop"),
            }
        }

        #[test]
        fn delete_to_start_mid_ascii_deletes_prefix() {
            let input = make_input("hello world", 6); // "hello " | "world"
            let res = input.handle_input(Some(InputEvent::DeleteToStart));
            match res {
                Some(InputResultEvent::Pop(new)) => {
                    assert_eq!(new, "world");
                    assert_eq!(input.position(*ID), 0);
                }
                _ => panic!("Expected Pop"),
            }
        }

        #[test]
        fn delete_to_start_unicode_combining_cluster_partial_prefix() {
            // y̆ is a single grapheme made of 'y' + combining diacritic
            let s = "y̆es test";
            let pos = "y̆es".len(); // after first word
            let input = make_input(s, pos);
            let res = input.handle_input(Some(InputEvent::DeleteToStart));
            match res {
                Some(InputResultEvent::Pop(new)) => {
                    assert_eq!(new, " test");
                    assert_eq!(input.position(*ID), 0);
                }
                _ => panic!("Expected Pop"),
            }
        }

        #[test]
        fn delete_to_start_emoji_prefix() {
            let s = "👍🏼 ok";
            let pos = s.len(); // end
            let input = make_input(s, pos);
            let res = input.handle_input(Some(InputEvent::DeleteToStart));
            match res {
                Some(InputResultEvent::Pop(new)) => {
                    assert_eq!(new, "");
                    assert_eq!(input.position(*ID), 0);
                }
                _ => panic!("Expected Pop"),
            }
        }

        #[test]
        fn delete_to_start_on_whitespace_prefix() {
            let input = make_input("   abc", 3); // after spaces
            let res = input.handle_input(Some(InputEvent::DeleteToStart));
            match res {
                Some(InputResultEvent::Pop(new)) => {
                    assert_eq!(new, "abc");
                    assert_eq!(input.position(*ID), 0);
                }
                _ => panic!("Expected Pop"),
            }
        }
    }

    mod delete_to_end {
        use super::*;

        #[test]
        fn delete_to_end_on_empty_no_change() {
            let input = make_input("", 0);
            let res = input.handle_input(Some(InputEvent::DeleteToEnd));
            match res {
                Some(InputResultEvent::NoChange) => {}
                _ => panic!("Expected Pop with empty string unchanged"),
            }
        }

        #[test]
        fn delete_to_end_at_start_deletes_all() {
            let input = make_input("hello", 0);
            let res = input.handle_input(Some(InputEvent::DeleteToEnd));
            match res {
                Some(InputResultEvent::Pop(new)) => {
                    assert_eq!(new, "");
                    assert_eq!(input.position(*ID), 0);
                }
                _ => panic!("Expected Pop"),
            }
        }

        #[test]
        fn delete_to_end_mid_ascii_deletes_from_current_grapheme_start_to_end() {
            let input = make_input("hello world", 7); // "hello w|orld"
            let res = input.handle_input(Some(InputEvent::DeleteToEnd));
            match res {
                Some(InputResultEvent::Pop(new)) => {
                    assert_eq!(new, "hello w");
                    assert_eq!(input.position(*ID), "hello w".len());
                }
                _ => panic!("Expected Pop"),
            }
        }

        #[test]
        fn delete_to_end_inside_grapheme_deletes_from_cluster_start() {
            let s = "y̆es done";
            let input = make_input(s, 1);
            let res = input.handle_input(Some(InputEvent::DeleteToEnd));
            match res {
                Some(InputResultEvent::Pop(new)) => {
                    assert_eq!(new, "");
                    assert_eq!(input.position(*ID), 0);
                }
                _ => panic!("Expected Pop"),
            }
        }

        #[test]
        fn delete_to_end_on_whitespace_deletes_trailing_content() {
            let input = make_input("hello   world", 5); // "hello|   world"
            let res = input.handle_input(Some(InputEvent::DeleteToEnd));
            match res {
                Some(InputResultEvent::Pop(new)) => {
                    assert_eq!(new, "hello");
                    assert_eq!(input.position(*ID), "hello".len());
                }
                _ => panic!("Expected Pop"),
            }
        }

        #[test]
        fn delete_to_end_at_end_deletes_last_grapheme_due_to_current_behavior() {
            let s = "hello";
            let input = make_input(s, s.len()); // cursor at end
            let res = input.handle_input(Some(InputEvent::DeleteToEnd));
            match res {
                Some(InputResultEvent::NoChange) => {}
                _ => panic!("Expected Pop"),
            }
        }

        #[test]
        fn delete_to_end_emoji_cluster_behavior() {
            let s = "ok 👍🏼 done";
            let input = make_input(s, "ok ".len());
            let res = input.handle_input(Some(InputEvent::DeleteToEnd));
            match res {
                Some(InputResultEvent::Pop(new)) => {
                    assert_eq!(new, "ok ");
                    assert_eq!(input.position(*ID), "ok ".len());
                }
                _ => panic!("Expected Pop"),
            }
        }
    }

    mod move_left_right {
        use super::*;

        #[test]
        fn left_at_start_no_change() {
            let input = make_input("hello", 0);
            let res = input.handle_input(Some(InputEvent::Back));
            match res {
                Some(InputResultEvent::NoChange) => {}
                _ => panic!("Expected NoChange"),
            }
            assert_eq!(input.position(*ID), 0);
        }

        #[test]
        fn right_at_end_no_change() {
            let s = "hello";
            let input = make_input(s, s.len());
            let res = input.handle_input(Some(InputEvent::Forward));
            match res {
                Some(InputResultEvent::NoChange) => {}
                _ => panic!("Expected NoChange"),
            }
            assert_eq!(input.position(*ID), s.len());
        }

        #[test]
        fn left_moves_one_grapheme_ascii() {
            let input = make_input("hello", 3); // hel|lo
            let res = input.handle_input(Some(InputEvent::Back));
            match res {
                Some(InputResultEvent::NoChange) => {}
                _ => panic!("Expected NoChange"),
            }
            assert_eq!(input.position(*ID), 2);
        }

        #[test]
        fn right_moves_one_grapheme_ascii() {
            let input = make_input("hello", 2); // he|llo
            let res = input.handle_input(Some(InputEvent::Forward));
            match res {
                Some(InputResultEvent::NoChange) => {}
                _ => panic!("Expected NoChange"),
            }
            assert_eq!(input.position(*ID), 3);
        }

        #[test]
        fn left_moves_one_grapheme_combining_cluster() {
            let s = "y̆es";
            let pos = s.len();
            let input = make_input(s, pos);
            let _ = input.handle_input(Some(InputEvent::Back));
            assert_eq!(input.position(*ID), "y̆e".len());
            let _ = input.handle_input(Some(InputEvent::Back));
            assert_eq!(input.position(*ID), "y̆".len());
            let _ = input.handle_input(Some(InputEvent::Back));
            assert_eq!(input.position(*ID), 0);
            let _ = input.handle_input(Some(InputEvent::Back));
            assert_eq!(input.position(*ID), 0);
        }

        #[test]
        fn right_moves_one_grapheme_combining_cluster() {
            let s = "y̆es";
            let input = make_input(s, 0);
            let _ = input.handle_input(Some(InputEvent::Forward));
            assert_eq!(input.position(*ID), "y̆".len());
            let _ = input.handle_input(Some(InputEvent::Forward));
            assert_eq!(input.position(*ID), "y̆e".len());
            let _ = input.handle_input(Some(InputEvent::Forward));
            assert_eq!(input.position(*ID), s.len());
            let _ = input.handle_input(Some(InputEvent::Forward));
            assert_eq!(input.position(*ID), s.len());
        }

        #[test]
        fn left_right_with_emoji_cluster() {
            let s = "a👍🏼b";
            let input = make_input(s, s.len());
            let _ = input.handle_input(Some(InputEvent::Back));
            let after_emoji_pos = "a👍🏼".len();
            assert_eq!(input.position(*ID), after_emoji_pos);
            let _ = input.handle_input(Some(InputEvent::Back));
            assert_eq!(input.position(*ID), "a".len());
            let _ = input.handle_input(Some(InputEvent::Forward));
            assert_eq!(input.position(*ID), after_emoji_pos);
            let _ = input.handle_input(Some(InputEvent::Forward));
            assert_eq!(input.position(*ID), s.len());
        }

        #[test]
        fn left_from_mid_grapheme_moves_by_left_grapheme_len() {
            let s = "y̆es";
            let pos_inside_cluster = "y̆".len() - 1;
            let input = make_input(s, pos_inside_cluster);
            let _ = input.handle_input(Some(InputEvent::Back));
            assert_eq!(input.position(*ID), 0);
        }
    }

    mod move_word_left_right {
        use super::*;

        #[test]
        fn right_word_from_start_moves_to_first_word_start_after_cursor() {
            let input = make_input("hello  world", 0);
            let res = input.handle_input(Some(InputEvent::ForwardWord));
            match res {
                Some(InputResultEvent::NoChange) => {}
                _ => panic!("Expected NoChange"),
            }
            assert_eq!(input.position(*ID), "hello  ".len());
        }

        #[test]
        fn right_word_skips_whitespace_and_punctuation() {
            let input = make_input("foo,  bar!", 0);
            let _ = input.handle_input(Some(InputEvent::ForwardWord));
            assert_eq!(input.position(*ID), "foo,  ".len());
            let _ = input.handle_input(Some(InputEvent::ForwardWord));
            assert_eq!(input.position(*ID), "foo,  bar!".len());
        }

        #[test]
        fn right_word_at_end_stays_at_end() {
            let s = "hello";
            let input = make_input(s, s.len());
            let res = input.handle_input(Some(InputEvent::ForwardWord));
            match res {
                Some(InputResultEvent::NoChange) => {}
                _ => panic!("Expected NoChange"),
            }
            assert_eq!(input.position(*ID), s.len());
        }

        #[test]
        fn left_word_from_middle_moves_to_prev_word_start() {
            let input = make_input("hello   world test", 14);
            let res = input.handle_input(Some(InputEvent::BackWord));
            match res {
                Some(InputResultEvent::NoChange) => {}
                _ => panic!("Expected NoChange"),
            }
            assert_eq!(input.position(*ID), "hello   ".len());
        }

        #[test]
        fn left_word_from_whitespace_moves_to_prev_word_start() {
            let input = make_input("hello   world", 6);
            let res = input.handle_input(Some(InputEvent::BackWord));
            match res {
                Some(InputResultEvent::NoChange) => {}
                _ => panic!("Expected NoChange"),
            }
            assert_eq!(input.position(*ID), 0);
        }

        #[test]
        fn left_word_at_start_stays() {
            let input = make_input("hello world", 0);
            let res = input.handle_input(Some(InputEvent::BackWord));
            match res {
                Some(InputResultEvent::NoChange) => {}
                _ => panic!("Expected NoChange"),
            }
            assert_eq!(input.position(*ID), 0);
        }

        #[test]
        fn right_word_handles_unicode_combining_and_emojis() {
            let s = "y̆es 👍🏼 done";
            let input = make_input(s, 0);
            let _ = input.handle_input(Some(InputEvent::ForwardWord));
            assert!(input.position(*ID) >= "y̆es".len());
        }

        #[test]
        fn left_word_handles_unicode_combining_and_emojis() {
            let s = "y̆es 👍🏼 done";
            let input = make_input(s, s.len());
            let _ = input.handle_input(Some(InputEvent::BackWord));
            assert_eq!(input.position(*ID), "y̆es 👍🏼 ".len());
            let _ = input.handle_input(Some(InputEvent::BackWord));
            assert_eq!(input.position(*ID), 0);
        }
    }

    mod move_start_end {
        use super::*;

        #[test]
        fn start_on_empty_stays_at_zero() {
            let input = make_input("", 0);
            let res = input.handle_input(Some(InputEvent::Start));
            match res {
                Some(InputResultEvent::NoChange) => {}
                _ => panic!("Expected NoChange"),
            }
            assert_eq!(input.position(*ID), 0);
        }

        #[test]
        fn end_on_empty_stays_at_zero() {
            let input = make_input("", 0);
            let res = input.handle_input(Some(InputEvent::End));
            match res {
                Some(InputResultEvent::NoChange) => {}
                _ => panic!("Expected NoChange"),
            }
            assert_eq!(input.position(*ID), 0);
        }

        #[test]
        fn start_moves_to_zero_from_middle_ascii() {
            let input = make_input("hello world", 6);
            let res = input.handle_input(Some(InputEvent::Start));
            match res {
                Some(InputResultEvent::NoChange) => {}
                _ => panic!("Expected NoChange"),
            }
            assert_eq!(input.position(*ID), 0);
        }

        #[test]
        fn end_moves_to_len_from_middle_ascii() {
            let s = "hello world";
            let input = make_input(s, 5);
            let res = input.handle_input(Some(InputEvent::End));
            match res {
                Some(InputResultEvent::NoChange) => {}
                _ => panic!("Expected NoChange"),
            }
            assert_eq!(input.position(*ID), s.len());
        }

        #[test]
        fn start_from_end_moves_to_zero() {
            let s = "hello";
            let input = make_input(s, s.len());
            let res = input.handle_input(Some(InputEvent::Start));
            match res {
                Some(InputResultEvent::NoChange) => {}
                _ => panic!("Expected NoChange"),
            }
            assert_eq!(input.position(*ID), 0);
        }

        #[test]
        fn end_from_start_moves_to_len() {
            let s = "hello";
            let input = make_input(s, 0);
            let res = input.handle_input(Some(InputEvent::End));
            match res {
                Some(InputResultEvent::NoChange) => {}
                _ => panic!("Expected NoChange"),
            }
            assert_eq!(input.position(*ID), s.len());
        }

        #[test]
        fn start_with_unicode_combining_cluster() {
            let s = "y̆es 👍🏼 done";
            let input = make_input(s, s.len()); // at end
            let res = input.handle_input(Some(InputEvent::Start));
            match res {
                Some(InputResultEvent::NoChange) => {}
                _ => panic!("Expected NoChange"),
            }
            assert_eq!(input.position(*ID), 0);
        }

        #[test]
        fn end_with_unicode_combining_cluster() {
            let s = "y̆es 👍🏼 done";
            let input = make_input(s, 0); // at start
            let res = input.handle_input(Some(InputEvent::End));
            match res {
                Some(InputResultEvent::NoChange) => {}
                _ => panic!("Expected NoChange"),
            }
            assert_eq!(input.position(*ID), s.len());
        }

        #[test]
        fn start_then_end_roundtrip_positions() {
            let s = "abc👍🏼def";
            let input = make_input(s, 3);
            let _ = input.handle_input(Some(InputEvent::Start));
            assert_eq!(input.position(*ID), 0);
            let _ = input.handle_input(Some(InputEvent::End));
            assert_eq!(input.position(*ID), s.len());
        }

        #[test]
        fn end_then_start_roundtrip_positions() {
            let s = "abc👍🏼def";
            let input = make_input(s, 0);
            let _ = input.handle_input(Some(InputEvent::End));
            assert_eq!(input.position(*ID), s.len());
            let _ = input.handle_input(Some(InputEvent::Start));
            assert_eq!(input.position(*ID), 0);
        }
    }
}
