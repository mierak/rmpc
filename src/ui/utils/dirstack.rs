use std::{collections::BTreeSet, ops::SubAssign};

use ratatui::widgets::{ListItem, ListState, ScrollbarState, TableState};

#[derive(Debug)]
pub struct DirStack<T: std::fmt::Debug + MatchesSearch + AsPath> {
    current: Dir<T>,
    others: Vec<Dir<T>>,
    preview: Option<Vec<ListItem<'static>>>,
    path: Vec<String>,
    // todo move filter inside
    pub filter: Option<String>,
    pub filter_ignore_case: bool,
}

impl<T: std::fmt::Debug + MatchesSearch + AsPath> Default for DirStack<T> {
    fn default() -> Self {
        DirStack::new(Vec::default())
    }
}

#[allow(dead_code)]
impl<T: std::fmt::Debug + MatchesSearch + AsPath> DirStack<T> {
    pub fn new(root: Vec<T>) -> Self {
        let mut result = Self {
            others: Vec::new(),
            path: Vec::new(),
            current: Dir::default(),
            filter: None,
            filter_ignore_case: true,
            preview: None,
        };
        let mut root_state = DirState::default();

        result.push(Vec::new());

        if !root.is_empty() {
            root_state.select(Some(0));
            result.current.items = root;
        };

        result.current.state = root_state;
        result
    }

    /// Returns the element at the top of the stack
    pub fn current(&self) -> &Dir<T> {
        &self.current
    }

    /// Returns the element at the top of the stack
    pub fn current_mut(&mut self) -> &mut Dir<T> {
        &mut self.current
    }

    /// Returns the element at the second element from the top of the stack
    pub fn previous(&self) -> &Dir<T> {
        self.others
            .last()
            .expect("Previous items to always contain at least one item. This should have been handled in pop()")
    }

    /// Returns the element at the second element from the top of the stack
    pub fn previous_mut(&mut self) -> &mut Dir<T> {
        self.others
            .last_mut()
            .expect("Previous items to always contain at least one item. This should have been handled in pop()")
    }

    pub fn path(&self) -> &[String] {
        &self.path
    }

    pub fn next_path(&self) -> Option<Vec<String>> {
        if let Some(Some(current)) = self.current().selected().map(AsPath::as_path) {
            let mut res = self.path().to_vec();
            res.push(current.to_owned());
            Some(res)
        } else {
            None
        }
    }

    /// Returns the element at the second element from the top of the stack
    pub fn preview(&self) -> Option<&Vec<ListItem<'static>>> {
        self.preview.as_ref()
    }

    /// Returns the element at the second element from the top of the stack
    pub fn set_preview(&mut self, preview: Option<Vec<ListItem<'static>>>) -> &Self {
        self.preview = preview;
        self
    }

    pub fn push(&mut self, head: Vec<T>) {
        let mut new_state = DirState::default();
        if !head.is_empty() {
            new_state.select(Some(0));
        };

        if let Some(Some(current)) = self.current().selected().map(AsPath::as_path) {
            self.path.push(current.to_owned());
        }

        let state = std::mem::replace(&mut self.current.state, new_state);
        let items = std::mem::replace(&mut self.current.items, head);
        self.others.push(Dir { items, state });
        self.filter = None;
    }

    pub fn pop(&mut self) -> Option<Dir<T>> {
        if self.others.len() > 1 {
            self.filter = None;
            let top = self.others.pop().expect("There should always be at least two elements");
            self.path.pop();
            Some(std::mem::replace(&mut self.current, top))
        } else {
            None
        }
    }

    pub fn next(&mut self) {
        self.current.state.next();
    }

    pub fn prev(&mut self) {
        self.current.state.prev();
    }

    pub fn next_half_viewport(&mut self) {
        self.current.state.next_half_viewport();
    }

    pub fn prev_half_viewport(&mut self) {
        self.current.state.prev_half_viewport();
    }

    pub fn last(&mut self) {
        self.current.state.last();
    }

    pub fn first(&mut self) {
        self.current.state.first();
    }

    pub fn jump_next_matching(&mut self) {
        if let Some(filter) = self.filter.as_ref() {
            if let Some(selected) = self.current.state.get_selected() {
                for i in selected + 1..self.current.items.len() {
                    let s = &self.current.items[i];
                    if s.matches(filter, self.filter_ignore_case) {
                        self.current.state.select(Some(i));
                        break;
                    }
                }
            }
        }
    }

    pub fn jump_previous_matching(&mut self) {
        if let Some(filter) = self.filter.as_ref() {
            if let Some(selected) = self.current.state.get_selected() {
                for i in (0..selected).rev() {
                    let s = &self.current.items[i];
                    if s.matches(filter, self.filter_ignore_case) {
                        self.current.state.select(Some(i));
                        break;
                    }
                }
            }
        }
    }
}

