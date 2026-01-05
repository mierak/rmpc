use std::{collections::BTreeSet, ops::SubAssign};

use ratatui::widgets::ScrollbarState;

use super::ScrollingState;

#[derive(Debug, Default)]
pub struct DirState<T: ScrollingState> {
    pub scrollbar_state: ScrollbarState,
    pub inner: T,
    pub marked: BTreeSet<usize>,
    content_len: Option<usize>,
    viewport_len: Option<usize>,
}

#[allow(dead_code)]
impl<T: ScrollingState> DirState<T> {
    pub fn viewport_len(&self) -> Option<usize> {
        self.viewport_len
    }

    pub fn set_viewport_len(&mut self, viewport_len: Option<usize>) {
        match (self.content_len, viewport_len) {
            (Some(content_len), Some(viewport_len)) => {
                self.set_content_and_viewport_len(content_len, viewport_len);
            }
            (None, Some(viewport_len)) => {
                self.viewport_len = Some(viewport_len);
            }
            (_, None) => {
                self.viewport_len = None;
            }
        }
    }

    pub fn set_content_len(&mut self, content_len: Option<usize>) {
        match (content_len, self.viewport_len) {
            (Some(content_len), Some(viewport_len)) => {
                self.set_content_and_viewport_len(content_len, viewport_len);
            }
            (Some(content_len), None) => {
                self.content_len = Some(content_len);
            }
            (None, _) => {
                self.content_len = None;
            }
        }
    }

    pub fn set_content_and_viewport_len(&mut self, content_len: usize, viewport_len: usize) {
        self.content_len = Some(content_len);
        self.viewport_len = Some(viewport_len);

        if content_len <= viewport_len {
            self.scrollbar_state =
                self.scrollbar_state.viewport_content_length(1).content_length(1);
        } else {
            self.scrollbar_state = self
                .scrollbar_state
                .viewport_content_length(viewport_len)
                .content_length(1 + content_len - viewport_len);
        }
    }

    /// Scrolls to a specific percentage of the content.
    /// `perc` must be between 0.0 and 1.0
    pub fn scroll_to(&mut self, perc: f64, scrolloff: usize) {
        debug_assert!((0.0..=1.0).contains(&perc), "Percentage must be between 0.0 and 1.0");

        let Some(viewport_len) = self.viewport_len else {
            return;
        };
        let Some(content_len) = self.content_len else {
            return;
        };

        let max_offset = content_len.saturating_sub(viewport_len) as f64;
        let new_offset = (perc * max_offset).floor() as usize;

        self.inner.set_offset(new_offset);
        self.scrollbar_state = self.scrollbar_state.position(new_offset);
        self.clamp_to_offset(scrolloff);
    }

    pub fn content_len(&self) -> Option<usize> {
        self.content_len
    }

    pub fn first(&mut self) {
        if self.content_len.is_some_and(|v| v > 0) {
            self.select(Some(0), 0);
        } else {
            self.select(None, 0);
        }
    }

    pub fn last(&mut self) {
        if let Some(item_count) = self.content_len {
            if item_count > 0 {
                self.select(Some(item_count.saturating_sub(1)), 0);
            } else {
                self.select(None, 0);
            }
        } else {
            self.select(None, 0);
        }
    }

    pub fn next(&mut self, scrolloff: usize, wrap: bool) {
        if wrap {
            self.next_wrapping(scrolloff);
        } else {
            self.next_non_wrapping(scrolloff);
        }
    }

    pub fn prev(&mut self, scrolloff: usize, wrap: bool) {
        if wrap {
            self.prev_wrapping(scrolloff);
        } else {
            self.prev_non_wrapping(scrolloff);
        }
    }

    fn prev_non_wrapping(&mut self, scrolloff: usize) {
        if let Some(item_count) = self.content_len {
            match self.get_selected() {
                Some(0) => {
                    self.select(Some(0), scrolloff);
                }
                Some(i) => {
                    self.select(Some(i.saturating_sub(1)), scrolloff);
                }
                None if item_count > 0 => {
                    self.select(Some(item_count.saturating_sub(1)), scrolloff);
                }
                None => self.select(None, scrolloff),
            }
        }
    }

    fn next_non_wrapping(&mut self, scrolloff: usize) {
        if let Some(item_count) = self.content_len {
            match self.get_selected() {
                Some(i) if i == item_count.saturating_sub(1) => {
                    self.select(Some(item_count.saturating_sub(1)), scrolloff);
                }
                Some(i) => {
                    self.select(Some(i + 1), scrolloff);
                }
                None if item_count > 0 => self.select(Some(0), scrolloff),
                None => self.select(None, scrolloff),
            }
        }
    }

