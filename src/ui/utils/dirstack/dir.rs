use std::collections::BTreeSet;

use log::error;
use ratatui::widgets::ListState;

use crate::utils::macros::status_warn;

use super::{state::DirState, DirStackItem};

#[derive(Debug)]
pub struct Dir<T: std::fmt::Debug + DirStackItem> {
    pub items: Vec<T>,
    pub state: DirState<ListState>,
    pub filter: Option<String>,
    pub filter_ignore_case: bool,
}

impl<T: std::fmt::Debug + DirStackItem> Default for Dir<T> {
    fn default() -> Self {
        Self {
            items: Vec::default(),
            state: DirState::default(),
            filter: None,
            filter_ignore_case: true,
        }
    }
}

#[allow(dead_code)]
impl<T: std::fmt::Debug + DirStackItem> Dir<T> {
    pub fn new(root: Vec<T>) -> Self {
        let mut result = Self {
            items: Vec::new(),
            state: DirState::default(),
            filter: None,
            filter_ignore_case: true,
        };

        if !root.is_empty() {
            result.state.select(Some(0));
            result.state.set_content_len(Some(root.len()));
            result.items = root;
        };

        result
    }

    pub fn replace(&mut self, new_current: Vec<T>) {
        if new_current.is_empty() {
            self.state.select(None);
        } else if self.state.get_selected().is_some_and(|v| v > new_current.len() - 1) {
            self.state.select(Some(new_current.len() - 1));
        } else {
            self.state.select(Some(0));
        }
        self.state.set_content_len(Some(new_current.len()));
        self.items = new_current;
    }

    pub fn to_list_items(&self, config: &crate::config::Config) -> Vec<T::Item> {
        self.items
            .iter()
            .enumerate()
            .map(|(i, item)| item.to_list_item(config, self.state.marked.contains(&i), self.filter.as_deref()))
            .collect()
    }

    pub fn selected(&self) -> Option<&T> {
        if let Some(sel) = self.state.get_selected() {
            self.items.get(sel)
        } else {
            None
        }
    }

    pub fn selected_with_idx(&self) -> Option<(&T, usize)> {
        if let Some(sel) = self.state.get_selected() {
            self.items.get(sel).map(|v| (v, sel))
        } else {
            None
        }
    }

    pub fn marked(&self) -> &BTreeSet<usize> {
        &self.state.marked
    }

    pub fn unmark_all(&mut self) {
        self.state.unmark_all();
    }

    pub fn toggle_mark_selected(&mut self) -> bool {
        if let Some(sel) = self.state.get_selected() {
            self.state.toggle_mark(sel)
        } else {
            false
        }
    }

    pub fn mark_selected(&mut self) -> bool {
        if let Some(sel) = self.state.get_selected() {
            self.state.mark(sel)
        } else {
            false
        }
    }

    pub fn unmark_selected(&mut self) -> bool {
        if let Some(sel) = self.state.get_selected() {
            self.state.unmark(sel)
        } else {
            false
        }
    }

    pub fn remove(&mut self, idx: usize) {
        if idx < self.items.len() {
            self.items.remove(idx);
        }
        self.state.remove(idx);
    }

    pub fn remove_all_marked(&mut self) {
        for i in 0..self.items.len() {
            if self.state.marked.contains(&i) {
                self.items.remove(i);
                self.state.remove(i);
            }
        }
    }

    pub fn next(&mut self) {
        self.state.next();
    }

    pub fn prev(&mut self) {
        self.state.prev();
    }

    pub fn next_half_viewport(&mut self) {
        self.state.next_half_viewport();
    }

    pub fn prev_half_viewport(&mut self) {
        self.state.prev_half_viewport();
    }

    pub fn last(&mut self) {
        self.state.last();
    }

    pub fn first(&mut self) {
        self.state.first();
    }

    pub fn jump_next_matching(&mut self) {
        let Some(filter) = self.filter.as_ref() else {
            status_warn!("No filter set");
            return;
        };
        let Some(selected) = self.state.get_selected() else {
            error!(state:? = self.state; "No song selected");
            return;
        };

        let length = self.items.len();
        for i in selected + 1..length + selected {
            let i = i % length;
            if self.items[i].matches(filter, self.filter_ignore_case) {
                self.state.select(Some(i));
                break;
            }
        }
    }