#[derive(Debug)]
pub struct Dir<T: std::fmt::Debug + MatchesSearch> {
    pub items: Vec<T>,
    pub state: DirState<ListState>,
}

impl<T: std::fmt::Debug + MatchesSearch> Default for Dir<T> {
    fn default() -> Self {
        Self {
            items: Vec::default(),
            state: DirState::default(),
        }
    }
}

#[allow(dead_code)]
impl<T: std::fmt::Debug + MatchesSearch> Dir<T> {
    pub fn replace(&mut self, new_current: Vec<T>) {
        if new_current.is_empty() {
            self.state.select(None);
        } else if self.state.get_selected().is_some_and(|v| v > new_current.len() - 1) {
            self.state.select(Some(new_current.len() - 1));
        } else {
            self.state.select(Some(0));
        }
        self.state.content_len(Some(new_current.len() as u16));
        self.items = new_current;
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
}

#[derive(Debug, Default)]
pub struct DirState<T: ScrollingState> {
    scrollbar_state: ScrollbarState,
    inner: T,
    marked: BTreeSet<usize>,
    content_len: Option<u16>,
    viewport_len: Option<u16>,
}

#[allow(dead_code)]
impl<T: ScrollingState> DirState<T> {
    pub fn viewport_len(&mut self, viewport_len: Option<u16>) -> &Self {
        self.viewport_len = viewport_len;
        self.scrollbar_state = self.scrollbar_state.viewport_content_length(viewport_len.unwrap_or(0));
        self
    }

