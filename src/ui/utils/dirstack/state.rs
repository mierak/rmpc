use std::{collections::BTreeSet, ops::SubAssign};

use ratatui::widgets::ScrollbarState;

use super::ScrollingState;

#[derive(Debug, Default)]
pub struct DirState<T: ScrollingState> {
    scrollbar_state: ScrollbarState,
    inner: T,
    pub marked: BTreeSet<usize>,
    content_len: Option<usize>,
    viewport_len: Option<usize>,
}

#[allow(dead_code)]
impl<T: ScrollingState> DirState<T> {
    pub fn set_viewport_len(&mut self, viewport_len: Option<usize>) -> &Self {
        self.viewport_len = viewport_len;
        self.scrollbar_state = self.scrollbar_state.viewport_content_length(viewport_len.unwrap_or(0));
        self
    }

    pub fn set_content_len(&mut self, content_len: Option<usize>) -> &Self {
        self.content_len = content_len;
        self.scrollbar_state = self.scrollbar_state.content_length(content_len.unwrap_or(0));
        self
    }

    pub fn first(&mut self) {
        if self.content_len.is_some_and(|v| v > 0) {
            self.select(Some(0));
        } else {
            self.select(None);
        }
    }

    pub fn last(&mut self) {
        if let Some(item_count) = self.content_len {
            if item_count > 0 {
                self.select(Some(item_count.saturating_sub(1)));
            } else {
                self.select(None);
            }
        } else {
            self.select(None);
        }
    }

    pub fn next(&mut self) {
        if let Some(item_count) = self.content_len {
            let i = match self.get_selected() {
                Some(i) => {
                    if i >= item_count.saturating_sub(1) {
                        Some(0)
                    } else {
                        Some(i + 1)
                    }
                }
                None if item_count > 0 => Some(0),
                None => None,
            };
            self.select(i);
        } else {
            self.select(None);
        }
    }

    pub fn prev(&mut self) {
        if let Some(item_count) = self.content_len {
            let i = match self.get_selected() {
                Some(i) => {
                    if i == 0 {
                        Some(item_count.saturating_sub(1))
                    } else {
                        Some(i - 1)
                    }
                }
                None if item_count > 0 => Some(item_count.saturating_sub(1)),
                None => None,
            };
            self.select(i);
        } else {
            self.select(None);
        }
    }

    pub fn next_half_viewport(&mut self) {
        if let Some(item_count) = self.content_len {
            if let Some(viewport) = self.viewport_len {
                self.select(
                    self.get_selected()
                        .map(|i| i.saturating_add(viewport / 2).min(item_count.saturating_sub(1))),
                );
            } else {
                self.select(None);
            }
        } else {
            self.select(None);
        }
    }

    pub fn prev_half_viewport(&mut self) {
        if self.content_len.is_some() {
            if let Some(viewport) = self.viewport_len {
                self.select(self.get_selected().map(|i| i.saturating_sub(viewport / 2).max(0)));
            } else {
                self.select(None);
            }
        } else {
            self.select(None);
        }
    }

    pub fn select(&mut self, idx: Option<usize>) {
        let idx = idx.map(|idx| idx.max(0).min(self.content_len.map_or(0, |len| len - 1)));
        self.inner.select_scrolling(idx);
        self.scrollbar_state = self.scrollbar_state.position(idx.unwrap_or(0));
    }

    #[allow(clippy::comparison_chain)]
    pub fn remove(&mut self, idx: usize) {
        match self.content_len {
            Some(len) if idx >= len => return,
            None => return,
            Some(ref mut len) => {
                self.marked = std::mem::take(&mut self.marked)
                    .into_iter()
                    .filter_map(|val| {
                        if val < idx {
                            Some(val)
                        } else if val > idx {
                            Some(val - 1)
                        } else {
                            None
                        }
                    })
                    .collect();
                len.sub_assign(1);
                let len: usize = *len;
                if self.get_selected().is_some_and(|selected| selected >= len) {
                    self.last();
                }
            }
        }
    }

    pub fn unmark_all(&mut self) {
        self.marked.clear();
    }

    pub fn mark(&mut self, idx: usize) -> bool {
        self.marked.insert(idx)
    }

    pub fn unmark(&mut self, idx: usize) -> bool {
        self.marked.remove(&idx)
    }

