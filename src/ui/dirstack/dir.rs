use std::collections::BTreeSet;

use log::error;
use ratatui::widgets::{ListItem, ListState};

use super::{DirStackItem, state::DirState};
use crate::{config::Config, shared::macros::status_warn};

#[derive(Debug)]
pub struct Dir<T: std::fmt::Debug + DirStackItem + Clone + Send> {
    pub items: Vec<T>,
    pub state: DirState<ListState>,
    filter: Option<String>,
    matched_item_count: usize,
}

impl<T: std::fmt::Debug + DirStackItem + Clone + Send> Default for Dir<T> {
    fn default() -> Self {
        Self {
            items: Vec::default(),
            state: DirState::default(),
            filter: None,
            matched_item_count: 0,
        }
    }
}

#[allow(dead_code)]
impl<T: std::fmt::Debug + DirStackItem + Clone + Send> Dir<T> {
    pub fn new(root: Vec<T>) -> Self {
        let mut result = Self {
            items: Vec::new(),
            state: DirState::default(),
            filter: None,
            matched_item_count: 0,
        };

        if !root.is_empty() {
            result.state.select(Some(0), 0);
            result.state.set_content_len(Some(root.len()));
            result.items = root;
        };

        result
    }

    pub fn new_with_state(items: Vec<T>, state: DirState<ListState>) -> Self {
        return Self { items, state, filter: None, matched_item_count: 0 };
    }

    pub fn filter(&self) -> Option<&str> {
        self.filter.as_deref()
    }

    pub fn set_filter(&mut self, value: Option<String>, config: &Config) {
        self.matched_item_count = if let Some(ref filter) = value {
            self.items.iter().filter(|item| item.matches(config, filter)).count()
        } else {
            0
        };
        self.filter = value;
    }

    pub fn push_filter(&mut self, char: char, config: &Config) {
        if let Some(ref mut filter) = self.filter {
            filter.push(char);
            self.matched_item_count =
                self.items.iter().filter(|item| item.matches(config, filter)).count();
        }
    }

    pub fn pop_filter(&mut self, config: &Config) {
        if let Some(ref mut filter) = self.filter {
            filter.pop();
            self.matched_item_count =
                self.items.iter().filter(|item| item.matches(config, filter)).count();
        }
    }

    pub fn to_list_items<'a>(&self, config: &Config) -> Vec<ListItem<'a>> {
        let mut already_matched: u32 = 0;
        let current_item_idx = self.selected_with_idx().map(|(idx, _)| idx);
        self.items
            .iter()
            .enumerate()
            .map(|(i, item)| {
                let matches = self.filter.as_ref().is_some_and(|v| item.matches(config, v));
                let is_current = current_item_idx.is_some_and(|idx| i == idx);
                if matches {
                    already_matched = already_matched.saturating_add(1);
                }
                let content = if matches && is_current {
                    Some(format!(" [{already_matched}/{}]", self.matched_item_count))
                } else {
                    None
                };
                item.to_list_item(config, self.marked().contains(&i), matches, content)
            })
            .collect()
    }

    pub fn selected(&self) -> Option<&T> {
        if let Some(sel) = self.state.get_selected() { self.items.get(sel) } else { None }
    }

    pub fn selected_mut(&mut self) -> Option<&mut T> {
        if let Some(sel) = self.state.get_selected() { self.items.get_mut(sel) } else { None }
    }

    pub fn selected_with_idx(&self) -> Option<(usize, &T)> {
        if let Some(sel) = self.state.get_selected() {
            self.items.get(sel).map(|v| (sel, v))
        } else {
            None
        }
    }

    pub fn marked_items(&self) -> impl Iterator<Item = &T> {
        self.state.marked.iter().filter_map(|idx| self.items.get(*idx))
    }

    pub fn marked(&self) -> &BTreeSet<usize> {
        &self.state.marked
    }

    pub fn marked_mut(&mut self) -> &mut BTreeSet<usize> {
        &mut self.state.marked
    }

    pub fn unmark_all(&mut self) {
        self.state.unmark_all();
    }

    pub fn invert_marked(&mut self) {
        self.state.invert_marked();
    }

    pub fn toggle_mark_selected(&mut self) -> bool {
        if let Some(sel) = self.state.get_selected() { self.state.toggle_mark(sel) } else { false }
    }

    pub fn mark_selected(&mut self) -> bool {
        if let Some(sel) = self.state.get_selected() { self.state.mark(sel) } else { false }
    }