    pub fn jump_previous_matching(&mut self) {
        let Some(filter) = self.filter.as_ref() else {
            status_warn!("No filter set");
            return;
        };
        let Some(selected) = self.state.get_selected() else {
            error!(state:? = self.state; "No song selected");
            return;
        };

        let length = self.items.len();
        for i in (0..length).rev() {
            let i = (i + selected) % length;
            if self.items[i].matches(filter, self.filter_ignore_case) {
                self.state.select(Some(i));
                break;
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::{Dir, DirState};

    fn create_subject() -> Dir<String> {
        let mut res = Dir {
            items: vec!["a", "b", "c", "d", "f"]
                .into_iter()
                .map(ToOwned::to_owned)
                .collect(),
            state: DirState::default(),
            filter: None,
            filter_ignore_case: true,
        };
        res.state.set_content_len(Some(res.items.len()));
        res.state.set_viewport_len(Some(res.items.len()));
        res
    }

    mod selected {
        use super::create_subject;

        #[test]
        fn returns_none() {
            let mut subject = create_subject();
            subject.state.select(None);

            let result = subject.selected();

            assert_eq!(result, None);
        }

        #[test]
        fn returns_item() {
            let mut subject = create_subject();
            subject.state.select(Some(2));

            let result = subject.selected();

            assert_eq!(result.unwrap(), "c");
        }
    }

    mod selected_with_idx {
        use super::create_subject;

        #[test]
        fn returns_none() {
            let mut subject = create_subject();
            subject.state.select(None);

            let result = subject.selected_with_idx();

            assert_eq!(result, None);
        }

        #[test]
        fn returns_item() {
            let mut subject = create_subject();
            subject.state.select(Some(2));

            let result = subject.selected_with_idx();

            assert_eq!(result.unwrap(), (&"c".to_owned(), 2));
        }
    }

    mod toggle_mark_selected {
        use std::collections::BTreeSet;

        use super::create_subject;

        #[test]
        fn toggles_marks() {
            let mut subject = create_subject();
            subject.state.mark(2);
            subject.state.mark(1);
            subject.state.unmark(3);

            subject.state.select(Some(2));
            subject.toggle_mark_selected();
            subject.state.select(Some(3));
            subject.toggle_mark_selected();

            assert_eq!(subject.marked(), &BTreeSet::from([1, 3]));
        }
    }

    mod mark_selected {
        use std::collections::BTreeSet;

        use super::create_subject;

        #[test]
        fn does_nothing_when_none_selected() {
            let mut subject = create_subject();

            subject.mark_selected();

            assert_eq!(subject.marked(), &BTreeSet::from([]));
        }

        #[test]
        fn marks_selected() {
            let mut subject = create_subject();
            subject.state.mark(2);
            subject.state.select(Some(3));

            subject.mark_selected();

            assert_eq!(subject.marked(), &BTreeSet::from([2, 3]));
        }
    }

    mod unmark_selected {
        use std::collections::BTreeSet;

        use super::create_subject;

        #[test]
        fn does_nothing_when_none_selected() {
            let mut subject = create_subject();
            subject.state.mark(3);

            subject.unmark_selected();

            assert_eq!(subject.marked(), &BTreeSet::from([3]));
        }

        #[test]
        fn unmarks_selected() {
            let mut subject = create_subject();
            subject.state.mark(2);
            subject.state.mark(3);
            subject.state.select(Some(2));

            subject.unmark_selected();

            assert_eq!(subject.marked(), &BTreeSet::from([3]));
        }
    }

    mod replace {

        use super::create_subject;

        #[test]
        fn selects_none_when_new_state_is_empty() {
            let mut subject = create_subject();
            subject.state.select(Some(2));
            assert_eq!(subject.selected().unwrap(), "c");

            subject.replace(Vec::default());

            assert_eq!(subject.selected(), None);
        }

        #[test]
        fn selects_first_element() {
            let mut subject = create_subject();
            subject.state.select(Some(2));
            assert_eq!(subject.selected().unwrap(), "c");

            subject.replace(
                vec!["q", "w", "f", "p", "b"]
                    .into_iter()
                    .map(ToOwned::to_owned)
                    .collect(),
            );

            assert_eq!(subject.selected().unwrap(), "q");
        }

        #[test]
        fn selects_last_element_if_previous_selected_was_higher_than_new_len() {
            let mut subject = create_subject();
            subject.state.select(Some(4));
            assert_eq!(subject.selected().unwrap(), "f");

            subject.replace(vec!["q", "w"].into_iter().map(ToOwned::to_owned).collect());

            assert_eq!(subject.selected().unwrap(), "w");
        }
    }

    mod remove {
        use std::collections::BTreeSet;

        use crate::ui::utils::dirstack::dir::tests::create_subject;

        #[test]
        fn does_nothing_when_outside_range() {
            let mut subject = create_subject();
            subject.state.mark(2);
            subject.state.mark(3);

            subject.remove(5);

            assert_eq!(subject.marked(), &BTreeSet::from([2, 3]));
        }

        #[test]
        fn removes_item() {
            let mut subject = create_subject();
            subject.state.mark(2);
            subject.state.mark(4);

            subject.remove(2);

            assert_eq!(subject.marked(), &BTreeSet::from([3]));
        }
    }

    mod jump_next_matching {
        use crate::ui::utils::dirstack::Dir;

        #[test]
        fn jumps_by_half_viewport() {
            let mut val: Dir<String> = Dir {
                items: vec!["aa", "ab", "c", "ad"].into_iter().map(ToOwned::to_owned).collect(),
                ..Default::default()
            };
            val.state.set_viewport_len(Some(2));
            val.state.set_content_len(Some(val.items.len()));
            val.state.select(Some(0));

            val.filter = Some("a".to_string());

            val.jump_next_matching();
            assert_eq!(val.state.get_selected(), Some(1));

            val.jump_next_matching();
            assert_eq!(val.state.get_selected(), Some(3));
        }
    }

    mod jump_previous_matching {
        use crate::ui::utils::dirstack::Dir;

        #[test]
        fn jumps_by_half_viewport() {
            let mut val: Dir<String> = Dir {
                items: vec!["aa", "ab", "c", "ad", "padding"]
                    .into_iter()
                    .map(ToOwned::to_owned)
                    .collect(),
                ..Default::default()
            };
            val.state.set_content_len(Some(val.items.len()));
            val.state.set_viewport_len(Some(2));
            val.state.select(Some(4));

            val.filter = Some("a".to_string());

            val.jump_previous_matching();
            assert_eq!(val.state.get_selected(), Some(3));

            val.jump_previous_matching();
            assert_eq!(val.state.get_selected(), Some(1));
        }
    }
}