    pub fn toggle_mark(&mut self, idx: usize) -> bool {
        if self.marked.contains(&idx) {
            self.marked.remove(&idx)
        } else {
            self.marked.insert(idx)
        }
    }

    pub fn get_marked(&self) -> &BTreeSet<usize> {
        &self.marked
    }

    pub fn get_selected(&self) -> Option<usize> {
        self.inner.get_selected_scrolling()
    }

    pub fn as_render_state_ref(&mut self) -> &mut T {
        &mut self.inner
    }

    pub fn as_scrollbar_state_ref(&mut self) -> &mut ScrollbarState {
        &mut self.scrollbar_state
    }
}

#[cfg(test)]
mod tests {
    use ratatui::widgets::ListState;

    use super::DirState;

    #[test]
    fn viewport_len_sets_properties() {
        let mut subject: DirState<ListState> = DirState::default();

        subject.set_viewport_len(Some(1337));

        assert_eq!(subject.viewport_len, Some(1337));
    }

    #[test]
    fn content_len_sets_properties() {
        let mut subject: DirState<ListState> = DirState::default();

        subject.set_content_len(Some(1337));

        assert_eq!(subject.content_len, Some(1337));
    }

    mod first {
        use ratatui::widgets::ListState;

        use crate::ui::utils::dirstack::DirState;

        #[test]
        fn when_content_is_none() {
            let mut subject: DirState<ListState> = DirState::default();
            subject.set_content_len(None);

            subject.first();

            assert_eq!(subject.get_selected(), None);
        }

        #[test]
        fn when_content_is_zero() {
            let mut subject: DirState<ListState> = DirState::default();
            subject.set_content_len(Some(0));

            subject.first();

            assert_eq!(subject.get_selected(), None);
        }

        #[test]
        fn when_content_is_not_empty() {
            let mut subject: DirState<ListState> = DirState::default();
            subject.set_content_len(Some(5));

            subject.first();

            assert_eq!(subject.get_selected(), Some(0));
        }
    }

    mod last {
        use ratatui::widgets::ListState;

        use crate::ui::utils::dirstack::DirState;

        #[test]
        fn when_content_is_none() {
            let mut subject: DirState<ListState> = DirState::default();
            subject.set_content_len(None);

            subject.last();

            assert_eq!(subject.get_selected(), None);
        }

        #[test]
        fn when_content_is_zero() {
            let mut subject: DirState<ListState> = DirState::default();
            subject.set_content_len(Some(0));

            subject.last();

            assert_eq!(subject.get_selected(), None);
        }

        #[test]
        fn when_content_is_not_empty() {
            let mut subject: DirState<ListState> = DirState::default();
            subject.set_content_len(Some(5));

            subject.last();

            assert_eq!(subject.get_selected(), Some(4));
        }
    }

    mod next {
        use ratatui::widgets::ListState;

        use crate::ui::utils::dirstack::DirState;

        #[test]
        fn when_content_is_none() {
            let mut subject: DirState<ListState> = DirState::default();
            subject.set_content_len(None);

            subject.next();

            assert_eq!(subject.get_selected(), None);
        }

        #[test]
        fn when_content_is_zero() {
            let mut subject: DirState<ListState> = DirState::default();
            subject.set_content_len(Some(0));

            subject.next();

            assert_eq!(subject.get_selected(), None);
        }

        #[test]
        fn switches_to_first_item_when_nothing_is_selected() {
            let mut subject: DirState<ListState> = DirState::default();
            subject.set_content_len(Some(10));
            subject.select(None);

            subject.next();

            assert_eq!(subject.get_selected(), Some(0));
        }

        #[test]
        fn switches_to_next_item() {
            let mut subject: DirState<ListState> = DirState::default();
            subject.set_content_len(Some(10));
            subject.select(Some(5));

            subject.next();

            assert_eq!(subject.get_selected(), Some(6));
        }

        #[test]
        fn wraps_around() {
            let mut subject: DirState<ListState> = DirState::default();
            subject.set_content_len(Some(10));
            subject.select(Some(9));

            subject.next();

            assert_eq!(subject.get_selected(), Some(0));
        }
    }

    mod prev {
        use ratatui::widgets::ListState;

        use crate::ui::utils::dirstack::DirState;