    pub fn unmark_selected(&mut self) -> bool {
        if let Some(sel) = self.state.get_selected() { self.state.unmark(sel) } else { false }
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

    pub fn next(&mut self, scrolloff: usize, wrap: bool) {
        self.state.next(scrolloff, wrap);
    }

    pub fn prev(&mut self, scrolloff: usize, wrap: bool) {
        self.state.prev(scrolloff, wrap);
    }

    pub fn select_idx(&mut self, idx: usize, scrolloff: usize) {
        self.state.select(Some(idx), scrolloff);
    }

    pub fn next_half_viewport(&mut self, scrolloff: usize) {
        self.state.next_half_viewport(scrolloff);
    }

    pub fn prev_half_viewport(&mut self, scrolloff: usize) {
        self.state.prev_half_viewport(scrolloff);
    }

    pub fn next_viewport(&mut self, scrolloff: usize) {
        self.state.next_viewport(scrolloff);
    }

    pub fn prev_viewport(&mut self, scrolloff: usize) {
        self.state.prev_viewport(scrolloff);
    }

    pub fn last(&mut self) {
        self.state.last();
    }

    pub fn first(&mut self) {
        self.state.first();
    }

    pub fn jump_next_matching(&mut self, config: &Config) {
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
            if self.items[i].matches(config, filter) {
                self.state.select(Some(i), config.scrolloff);
                break;
            }
        }
    }

    pub fn jump_previous_matching(&mut self, config: &Config) {
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
            if self.items[i].matches(config, filter) {
                self.state.select(Some(i), config.scrolloff);
                break;
            }
        }
    }

    pub fn jump_first_matching(&mut self, config: &Config) {
        let Some(filter) = self.filter.as_ref() else {
            status_warn!("No filter set");
            return;
        };

        self.items
            .iter()
            .enumerate()
            .find(|(_, item)| item.matches(config, filter))
            .inspect(|(idx, _)| self.state.select(Some(*idx), config.scrolloff));
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::{Dir, DirState};

    fn create_subject() -> Dir<String> {
        let mut res = Dir {
            items: vec!["a", "b", "c", "d", "f"].into_iter().map(ToOwned::to_owned).collect(),
            state: DirState::default(),
            filter: None,
            matched_item_count: 0,
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
            subject.state.select(None, 0);

            let result = subject.selected();

            assert_eq!(result, None);
        }

        #[test]
        fn returns_item() {
            let mut subject = create_subject();
            subject.state.select(Some(2), 0);

            let result = subject.selected();

            assert_eq!(result.unwrap(), "c");
        }
    }

    mod selected_with_idx {
        use super::create_subject;

        #[test]
        fn returns_none() {
            let mut subject = create_subject();
            subject.state.select(None, 0);

            let result = subject.selected_with_idx();

            assert_eq!(result, None);
        }

        #[test]
        fn returns_item() {
            let mut subject = create_subject();
            subject.state.select(Some(2), 0);

            let result = subject.selected_with_idx();

            assert_eq!(result.unwrap(), (2, &"c".to_owned()));
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

            subject.state.select(Some(2), 0);
            subject.toggle_mark_selected();
            subject.state.select(Some(3), 0);
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
            subject.state.select(Some(3), 0);

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
            subject.state.select(Some(2), 0);

            subject.unmark_selected();

            assert_eq!(subject.marked(), &BTreeSet::from([3]));
        }
    }

    mod remove {
        use std::collections::BTreeSet;

        use crate::ui::dirstack::dir::tests::create_subject;

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
        use crate::{config::Config, ui::dirstack::Dir};

        #[test]
        fn jumps_by_half_viewport() {
            let mut val: Dir<String> = Dir {
                items: vec!["aa", "ab", "c", "ad"].into_iter().map(ToOwned::to_owned).collect(),
                ..Default::default()
            };
            val.state.set_viewport_len(Some(2));
            val.state.set_content_len(Some(val.items.len()));
            val.state.select(Some(0), 0);

            val.filter = Some("a".to_string());

            val.jump_next_matching(&Config::default());
            assert_eq!(val.state.get_selected(), Some(1));

            val.jump_next_matching(&Config::default());
            assert_eq!(val.state.get_selected(), Some(3));
        }
    }

    mod jump_previous_matching {
        use crate::{config::Config, ui::dirstack::Dir};

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
            val.state.select(Some(4), 0);

            val.filter = Some("a".to_string());

            val.jump_previous_matching(&Config::default());
            assert_eq!(val.state.get_selected(), Some(3));

            val.jump_previous_matching(&Config::default());
            assert_eq!(val.state.get_selected(), Some(1));
        }
    }

    mod matched_item_count {
        use crate::{config::Config, ui::dirstack::Dir};

        #[test]
        fn filter_changes_recounts_matched_items() {
            let mut val: Dir<String> = Dir {
                items: vec!["aa", "ab", "c", "ad", "padding"]
                    .into_iter()
                    .map(ToOwned::to_owned)
                    .collect(),
                filter: None,
                ..Default::default()
            };
            val.set_filter(Some("a".to_string()), &Config::default());
            assert_eq!(val.matched_item_count, 4);

            val.push_filter('d', &Config::default());
            assert_eq!(val.matched_item_count, 2);

            val.pop_filter(&Config::default());
            assert_eq!(val.matched_item_count, 4);

            val.pop_filter(&Config::default());
            assert_eq!(val.matched_item_count, 5);

            val.set_filter(None, &Config::default());
            assert_eq!(val.matched_item_count, 0);
        }
    }
}
