use std::collections::HashMap;

use super::{DirStackItem, dir::Dir, state::DirState};
use crate::ui::dirstack::{ScrollingState, path::Path};

#[derive(Debug)]
pub struct DirStack<T, S>
where
    T: std::fmt::Debug + DirStackItem + Clone + Send,
    S: ScrollingState + std::fmt::Debug + Default,
{
    path: Path,
    pub dirs: HashMap<Path, Dir<T, S>>,
    empty: Dir<T, S>,
}

impl<T, S> Default for DirStack<T, S>
where
    T: std::fmt::Debug + DirStackItem + Clone + Send,
    S: ScrollingState + std::fmt::Debug + Default,
{
    fn default() -> Self {
        DirStack::new(Vec::default())
    }
}

#[allow(dead_code)]
impl<T, S> DirStack<T, S>
where
    T: std::fmt::Debug + DirStackItem + Clone + Send,
    S: ScrollingState + std::fmt::Debug + Default,
{
    pub fn new(root: Vec<T>) -> Self {
        let mut result =
            Self { dirs: HashMap::new(), path: Path::new(), empty: Dir::new(Vec::new()) };

        result.dirs.insert(result.path.clone(), Dir::new(root));
        result
    }

    pub fn len(&self) -> usize {
        self.dirs.len()
    }

    pub fn current(&self) -> &Dir<T, S> {
        self.dirs.get(&self.path).unwrap_or(&self.empty)
    }

    pub fn current_mut(&mut self) -> &mut Dir<T, S> {
        self.dirs.get_mut(&self.path).unwrap_or(&mut self.empty)
    }

    pub fn previous(&self) -> Option<&Dir<T, S>> {
        // If path is empty, meaning we are at root, there is no previous directory...
        if self.path.is_empty() {
            None
        } else {
            let mut path = self.path.clone();
            path.pop();
            self.dirs.get(&path)
        }
    }

    pub fn previous_mut(&mut self) -> Option<&mut Dir<T, S>> {
        // If path is empty, meaning we are at root, there is no previous directory...
        if self.path.is_empty() {
            None
        } else {
            let mut path = self.path.clone();
            path.pop();
            self.dirs.get_mut(&path)
        }
    }

    pub fn next(&self) -> Option<&Dir<T, S>> {
        self.next_path().and_then(|path| self.dirs.get(&path))
    }

    pub fn next_mut(&mut self) -> Option<&mut Dir<T, S>> {
        self.next_path().and_then(|path| self.dirs.get_mut(&path))
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn next_path(&self) -> Option<Path> {
        self.current().selected().map(DirStackItem::as_path).map(|current| self.path.join(current))
    }

    // Returns items of the directory that is pointed to by the currently selected
    // item if any
    pub fn next_dir_items(&self) -> Option<&Vec<T>> {
        self.next_path().and_then(|path| self.dirs.get(&path).map(|d| &d.items))
    }

    pub fn insert(&mut self, path: Path, items: Vec<T>) {
        let mut new_state = DirState::default();
        if !items.is_empty() {
            new_state.select(Some(0), 0);
        }
        new_state.set_content_len(Some(items.len()));

        self.dirs.insert(path, Dir::new_with_state(items, new_state));
    }

    pub fn enter(&mut self) {
        if let Some(next_path) = self.next_path() {
            self.path = next_path;
            // Ensure that the new path exists even if empty - it might get filled
            // asynchronously
            if !self.dirs.contains_key(&self.path) {
                self.dirs.insert(self.path.clone(), Dir::default());
            }
        } else {
            log::error!(stack:? = self; "Cannot enter because next path is not available");
        }
    }

    pub fn leave(&mut self) -> bool {
        if self.path.is_empty() {
            false
        } else {
            self.path.pop();
            true
        }
    }
}

#[cfg(test)]
mod tests {

    mod new {
        use ratatui::widgets::ListState;

        use crate::ui::dirstack::DirStack;

        #[test]
        fn creates_with_correct_current() {
            let input = vec!["test".to_owned()];

            let result: DirStack<String, ListState> = DirStack::new(input.clone());

            assert_eq!(result.current().items, input);
        }

        #[test]
        fn selects_none_when_input_is_empty() {
            let input = Vec::new();

            let result: DirStack<String, ListState> = DirStack::new(input.clone());

            assert_eq!(result.current().selected(), None);
        }

        #[test]
        fn selects_first_when_input_is_not_empty() {
            let input = vec!["test".to_owned(), "test2".to_owned(), "test3".to_owned()];

            let result: DirStack<String, ListState> = DirStack::new(input.clone());

            assert_eq!(result.current().selected(), Some("test".to_owned()).as_ref());
        }
    }

    mod next_path {
        use ratatui::widgets::ListState;

        use crate::ui::dirstack::{DirStack, Path};

        #[test]
        fn returns_none_when_nothing_is_selected() {
            let input = vec!["test".to_owned(), "test2".to_owned(), "test3".to_owned()];
            let mut subject: DirStack<String, ListState> = DirStack::new(input.clone());
            subject.current_mut().state.select(None, 0);

            let result = subject.next_path();

            assert_eq!(result, None);
        }

        #[test]
        fn returns_correct_path() {
            let level1 = vec!["a".to_owned(), "b".to_owned(), "c".to_owned()];
            let mut subject: DirStack<String, ListState> = DirStack::new(level1.clone());
            subject.current_mut().state.select(Some(1), 0);
            subject.enter();
            let level2 = vec!["d".to_owned(), "e".to_owned(), "f".to_owned()];
            subject.insert("b".into(), level2);
            subject.current_mut().state.select(Some(2), 0);

            let result = subject.next_path();

            assert_eq!(result, Some(Path::from(["b", "f"])));
        }
    }

    mod leave {
        use ratatui::widgets::ListState;

        use crate::ui::dirstack::{DirStack, Path};

        #[test]
        fn enter_and_leave_alters_path_correctly() {
            let mut subject: DirStack<String, ListState> = DirStack::new(vec!["first".to_owned()]);
            subject.insert("first".into(), vec!["second".to_owned()]);
            subject.insert(["first", "second"].into(), vec!["third".to_owned()]);
            subject.insert(["first", "second", "third"].into(), vec!["fourth".to_owned()]);

            assert_eq!(subject.path(), &Path::new());

            subject.current_mut().select_idx(0, 0);
            subject.enter();
            assert_eq!(subject.path(), &Path::from(["first"]));

            subject.current_mut().select_idx(0, 0);
            subject.enter();
            assert_eq!(subject.path(), &Path::from(["first", "second"]));

            subject.current_mut().select_idx(0, 0);
            subject.enter();
            assert_eq!(subject.path(), &Path::from(["first", "second", "third"]));

            subject.leave();
            assert_eq!(subject.path(), &Path::from(["first", "second"]));
        }

        #[test]
        fn returns_false_on_root() {
            let mut val: DirStack<String, ListState> = DirStack::new(Vec::new());
            assert!(!val.leave());
        }
    }
}