        #[test]
        fn when_content_is_none() {
            let mut subject: DirState<ListState> = DirState::default();
            subject.set_content_len(None);

            subject.prev();

            assert_eq!(subject.get_selected(), None);
        }

        #[test]
        fn when_content_is_zero() {
            let mut subject: DirState<ListState> = DirState::default();
            subject.set_content_len(Some(0));

            subject.prev();

            assert_eq!(subject.get_selected(), None);
        }

        #[test]
        fn switches_to_last_item_when_nothing_is_selected() {
            let mut subject: DirState<ListState> = DirState::default();
            subject.set_content_len(Some(10));
            subject.select(None);

            subject.prev();

            assert_eq!(subject.get_selected(), Some(9));
        }

        #[test]
        fn switches_to_prev_item() {
            let mut subject: DirState<ListState> = DirState::default();
            subject.set_content_len(Some(10));
            subject.select(Some(5));

            subject.prev();

            assert_eq!(subject.get_selected(), Some(4));
        }

        #[test]
        fn wraps_around() {
            let mut subject: DirState<ListState> = DirState::default();
            subject.set_content_len(Some(10));
            subject.select(Some(0));

            subject.prev();

            assert_eq!(subject.get_selected(), Some(9));
        }
    }

    mod next_half_viewport {
        use ratatui::widgets::ListState;

        use crate::ui::utils::dirstack::DirState;

        #[test]
        fn when_content_is_none() {
            let mut subject: DirState<ListState> = DirState::default();
            subject.set_content_len(None);
            subject.set_viewport_len(Some(5));

            subject.next_half_viewport();

            assert_eq!(subject.get_selected(), None);
        }

        #[test]
        fn when_viewport_is_none() {
            let mut subject: DirState<ListState> = DirState::default();
            subject.set_content_len(Some(5));
            subject.set_viewport_len(None);

            subject.next_half_viewport();

            assert_eq!(subject.get_selected(), None);
        }

        #[test]
        fn goes_forward_by_half_viewport() {
            let mut subject: DirState<ListState> = DirState::default();
            subject.set_content_len(Some(20));
            subject.set_viewport_len(Some(10));
            subject.select(Some(8));

            subject.next_half_viewport();

            assert_eq!(subject.get_selected(), Some(13));
        }

        #[test]
        fn caps_at_last_item() {
            let mut subject: DirState<ListState> = DirState::default();
            subject.set_content_len(Some(20));
            subject.set_viewport_len(Some(10));
            subject.select(Some(16));

            subject.next_half_viewport();

            assert_eq!(subject.get_selected(), Some(19));
        }
    }

    mod prev_half_viewport {
        use ratatui::widgets::ListState;

        use crate::ui::utils::dirstack::DirState;

        #[test]
        fn when_content_is_none() {
            let mut subject: DirState<ListState> = DirState::default();
            subject.set_content_len(None);
            subject.set_viewport_len(Some(5));

            subject.prev_half_viewport();

            assert_eq!(subject.get_selected(), None);
        }

        #[test]
        fn when_viewport_is_none() {
            let mut subject: DirState<ListState> = DirState::default();
            subject.set_content_len(Some(5));
            subject.set_viewport_len(None);

            subject.prev_half_viewport();

            assert_eq!(subject.get_selected(), None);
        }

        #[test]
        fn goes_forward_by_half_viewport() {
            let mut subject: DirState<ListState> = DirState::default();
            subject.set_content_len(Some(20));
            subject.set_viewport_len(Some(10));
            subject.select(Some(8));

            subject.prev_half_viewport();

            assert_eq!(subject.get_selected(), Some(3));
        }

        #[test]
        fn caps_at_first_item() {
            let mut subject: DirState<ListState> = DirState::default();
            subject.set_content_len(Some(20));
            subject.set_viewport_len(Some(10));
            subject.select(Some(4));

            subject.prev_half_viewport();

            assert_eq!(subject.get_selected(), Some(0));
        }
    }

    mod select {

        use ratatui::widgets::ListState;

        use crate::ui::utils::dirstack::DirState;

        #[test]
        fn select_last_element_when_out_of_bounds() {
            let mut subject: DirState<ListState> = DirState::default();
            subject.set_content_len(Some(100));

            subject.select(Some(150));

            assert_eq!(subject.get_selected(), Some(99));
        }
    }

