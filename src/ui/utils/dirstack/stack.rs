use ratatui::widgets::ListItem;

use super::{dir::Dir, state::DirState, DirStackItem};

#[derive(Debug)]
pub struct DirStack<T: std::fmt::Debug + DirStackItem> {
    current: Dir<T>,
    others: Vec<Dir<T>>,
    preview: Option<Vec<ListItem<'static>>>,
    path: Vec<String>,
}

impl<T: std::fmt::Debug + DirStackItem> Default for DirStack<T> {
    fn default() -> Self {
        DirStack::new(Vec::default())
    }
}

#[allow(dead_code)]
impl<T: std::fmt::Debug + DirStackItem> DirStack<T> {
    pub fn new(root: Vec<T>) -> Self {
        let mut result = Self {
            others: Vec::new(),
            path: Vec::new(),
            current: Dir::default(),
            preview: None,
        };
        result.push(Vec::new());
        result.current = Dir::new(root);

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
        if let Some(current) = self.current().selected().map(DirStackItem::as_path) {
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
            new_state.select(Some(0), 0);
        };
        new_state.set_content_len(Some(head.len()));

        if let Some(current) = self.current().selected().map(DirStackItem::as_path) {
            self.path.push(current.to_owned());
        }

        let old_current_dir = std::mem::replace(&mut self.current, Dir::new_with_state(head, new_state));
        self.others.push(old_current_dir);
    }

    pub fn pop(&mut self) -> Option<Dir<T>> {
        if self.others.len() > 1 {
            let top = self.others.pop().expect("There should always be at least two elements");
            self.path.pop();
            Some(std::mem::replace(&mut self.current, top))
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
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
            subject.current_mut().state.select(None, 0);

            let result = subject.next_path();

            assert_eq!(result, None);
        }

        #[test]
        fn returns_correct_path() {
            let level1 = vec!["a".to_owned(), "b".to_owned(), "c".to_owned()];
            let mut subject: DirStack<String> = DirStack::new(level1.clone());
            subject.current_mut().state.select(Some(1), 0);
            let level2 = vec!["d".to_owned(), "e".to_owned(), "f".to_owned()];
            subject.push(level2);
            subject.current_mut().state.select(Some(2), 0);

            let result = subject.next_path();

            assert_eq!(result, Some(vec!["b".to_owned(), "f".to_owned()]));
        }
    }

    mod push {
        use crate::ui::utils::dirstack::DirStack;

        #[test]
        fn puts_current_to_top_of_others_and_new_input_to_current() {
            let input = vec!["test".to_owned(), "test2".to_owned(), "test3".to_owned()];
            let mut subject: DirStack<String> = DirStack::new(input.clone());
            subject.current_mut().state.select(Some(1), 0);
            let input2 = vec!["test4".to_owned(), "test3".to_owned(), "test4".to_owned()];
            subject.previous_mut().state.select(Some(2), 0);

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
}
