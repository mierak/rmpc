use unicode_segmentation::UnicodeSegmentation;

use crate::ui::input::{InputEvent, InputResultEvent};

#[derive(Debug, Default)]
pub(super) struct InputBuffer {
    value: String,
    offset: usize,
}

#[derive(Default)]
pub struct Grapheme {
    offset: usize,
    len: usize,
}

impl InputBuffer {
    pub fn value(&self) -> String {
        self.value.clone()
    }

    pub fn set_value(&mut self, value: String) {
        self.value = value;
        self.offset = self.value.len();
    }

    pub fn clear(&mut self) {
        self.value.clear();
        self.offset = 0;
    }

    pub fn offset(&self) -> usize {
        self.offset
    }

    pub fn set_offset(&mut self, offset: usize) {
        self.offset = offset;
    }

    pub fn handle_input(&mut self, ev: Option<InputEvent>) -> Option<InputResultEvent> {
        match ev {
            Some(InputEvent::Cancel) => {
                debug_assert!(false, "Handled in Input, should be unreachable");
                None
            }
            Some(InputEvent::Confirm) => {
                debug_assert!(false, "Handled in Input, should be unreachable");
                None
            }
            Some(InputEvent::Push(c)) => {
                self.value.insert(self.offset, c);
                self.offset += c.len_utf8();

                Some(InputResultEvent::Push(self.value.clone()))
            }
            // Delete
            Some(InputEvent::PopLeft) => {
                if self.offset == 0 {
                    return Some(InputResultEvent::NoChange);
                }

                let grapheme = self.current_grapheme();
                self.value.drain(grapheme.offset..grapheme.offset + grapheme.len);
                self.offset = grapheme.offset;

                Some(InputResultEvent::Pop(self.value.clone()))
            }
            Some(InputEvent::PopRight) => {
                if self.offset == self.value.len() {
                    return Some(InputResultEvent::NoChange);
                }

                let grapheme = self.next_grapheme();
                self.value.drain(grapheme.offset..grapheme.offset + grapheme.len);

                Some(InputResultEvent::Pop(self.value.clone()))
            }
            Some(InputEvent::PopWordLeft) => {
                if self.offset == 0 {
                    return Some(InputResultEvent::NoChange);
                }

                let deletion_start = self
                    .value
                    .unicode_word_indices()
                    .find(|(idx, w)| {
                        // -1 so taht the if cursor is at the start of a word, the word itself is
                        // not counted and instead the word before is considered
                        (*idx..*idx + w.len()).contains(&self.offset.saturating_sub(1))
                    })
                    .map(|(idx, _)| idx)
                    .or_else(|| {
                        self.value
                            .unicode_word_indices()
                            .take_while(|(idx, _)| *idx < self.offset)
                            .last()
                            .map(|(idx, _)| idx)
                    })
                    .unwrap_or(0);

                if deletion_start >= self.offset {
                    return Some(InputResultEvent::NoChange);
                }

                self.value.drain(deletion_start..self.offset);
                self.offset = deletion_start;

                Some(InputResultEvent::Pop(self.value.clone()))
            }
            Some(InputEvent::PopWordRight) => {
                if self.offset >= self.value.len() {
                    return Some(InputResultEvent::NoChange);
                }

                let bytes_to_drain = self
                    .value
                    .unicode_word_indices()
                    .find(|(idx, w)| (*idx..*idx + w.len()).contains(&self.offset))
                    .map(|(idx, w)| w.len().saturating_sub(self.offset.saturating_sub(idx)))
                    .or_else(|| {
                        self.value
                            .unicode_word_indices()
                            .find(|(idx, _)| idx > &self.offset)
                            .map(|(idx, w)| idx.saturating_sub(self.offset) + w.len())
                    })
                    .unwrap_or_else(|| self.value.len().saturating_sub(self.offset));

                self.value.drain(self.offset..self.offset + bytes_to_drain);

                Some(InputResultEvent::Pop(self.value.clone()))
            }
            Some(InputEvent::DeleteToStart) => {
                if self.offset == 0 {
                    return Some(InputResultEvent::NoChange);
                }
                self.value.drain(0..self.offset);
                self.offset = 0;

                Some(InputResultEvent::Pop(self.value.clone()))
            }
            Some(InputEvent::DeleteToEnd) => {
                if self.offset == self.value.len() {
                    return Some(InputResultEvent::NoChange);
                }
                let grapheme = self.next_grapheme();
                self.value.drain(grapheme.offset..);
                self.offset = self.value.len();

                Some(InputResultEvent::Pop(self.value.clone()))
            }

            // Movement
            Some(InputEvent::Back) => {
                self.offset = self.offset.saturating_sub(self.current_grapheme().len);
                Some(InputResultEvent::NoChange)
            }
            Some(InputEvent::Forward) => {
                self.offset = (self.offset + self.next_grapheme().len).min(self.value.len());
                Some(InputResultEvent::NoChange)
            }
            Some(InputEvent::Start) => {
                self.offset = 0;
                Some(InputResultEvent::NoChange)
            }
            Some(InputEvent::End) => {
                self.offset = self.value.len();
                Some(InputResultEvent::NoChange)
            }
            Some(InputEvent::BackWord) => {
                let prev = self.prev_word_boundary();
                self.offset = prev.max(0);
                Some(InputResultEvent::NoChange)
            }
            Some(InputEvent::ForwardWord) => {
                let next = self.next_word_boundary();
                self.offset = next.min(self.value.len());
                Some(InputResultEvent::NoChange)
            }
            None => None,
        }
    }

    #[inline]
    pub fn next_word_boundary(&self) -> usize {
        self.value
            .unicode_word_indices()
            .find(|(idx, _)| idx > &self.offset)
            .map_or(self.value.len(), |(idx, _)| idx)
    }

    #[inline]
    pub fn prev_word_boundary(&self) -> usize {
        self.value
            .unicode_word_indices()
            .take_while(|(idx, _)| idx < &self.offset)
            .last()
            .map_or(0, |(idx, _)| idx)
    }

    #[inline]
    pub fn current_grapheme(&self) -> Grapheme {
        self.value
            .grapheme_indices(true)
            .take_while(|(idx, _)| idx < &self.offset)
            .last()
            .map_or(Grapheme::default(), |(idx, g)| Grapheme { offset: idx, len: g.len() })
    }

    #[inline]
    pub fn next_grapheme(&self) -> Grapheme {
        self.value
            .grapheme_indices(true)
            .take_while(|(idx, _)| idx <= &self.offset)
            .last()
            .map_or(Grapheme::default(), |(idx, g)| Grapheme { offset: idx, len: g.len() })
    }
}