    mod remove {
        use std::collections::BTreeSet;

        use ratatui::widgets::ListState;

        use crate::ui::utils::dirstack::DirState;

        #[test]
        fn does_nothing_when_no_content() {
            let mut subject: DirState<ListState> = DirState::default();
            subject.set_content_len(Some(100));
            subject.select(Some(50));
            subject.mark(5);
            assert_eq!(subject.get_selected(), Some(50));
            assert_eq!(subject.marked, BTreeSet::from([5]));
            subject.set_content_len(None);

            subject.remove(5);

            assert_eq!(subject.get_selected(), Some(50));
            assert_eq!(subject.marked, BTreeSet::from([5]));
        }

        #[test]
        fn does_nothing_when_removing_outside_range() {
            let mut subject: DirState<ListState> = DirState::default();
            subject.set_content_len(Some(100));
            subject.select(Some(50));
            subject.mark(5);
            assert_eq!(subject.get_selected(), Some(50));
            assert_eq!(subject.marked, BTreeSet::from([5]));

            subject.remove(101);

            assert_eq!(subject.get_selected(), Some(50));
            assert_eq!(subject.marked, BTreeSet::from([5]));
        }

        #[test]
        fn properly_filters_marked_elements() {
            let mut subject: DirState<ListState> = DirState::default();
            subject.set_content_len(Some(100));
            subject.select(Some(50));
            (0..100).step_by(10).for_each(|v| {
                subject.mark(v);
            });

            subject.remove(50);

            assert_eq!(subject.marked, BTreeSet::from([0, 10, 20, 30, 40, 59, 69, 79, 89]));
        }

        #[test]
        fn selects_last_element_when_last_was_selected() {
            let mut subject: DirState<ListState> = DirState::default();
            subject.set_content_len(Some(100));
            subject.select(Some(99));

            subject.remove(51);

            assert_eq!(subject.get_selected(), Some(98));
        }

        #[test]
        fn changes_length_properly() {
            let mut subject: DirState<ListState> = DirState::default();
            subject.set_content_len(Some(100));

            subject.remove(51);

            assert_eq!(subject.content_len, Some(99));
        }
    }

    mod marks {
        use std::collections::BTreeSet;

        use ratatui::widgets::ListState;

        use crate::ui::utils::dirstack::DirState;

        #[test]
        fn unmark_all_clears_map() {
            let mut subject: DirState<ListState> = DirState::default();
            (0..100).step_by(10).for_each(|v| {
                subject.mark(v);
            });
            assert_eq!(subject.marked, BTreeSet::from([0, 10, 20, 30, 40, 50, 60, 70, 80, 90]));

            subject.unmark_all();

            assert_eq!(subject.marked, BTreeSet::from([]));
        }

        #[test]
        fn unmark_all_with_no_marks_does_nothing() {
            let mut subject: DirState<ListState> = DirState::default();

            subject.unmark_all();

            assert_eq!(subject.marked, BTreeSet::from([]));
        }

        #[test]
        fn mark_marks_item() {
            let mut subject: DirState<ListState> = DirState::default();

            subject.mark(5);

            assert_eq!(subject.marked, BTreeSet::from([5]));
        }

        #[test]
        fn marking_marked_item_does_does_nothing() {
            let mut subject: DirState<ListState> = DirState::default();
            subject.mark(5);

            subject.mark(5);

            assert_eq!(subject.marked, BTreeSet::from([5]));
        }

        #[test]
        fn unmark_unmarks_item() {
            let mut subject: DirState<ListState> = DirState::default();
            subject.mark(5);
            assert_eq!(subject.marked, BTreeSet::from([5]));

            subject.unmark(5);

            assert_eq!(subject.marked, BTreeSet::from([]));
        }

        #[test]
        fn unmark_with_no_marks_does_nothing() {
            let mut subject: DirState<ListState> = DirState::default();

            subject.unmark(5);

            assert_eq!(subject.marked, BTreeSet::from([]));
        }

        #[test]
        fn toggle_switches_item() {
            let mut subject: DirState<ListState> = DirState::default();
            subject.mark(5);
            assert_eq!(subject.marked, BTreeSet::from([5]));

            subject.toggle_mark(5);
            subject.toggle_mark(10);

            assert_eq!(subject.marked, BTreeSet::from([10]));
        }
    }
}