    fn next_wrapping(&mut self, scrolloff: usize) {
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
            self.select(i, scrolloff);
        } else {
            self.select(None, scrolloff);
        }
    }

    fn prev_wrapping(&mut self, scrolloff: usize) {
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
            self.select(i, scrolloff);
        } else {
            self.select(None, scrolloff);
        }
    }

    pub fn next_half_viewport(&mut self, scrolloff: usize) {
        if let Some(item_count) = self.content_len {
            if let Some(viewport) = self.viewport_len {
                self.select(
                    self.get_selected()
                        .map(|i| i.saturating_add(viewport / 2).min(item_count.saturating_sub(1))),
                    scrolloff,
                );
            } else {
                self.select(None, scrolloff);
            }
        } else {
            self.select(None, scrolloff);
        }
    }

    pub fn prev_half_viewport(&mut self, scrolloff: usize) {
        if self.content_len.is_some() {
            if let Some(viewport) = self.viewport_len {
                self.select(
                    self.get_selected().map(|i| i.saturating_sub(viewport / 2).max(0)),
                    scrolloff,
                );
            } else {
                self.select(None, scrolloff);
            }
        } else {
            self.select(None, scrolloff);
        }
    }

    pub fn next_viewport(&mut self, scrolloff: usize) {
        if let Some(item_count) = self.content_len {
            if let Some(viewport) = self.viewport_len {
                self.select(
                    self.get_selected()
                        .map(|i| i.saturating_add(viewport).min(item_count.saturating_sub(1))),
                    scrolloff,
                );
            } else {
                self.select(None, scrolloff);
            }
        } else {
            self.select(None, scrolloff);
        }
    }

    pub fn prev_viewport(&mut self, scrolloff: usize) {
        if self.content_len.is_some() {
            if let Some(viewport) = self.viewport_len {
                self.select(
                    self.get_selected().map(|i| i.saturating_sub(viewport).max(0)),
                    scrolloff,
                );
            } else {
                self.select(None, scrolloff);
            }
        } else {
            self.select(None, scrolloff);
        }
    }

    pub fn scroll_up(&mut self, amount: usize, scrolloff: usize) {
        let Some(_content_len) = self.content_len else {
            return;
        };

        let old_offset = self.offset();
        let new_offset = self.offset().saturating_sub(amount);
        if old_offset == new_offset && new_offset == 0 {
            return;
        }

        self.inner.set_offset(new_offset);
        self.scrollbar_state = self.scrollbar_state.position(new_offset);
        self.clamp_to_offset(scrolloff);
    }

    pub fn scroll_down(&mut self, amount: usize, scrolloff: usize) {
        let Some(viewport_len) = self.viewport_len else {
            return;
        };
        let Some(content_len) = self.content_len else {
            return;
        };

        let old_offset = self.offset();
        let max_offset = content_len.saturating_sub(viewport_len);
        let new_offset = (old_offset + amount).min(max_offset);

        if new_offset == old_offset {
            return;
        }

        self.inner.set_offset(new_offset);
        self.scrollbar_state = self.scrollbar_state.position(new_offset);
        self.clamp_to_offset(scrolloff);
    }

    pub fn clamp_to_offset(&mut self, scrolloff: usize) {
        let Some(viewport_len) = self.viewport_len else {
            return;
        };

        let offset = self.offset();
        let Some(selected) = self.get_selected() else {
            return;
        };

        if selected > (offset + viewport_len).saturating_sub(scrolloff + 1) {
            self.select(Some(offset + viewport_len - scrolloff - 1), scrolloff);
        } else if selected < offset + scrolloff {
            self.select(Some(offset + scrolloff), scrolloff);
        }
    }

    pub fn select(&mut self, idx: Option<usize>, scrolloff: usize) {
        let content_len = self.content_len.unwrap_or_default();
        let idx = idx.map(|idx| idx.max(0).min(content_len.saturating_sub(1)));
        self.inner.select_scrolling(idx);
        self.apply_scrolloff(scrolloff);

        self.scrollbar_state = self.scrollbar_state.position(self.offset());
    }

    fn apply_scrolloff(&mut self, scrolloff: usize) {
        let viewport_len = self.viewport_len.unwrap_or_default();
        let offset = self.inner.offset();
        let idx = self.get_selected().unwrap_or_default();
        let content_len = self.content_len.unwrap_or_default();
        let max_offset = content_len.saturating_sub(viewport_len);

        // Always place cursor in the middle of the screen when scrolloff is too
        // big
        if scrolloff.saturating_mul(2) >= viewport_len {
            self.inner.set_offset(idx.saturating_sub(viewport_len / 2).min(max_offset));
            return;
        }

        let scrolloff_start_down =
            (offset.saturating_add(viewport_len)).saturating_sub(scrolloff.saturating_add(1));
        if idx > scrolloff_start_down {
            let new_offset =
                (offset.saturating_add(idx.saturating_sub(scrolloff_start_down))).min(max_offset);
            self.inner.set_offset(new_offset);
            return;
        }

        if idx < offset.saturating_add(scrolloff) {
            self.inner.set_offset(
                offset.saturating_sub((offset.saturating_add(scrolloff)).saturating_sub(idx)),
            );
            return;
        }
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
        if self.marked.contains(&idx) { self.marked.remove(&idx) } else { self.marked.insert(idx) }
    }

    pub fn invert_marked(&mut self) {
        let Some(content_len) = self.content_len else {
            log::warn!("Failed to invert marked items because content length is None");
            return;
        };
        let all = (0..content_len).collect::<BTreeSet<usize>>();
        self.marked = all.difference(&self.marked).copied().collect();
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

    pub fn get_at_rendered_row(&self, row: usize) -> Option<usize> {
        let offset = self.inner.offset();
        let idx_to_select = row + offset;

        // to not select last song if clicking on an empty space after table
        if self.content_len().is_some_and(|len| idx_to_select < len) {
            Some(idx_to_select)
        } else {
            None
        }
    }

    pub fn offset(&self) -> usize {
        self.inner.offset()
    }

    pub fn set_offset(&mut self, offset: usize) {
        self.inner.set_offset(offset);
        self.scrollbar_state = self.scrollbar_state.position(offset);
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

        use crate::ui::dirstack::DirState;

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

        use crate::ui::dirstack::DirState;

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

        use crate::ui::dirstack::DirState;

        #[test]
        fn when_content_is_none() {
            let mut subject: DirState<ListState> = DirState::default();
            subject.set_content_len(None);

            subject.next(0, true);

            assert_eq!(subject.get_selected(), None);
        }

        #[test]
        fn when_content_is_zero() {
            let mut subject: DirState<ListState> = DirState::default();
            subject.set_content_len(Some(0));

            subject.next(0, true);

            assert_eq!(subject.get_selected(), None);
        }

        #[test]
        fn switches_to_first_item_when_nothing_is_selected() {
            let mut subject: DirState<ListState> = DirState::default();
            subject.set_content_len(Some(10));
            subject.select(None, 0);

            subject.next(0, true);

            assert_eq!(subject.get_selected(), Some(0));
        }

        #[test]
        fn switches_to_next_item() {
            let mut subject: DirState<ListState> = DirState::default();
            subject.set_content_len(Some(10));
            subject.select(Some(5), 0);

            subject.next(0, true);

            assert_eq!(subject.get_selected(), Some(6));
        }

        #[test]
        fn wraps_around() {
            let mut subject: DirState<ListState> = DirState::default();
            subject.set_content_len(Some(10));
            subject.select(Some(9), 0);

            subject.next(0, true);

            assert_eq!(subject.get_selected(), Some(0));
        }
    }

    mod prev {
        use ratatui::widgets::ListState;

        use crate::ui::dirstack::DirState;

        #[test]
        fn when_content_is_none() {
            let mut subject: DirState<ListState> = DirState::default();
            subject.set_content_len(None);

            subject.prev(0, true);

            assert_eq!(subject.get_selected(), None);
        }

        #[test]
        fn when_content_is_zero() {
            let mut subject: DirState<ListState> = DirState::default();
            subject.set_content_len(Some(0));

            subject.prev(0, true);

            assert_eq!(subject.get_selected(), None);
        }

        #[test]
        fn switches_to_last_item_when_nothing_is_selected() {
            let mut subject: DirState<ListState> = DirState::default();
            subject.set_content_len(Some(10));
            subject.select(None, 0);

            subject.prev(0, true);

            assert_eq!(subject.get_selected(), Some(9));
        }

        #[test]
        fn switches_to_prev_item() {
            let mut subject: DirState<ListState> = DirState::default();
            subject.set_content_len(Some(10));
            subject.select(Some(5), 0);

            subject.prev(0, true);

            assert_eq!(subject.get_selected(), Some(4));
        }

        #[test]
        fn wraps_around() {
            let mut subject: DirState<ListState> = DirState::default();
            subject.set_content_len(Some(10));
            subject.select(Some(0), 0);

            subject.prev(0, true);

            assert_eq!(subject.get_selected(), Some(9));
        }
    }

    mod next_half_viewport {
        use ratatui::widgets::ListState;

        use crate::ui::dirstack::DirState;

        #[test]
        fn when_content_is_none() {
            let mut subject: DirState<ListState> = DirState::default();
            subject.set_content_len(None);
            subject.set_viewport_len(Some(5));

            subject.next_half_viewport(0);

            assert_eq!(subject.get_selected(), None);
        }

        #[test]
        fn when_viewport_is_none() {
            let mut subject: DirState<ListState> = DirState::default();
            subject.set_content_len(Some(5));
            subject.set_viewport_len(None);

            subject.next_half_viewport(0);

            assert_eq!(subject.get_selected(), None);
        }

        #[test]
        fn goes_forward_by_half_viewport() {
            let mut subject: DirState<ListState> = DirState::default();
            subject.set_content_len(Some(20));
            subject.set_viewport_len(Some(10));
            subject.select(Some(8), 0);

            subject.next_half_viewport(0);

            assert_eq!(subject.get_selected(), Some(13));
        }

        #[test]
        fn caps_at_last_item() {
            let mut subject: DirState<ListState> = DirState::default();
            subject.set_content_len(Some(20));
            subject.set_viewport_len(Some(10));
            subject.select(Some(16), 0);

            subject.next_half_viewport(0);

            assert_eq!(subject.get_selected(), Some(19));
        }
    }

    mod prev_half_viewport {
        use ratatui::widgets::ListState;

        use crate::ui::dirstack::DirState;

        #[test]
        fn when_content_is_none() {
            let mut subject: DirState<ListState> = DirState::default();
            subject.set_content_len(None);
            subject.set_viewport_len(Some(5));

            subject.prev_half_viewport(0);

            assert_eq!(subject.get_selected(), None);
        }

        #[test]
        fn when_viewport_is_none() {
            let mut subject: DirState<ListState> = DirState::default();
            subject.set_content_len(Some(5));
            subject.set_viewport_len(None);

            subject.prev_half_viewport(0);

            assert_eq!(subject.get_selected(), None);
        }

        #[test]
        fn goes_forward_by_half_viewport() {
            let mut subject: DirState<ListState> = DirState::default();
            subject.set_content_len(Some(20));
            subject.set_viewport_len(Some(10));
            subject.select(Some(8), 0);

            subject.prev_half_viewport(0);

            assert_eq!(subject.get_selected(), Some(3));
        }

        #[test]
        fn caps_at_first_item() {
            let mut subject: DirState<ListState> = DirState::default();
            subject.set_content_len(Some(20));
            subject.set_viewport_len(Some(10));
            subject.select(Some(4), 0);

            subject.prev_half_viewport(0);

            assert_eq!(subject.get_selected(), Some(0));
        }
    }

    mod select {

        use ratatui::widgets::ListState;

        use crate::ui::dirstack::DirState;

        #[test]
        fn select_last_element_when_out_of_bounds() {
            let mut subject: DirState<ListState> = DirState::default();
            subject.set_content_len(Some(100));

            subject.select(Some(150), 0);

            assert_eq!(subject.get_selected(), Some(99));
        }
    }

    mod remove {
        use std::collections::BTreeSet;

        use ratatui::widgets::ListState;

        use crate::ui::dirstack::DirState;

        #[test]
        fn does_nothing_when_no_content() {
            let mut subject: DirState<ListState> = DirState::default();
            subject.set_content_len(Some(100));
            subject.select(Some(50), 0);
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
            subject.select(Some(50), 0);
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
            subject.select(Some(50), 0);
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
            subject.select(Some(99), 0);

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

        use crate::ui::dirstack::DirState;

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

        #[test]
        fn inverts_marked() {
            let mut subject: DirState<ListState> =
                DirState { content_len: Some(20), ..DirState::default() };
            subject.mark(5);
            subject.mark(10);
            subject.mark(15);
            assert_eq!(subject.marked, BTreeSet::from([5, 10, 15]));

            subject.invert_marked();

            assert_eq!(
                subject.marked,
                BTreeSet::from([0, 1, 2, 3, 4, 6, 7, 8, 9, 11, 12, 13, 14, 16, 17, 18, 19])
            );
        }
    }

    mod scrolloff {
        use ratatui::widgets::ListState;

        use crate::ui::dirstack::{DirState, ScrollingState};

        #[test]
        fn big_scrolloff_should_keep_cursor_in_the_middle() {
            let scrolloff = 999;
            let mut subject: DirState<ListState> = DirState::default();
            subject.set_content_len(Some(100));
            subject.set_viewport_len(Some(10));

            subject.select(Some(50), scrolloff);

            assert_eq!(subject.inner.offset(), 45);
        }

        #[test]
        fn should_not_apply_scrolloff_top_at_edge() {
            let scrolloff = 5;
            let mut subject: DirState<ListState> = DirState::default();
            subject.set_content_len(Some(100));
            subject.set_viewport_len(Some(20));
            subject.inner.set_offset(35);

            subject.select(Some(40), scrolloff);

            assert_eq!(subject.inner.offset(), 35);
        }

        #[test]
        fn should_apply_scrolloff_top() {
            let scrolloff = 5;
            let mut subject: DirState<ListState> = DirState::default();
            subject.set_content_len(Some(100));
            subject.set_viewport_len(Some(20));
            subject.inner.set_offset(35);

            subject.select(Some(37), scrolloff);

            assert_eq!(subject.inner.offset(), 32);
        }

        #[test]
        fn should_not_apply_scrolloff_bottom_at_edge() {
            let scrolloff = 5;
            let mut subject: DirState<ListState> = DirState::default();
            subject.set_content_len(Some(100));
            subject.set_viewport_len(Some(20));
            subject.inner.set_offset(55);

            subject.select(Some(69), scrolloff);

            assert_eq!(subject.inner.offset(), 55);
        }

        #[test]
        fn should_apply_scrolloff_bottom() {
            let scrolloff = 5;
            let mut subject: DirState<ListState> = DirState::default();
            subject.set_content_len(Some(100));
            subject.set_viewport_len(Some(20));
            subject.inner.set_offset(55);

            subject.select(Some(72), scrolloff);

            assert_eq!(subject.inner.offset(), 58);
        }

        #[test]
        fn scrolloff_does_not_put_offset_out_of_bounds() {
            let scrolloff = 5;
            let mut subject: DirState<ListState> = DirState::default();
            subject.set_content_len(Some(100));
            subject.set_viewport_len(Some(20));
            subject.inner.set_offset(55);

            subject.select(Some(0), scrolloff);
            assert_eq!(subject.inner.offset(), 0);

            subject.select(Some(99), scrolloff);
            assert_eq!(subject.inner.offset(), 80);
        }
    }

    mod scroll {
        use ratatui::widgets::ListState;
        use rstest::rstest;

        use crate::ui::dirstack::DirState;

        #[rstest]
        #[case(1, 0, 0, 1, 1)]
        #[case(3, 0, 0, 3, 3)]
        #[case(3, 0, 6, 3, 6)]
        #[case(1, 3, 0, 1, 4)]
        fn scroll_down(
            #[case] amount: usize,
            #[case] scrolloff: usize,
            #[case] initial_selected: usize,
            #[case] expected_offset: usize,
            #[case] expected_selected: usize,
        ) {
            let mut subject: DirState<ListState> = DirState::default();
            subject.set_content_len(Some(100));
            subject.set_viewport_len(Some(10));
            subject.select(Some(initial_selected), scrolloff);

            subject.scroll_down(amount, scrolloff);

            assert_eq!(subject.offset(), expected_offset);
            assert_eq!(subject.get_selected(), Some(expected_selected));
        }

        #[rstest]
        #[case(1, 0, 99, 89, 98)]
        #[case(3, 0, 99, 87, 96)]
        #[case(3, 0, 96, 84, 93)]
        #[case(1, 3, 99, 89, 95)]
        fn scroll_up(
            #[case] amount: usize,
            #[case] scrolloff: usize,
            #[case] initial_selected: usize,
            #[case] expected_offset: usize,
            #[case] expected_selected: usize,
        ) {
            let mut subject: DirState<ListState> = DirState::default();
            subject.set_content_len(Some(100));
            subject.set_viewport_len(Some(10));
            subject.select(Some(initial_selected), scrolloff);

            subject.scroll_up(amount, scrolloff);

            assert_eq!(subject.offset(), expected_offset);
            assert_eq!(subject.get_selected(), Some(expected_selected));
        }
    }
}