    pub fn content_len(&mut self, content_len: Option<u16>) -> &Self {
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
                self.select(Some(item_count.saturating_sub(1) as usize));
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
                    if i >= item_count.saturating_sub(1) as usize {
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
                        Some(item_count.saturating_sub(1) as usize)
                    } else {
                        Some(i - 1)
                    }
                }
                None if item_count > 0 => Some(item_count.saturating_sub(1) as usize),
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
                self.select(self.get_selected().map(|i| {
                    i.saturating_add(viewport as usize / 2)
                        .min(item_count.saturating_sub(1) as usize)
                }));
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
                self.select(
                    self.get_selected()
                        .map(|i| i.saturating_sub(viewport as usize / 2).max(0)),
                );
            } else {
                self.select(None);
            }
        } else {
            self.select(None);
        }
    }

    pub fn select(&mut self, idx: Option<usize>) {
        self.inner.select_scrolling(idx);
        self.scrollbar_state = self.scrollbar_state.position(idx.unwrap_or(0) as u16);
    }

    #[allow(clippy::comparison_chain)]
    pub fn remove(&mut self, idx: usize) {
        match self.content_len {
            Some(len) if idx >= len.into() => return,
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
                let len: usize = (*len).into();
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

pub trait ScrollingState {
    fn select_scrolling(&mut self, idx: Option<usize>);
    fn get_selected_scrolling(&self) -> Option<usize>;
}

impl ScrollingState for TableState {
    fn select_scrolling(&mut self, idx: Option<usize>) {
        self.select(idx);
    }

    fn get_selected_scrolling(&self) -> Option<usize> {
        self.selected()
    }
}

impl ScrollingState for ListState {
    fn select_scrolling(&mut self, idx: Option<usize>) {
        self.select(idx);
    }

    fn get_selected_scrolling(&self) -> Option<usize> {
        self.selected()
    }
}

pub trait MatchesSearch {
    fn matches(&self, filter: &str, ignorecase: bool) -> bool;
}

impl MatchesSearch for String {
    fn matches(&self, filter: &str, ignorecase: bool) -> bool {
        if ignorecase {
            self.to_lowercase().contains(&filter.to_lowercase())
        } else {
            self.contains(filter)
        }
    }
}

pub trait AsPath {
    fn as_path(&self) -> Option<&str>;
}

impl AsPath for String {
    fn as_path(&self) -> Option<&str> {
        Some(self)
    }
}

#[cfg(test)]
mod dirstack_tests {

    mod new {
        use crate::ui::utils::dirstack::DirStack;

        #[test]
        fn creates_with_correct_current() {
            let input = vec!["test".to_owned()];

            let result: DirStack<String> = DirStack::new(input.clone());

            assert_eq!(result.current().items, input);
        }

        #[test]
        fn selects_none_when_input_is_empty() {
            let input = Vec::new();

            let result: DirStack<String> = DirStack::new(input.clone());

            assert_eq!(result.current().selected(), None);
        }

        #[test]
        fn selects_first_when_input_is_not_empty() {
            let input = vec!["test".to_owned(), "test2".to_owned(), "test3".to_owned()];

            let result: DirStack<String> = DirStack::new(input.clone());

            assert_eq!(result.current().selected(), Some("test".to_owned()).as_ref());
        }
    }

    mod next_path {
        use crate::ui::utils::dirstack::DirStack;

        #[test]
        fn returns_none_when_nothing_is_selected() {
            let input = vec!["test".to_owned(), "test2".to_owned(), "test3".to_owned()];
            let mut subject: DirStack<String> = DirStack::new(input.clone());
            let _ = &mut subject.current_mut().state.select(None);

            let result = subject.next_path();

            assert_eq!(result, None);
        }

        #[test]
        fn returns_correct_path() {
            let input = vec!["test".to_owned(), "test2".to_owned(), "test3".to_owned()];
            let mut subject: DirStack<String> = DirStack::new(input.clone());
            let _ = &mut subject.current_mut().state.select(Some(1));
            let input = vec!["test".to_owned(), "test2".to_owned(), "test3".to_owned()];
            subject.push(input);
            let _ = &mut subject.current_mut().state.select(Some(2));

            let result = subject.next_path();

            assert_eq!(result, Some(vec!["test2".to_owned(), "test3".to_owned()]));
        }
    }

    mod push {
        use crate::ui::utils::dirstack::DirStack;

        #[test]
        fn puts_current_to_top_of_others_and_new_input_to_current() {
            let input = vec!["test".to_owned(), "test2".to_owned(), "test3".to_owned()];
            let mut subject: DirStack<String> = DirStack::new(input.clone());
            let _ = &mut subject.current_mut().state.select(Some(1));
            let input2 = vec!["test4".to_owned(), "test3".to_owned(), "test4".to_owned()];
            let _ = &mut subject.previous_mut().state.select(Some(2));

            subject.push(input2.clone());

            assert_eq!(subject.current().items, input2);
            assert_eq!(subject.current().selected(), Some(input2[2].clone()).as_ref());
            assert_eq!(subject.previous().items, input);
            assert_eq!(subject.previous().selected(), Some(input[1].clone()).as_ref());
        }
    }

    mod pop {
        use crate::ui::utils::dirstack::DirStack;

        #[test]
        fn previous_element_is_moved_to_current() {
            let mut subject: DirStack<String> = DirStack::new(Vec::new());
            let el: Vec<String> = vec!["a", "b", "c", "d"].into_iter().map(ToOwned::to_owned).collect();
            let el2: Vec<String> = vec!["e", "f", "g", "h"].into_iter().map(ToOwned::to_owned).collect();
            subject.push(el.clone());
            subject.push(el2.clone());

            subject.pop();

            assert_eq!(el, subject.current().items);
        }

        #[test]
        fn returns_the_popped_element() {
            let mut val: DirStack<String> = DirStack::new(Vec::new());
            let el: Vec<String> = vec!["a", "b", "c", "d"].into_iter().map(ToOwned::to_owned).collect();
            val.push(el.clone());

            let result = val.pop();

            assert_eq!(Some(el), result.map(|v| v.items));
        }

        #[test]
        fn leaves_at_least_one_element_in_others() {
            let mut val: DirStack<String> = DirStack::new(Vec::new());
            val.push(Vec::new());
            assert!(val.pop().is_some());
            assert!(val.pop().is_none());

            val.previous();
        }
    }

    mod jump_next_matching {
        use crate::ui::utils::dirstack::DirStack;

        #[test]
        fn jumps_by_half_viewport() {
            let mut val: DirStack<String> = DirStack::new(Vec::new());
            let el: Vec<String> = vec!["aa", "ab", "c", "ad"].into_iter().map(ToOwned::to_owned).collect();
            val.push(el.clone());
            val.current_mut().state.viewport_len(Some(2));

            val.filter = Some("a".to_string());

            val.jump_next_matching();
            assert_eq!(val.current().state.get_selected(), Some(1));

            val.jump_next_matching();
            assert_eq!(val.current().state.get_selected(), Some(3));
        }
    }

    mod jump_previous_matching {
        use crate::ui::utils::dirstack::DirStack;

        #[test]
        fn jumps_by_half_viewport() {
            let mut val: DirStack<String> = DirStack::new(Vec::new());
            let el: Vec<String> = vec!["aa", "ab", "c", "ad", "padding"]
                .into_iter()
                .map(ToOwned::to_owned)
                .collect();
            val.push(el.clone());
            val.current_mut().state.viewport_len(Some(2));
            val.current_mut().state.select(Some(4));

            val.filter = Some("a".to_string());

            val.jump_previous_matching();
            assert_eq!(val.current().state.get_selected(), Some(3));

            val.jump_previous_matching();
            assert_eq!(val.current().state.get_selected(), Some(1));
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod dir_test {
    use super::{Dir, DirState};

    fn create_subject() -> Dir<String> {
        let mut res = Dir {
            items: vec!["a", "b", "c", "d", "f"]
                .into_iter()
                .map(ToOwned::to_owned)
                .collect(),
            state: DirState::default(),
        };
        res.state.content_len(Some(res.items.len() as u16));
        res.state.viewport_len(Some(res.items.len() as u16));
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

        use crate::ui::utils::dirstack::dir_test::create_subject;

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
            subject.state.mark(3);

            subject.remove(2);

            assert_eq!(subject.marked(), &BTreeSet::from([3]));
        }
    }
}

#[cfg(test)]
mod dir_state_test {
    use ratatui::widgets::ListState;

    use super::DirState;

    #[test]
    fn viewport_len_sets_properties() {
        let mut subject: DirState<ListState> = DirState::default();

        subject.viewport_len(Some(1337));

        assert_eq!(subject.viewport_len, Some(1337));
    }

    #[test]
    fn content_len_sets_properties() {
        let mut subject: DirState<ListState> = DirState::default();

        subject.content_len(Some(1337));

        assert_eq!(subject.content_len, Some(1337));
    }

    mod first {
        use ratatui::widgets::ListState;

        use crate::ui::utils::dirstack::DirState;

        #[test]
        fn when_content_is_none() {
            let mut subject: DirState<ListState> = DirState::default();
            subject.content_len(None);

            subject.first();

            assert_eq!(subject.get_selected(), None);
        }

        #[test]
        fn when_content_is_zero() {
            let mut subject: DirState<ListState> = DirState::default();
            subject.content_len(Some(0));

            subject.first();

            assert_eq!(subject.get_selected(), None);
        }

        #[test]
        fn when_content_is_not_empty() {
            let mut subject: DirState<ListState> = DirState::default();
            subject.content_len(Some(5));

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
            subject.content_len(None);

            subject.last();

            assert_eq!(subject.get_selected(), None);
        }

        #[test]
        fn when_content_is_zero() {
            let mut subject: DirState<ListState> = DirState::default();
            subject.content_len(Some(0));

            subject.last();

            assert_eq!(subject.get_selected(), None);
        }

        #[test]
        fn when_content_is_not_empty() {
            let mut subject: DirState<ListState> = DirState::default();
            subject.content_len(Some(5));

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
            subject.content_len(None);

            subject.next();

            assert_eq!(subject.get_selected(), None);
        }

        #[test]
        fn when_content_is_zero() {
            let mut subject: DirState<ListState> = DirState::default();
            subject.content_len(Some(0));

            subject.next();

            assert_eq!(subject.get_selected(), None);
        }

        #[test]
        fn switches_to_first_item_when_nothing_is_selected() {
            let mut subject: DirState<ListState> = DirState::default();
            subject.content_len(Some(10));
            subject.select(None);

            subject.next();

            assert_eq!(subject.get_selected(), Some(0));
        }

        #[test]
        fn switches_to_next_item() {
            let mut subject: DirState<ListState> = DirState::default();
            subject.content_len(Some(10));
            subject.select(Some(5));

            subject.next();

            assert_eq!(subject.get_selected(), Some(6));
        }

        #[test]
        fn wraps_around() {
            let mut subject: DirState<ListState> = DirState::default();
            subject.content_len(Some(10));
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
            subject.content_len(None);

            subject.prev();

            assert_eq!(subject.get_selected(), None);
        }

        #[test]
        fn when_content_is_zero() {
            let mut subject: DirState<ListState> = DirState::default();
            subject.content_len(Some(0));

            subject.prev();

            assert_eq!(subject.get_selected(), None);
        }

        #[test]
        fn switches_to_last_item_when_nothing_is_selected() {
            let mut subject: DirState<ListState> = DirState::default();
            subject.content_len(Some(10));
            subject.select(None);

            subject.prev();

            assert_eq!(subject.get_selected(), Some(9));
        }

        #[test]
        fn switches_to_prev_item() {
            let mut subject: DirState<ListState> = DirState::default();
            subject.content_len(Some(10));
            subject.select(Some(5));

            subject.prev();

            assert_eq!(subject.get_selected(), Some(4));
        }

        #[test]
        fn wraps_around() {
            let mut subject: DirState<ListState> = DirState::default();
            subject.content_len(Some(10));
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
            subject.content_len(None);
            subject.viewport_len(Some(5));

            subject.next_half_viewport();

            assert_eq!(subject.get_selected(), None);
        }

        #[test]
        fn when_viewport_is_none() {
            let mut subject: DirState<ListState> = DirState::default();
            subject.content_len(Some(5));
            subject.viewport_len(None);

            subject.next_half_viewport();

            assert_eq!(subject.get_selected(), None);
        }

        #[test]
        fn goes_forward_by_half_viewport() {
            let mut subject: DirState<ListState> = DirState::default();
            subject.content_len(Some(20));
            subject.viewport_len(Some(10));
            subject.select(Some(8));

            subject.next_half_viewport();

            assert_eq!(subject.get_selected(), Some(13));
        }

        #[test]
        fn caps_at_last_item() {
            let mut subject: DirState<ListState> = DirState::default();
            subject.content_len(Some(20));
            subject.viewport_len(Some(10));
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
            subject.content_len(None);
            subject.viewport_len(Some(5));

            subject.prev_half_viewport();

            assert_eq!(subject.get_selected(), None);
        }

        #[test]
        fn when_viewport_is_none() {
            let mut subject: DirState<ListState> = DirState::default();
            subject.content_len(Some(5));
            subject.viewport_len(None);

            subject.prev_half_viewport();

            assert_eq!(subject.get_selected(), None);
        }

        #[test]
        fn goes_forward_by_half_viewport() {
            let mut subject: DirState<ListState> = DirState::default();
            subject.content_len(Some(20));
            subject.viewport_len(Some(10));
            subject.select(Some(8));

            subject.prev_half_viewport();

            assert_eq!(subject.get_selected(), Some(3));
        }

        #[test]
        fn caps_at_first_item() {
            let mut subject: DirState<ListState> = DirState::default();
            subject.content_len(Some(20));
            subject.viewport_len(Some(10));
            subject.select(Some(4));

            subject.prev_half_viewport();

            assert_eq!(subject.get_selected(), Some(0));
        }
    }
    mod remove {
        use std::collections::BTreeSet;

        use ratatui::widgets::ListState;

        use crate::ui::utils::dirstack::DirState;

        #[test]
        fn does_nothing_when_no_content() {
            let mut subject: DirState<ListState> = DirState::default();
            subject.content_len(Some(100));
            subject.select(Some(50));
            subject.mark(5);
            assert_eq!(subject.get_selected(), Some(50));
            assert_eq!(subject.marked, BTreeSet::from([5]));
            subject.content_len(None);

            subject.remove(5);

            assert_eq!(subject.get_selected(), Some(50));
            assert_eq!(subject.marked, BTreeSet::from([5]));
        }

        #[test]
        fn does_nothing_when_removing_outside_range() {
            let mut subject: DirState<ListState> = DirState::default();
            subject.content_len(Some(100));
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
            subject.content_len(Some(100));
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
            subject.content_len(Some(100));
            subject.select(Some(99));

            subject.remove(51);

            assert_eq!(subject.get_selected(), Some(98));
        }

        #[test]
        fn changes_length_properly() {
            let mut subject: DirState<ListState> = DirState::default();
            subject.content_len(Some(100));

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
