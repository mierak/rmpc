use crate::ui::dirstack::{Dir, DirStack, DirStackItem, Path, ScrollingState};

pub struct Walk<'a, T, S>
where
    T: std::fmt::Debug + DirStackItem + Clone + Send,
    S: ScrollingState + std::fmt::Debug + Default,
{
    stack: &'a DirStack<T, S>,
    dir: Option<&'a Dir<T, S>>,
    item: Option<&'a T>,
    walker: Option<Box<Walk<'a, T, S>>>,
    path: Path,
    idx: usize,
}

pub trait WalkDirStackItem<'a, T, S>
where
    T: std::fmt::Debug + DirStackItem + Clone + Send,
    S: ScrollingState + std::fmt::Debug + Default,
{
    fn walk(&'a self, stack: &'a DirStack<T, S>, path: Path) -> Walk<'a, T, S>;
}

impl<'a, T, S> WalkDirStackItem<'a, T, S> for Dir<T, S>
where
    T: std::fmt::Debug + DirStackItem + Clone + Send,
    S: ScrollingState + std::fmt::Debug + Default,
{
    fn walk(&'a self, stack: &'a DirStack<T, S>, path: Path) -> Walk<'a, T, S> {
        let dir = stack.get(&path);
        Walk { stack, dir, item: None, walker: None, path, idx: 0 }
    }
}

impl<'a, T, S> WalkDirStackItem<'a, T, S> for T
where
    T: std::fmt::Debug + DirStackItem + Clone + Send,
    S: ScrollingState + std::fmt::Debug + Default,
{
    fn walk(&'a self, stack: &'a DirStack<T, S>, path: Path) -> Walk<'a, T, S> {
        if self.is_file() {
            Walk { stack, dir: None, item: Some(self), walker: None, path, idx: 0 }
        } else {
            let path = path.join(self.as_path());
            let dir = stack.get(&path);
            Walk { stack, dir, item: None, walker: None, path, idx: 0 }
        }
    }
}

#[allow(dead_code)]
impl<T, S> DirStack<T, S>
where
    T: std::fmt::Debug + DirStackItem + Clone + Send,
    S: ScrollingState + std::fmt::Debug + Default,
{
    pub fn walk_current(&self) -> Walk<'_, T, S> {
        self.current().walk(self, self.path().clone())
    }

    pub fn walk_dir(&self, path: Path) -> Walk<'_, T, S> {
        let dir = self.get(&path);
        Walk { stack: self, dir, item: None, walker: None, path, idx: 0 }
    }
}

impl<'a, T, S> Iterator for Walk<'a, T, S>
where
    T: std::fmt::Debug + DirStackItem + Clone + Send,
    S: ScrollingState + std::fmt::Debug + Default,
{
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(item) = self.item.take() {
            return Some(item);
        }

        if let Some(walker) = &mut self.walker {
            if let Some(item) = walker.next() {
                return Some(item);
            }

            self.walker = None;
        }

        let dir = self.dir?;

        if let Some(item) = dir.items.get(self.idx) {
            self.idx += 1;
            if item.is_file() {
                return Some(item);
            }

            let subpath = self.path.join(item.as_path());
            let subdir = self.stack.get(&subpath);
            self.walker = subdir.map(|subdir| Box::new(subdir.walk(self.stack, subpath)));
            return self.next();
        }

        return None;
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod test {
    use itertools::Itertools;

    use crate::{
        mpd::commands::Song,
        ui::{
            dir_or_song::DirOrSong,
            dirstack::{DirStack, DirStackItem, ListState, WalkDirStackItem},
        },
    };

    fn song(name: &str) -> DirOrSong {
        DirOrSong::Song(Song { file: name.to_owned(), ..Default::default() })
    }

    #[test]
    fn only_songs_are_listed() {
        let stack = DirStack::<DirOrSong, ListState>::new(vec![
            DirOrSong::name_only("dirA".to_owned()),
            DirOrSong::name_only("dirB".to_owned()),
            song("song1"),
            DirOrSong::name_only("dirC".to_owned()),
            song("song2"),
            DirOrSong::name_only("dirD".to_owned()),
            song("song3"),
            song("song4"),
        ]);

        let result = stack.walk_current().collect_vec();

        assert_eq!(result, vec![&song("song1"), &song("song2"), &song("song3"), &song("song4"),]);
    }

    #[test]
    fn songs_in_subdirs_are_listed() {
        let mut stack = DirStack::<DirOrSong, ListState>::new(vec![
            DirOrSong::name_only("dirA".to_owned()),
            DirOrSong::name_only("dirB".to_owned()),
            song("song1"),
            DirOrSong::name_only("dirC".to_owned()),
            song("song2"),
            DirOrSong::name_only("dirD".to_owned()),
            song("song3"),
            song("song4"),
        ]);

        stack.insert(["dirA"].into(), vec![song("songA1"), song("songA2")]);
        stack.insert(["dirB"].into(), vec![DirOrSong::name_only("dirB1".to_owned())]);
        stack.insert(["dirB", "dirB1"].into(), vec![song("songB1a"), song("songB1b")]);
        stack.insert(["dirC"].into(), vec![]);
        stack.insert(["dirD"].into(), vec![song("songD1")]);

        let result = stack.walk_current().collect_vec();

        assert_eq!(result, vec![
            &song("songA1"),
            &song("songA2"),
            &song("songB1a"),
            &song("songB1b"),
            &song("song1"),
            &song("song2"),
            &song("songD1"),
            &song("song3"),
            &song("song4"),
        ],);
    }

    #[test]
    fn walk_nonexistent_dir_yields_no_songs() {
        let stack = DirStack::<DirOrSong, ListState>::new(vec![
            DirOrSong::name_only("dirA".to_owned()),
            DirOrSong::name_only("dirB".to_owned()),
            song("song1"),
            DirOrSong::name_only("dirC".to_owned()),
            song("song2"),
            DirOrSong::name_only("dirD".to_owned()),
            song("song3"),
            song("song4"),
        ]);

        let result = stack.walk_dir(["nonexistent"].into()).collect_vec();

        assert!(result.is_empty());
    }

    #[test]
    fn walk_in_subdir_songs_are_listed() {
        let mut stack = DirStack::<DirOrSong, ListState>::new(vec![
            DirOrSong::name_only("dirA".to_owned()),
            DirOrSong::name_only("dirB".to_owned()),
            song("song1"),
            DirOrSong::name_only("dirC".to_owned()),
            song("song2"),
            DirOrSong::name_only("dirD".to_owned()),
            song("song3"),
            song("song4"),
        ]);

        stack.insert(["dirA"].into(), vec![song("songA1"), song("songA2")]);
        stack.insert(["dirB"].into(), vec![
            DirOrSong::name_only("dirB1".to_owned()),
            song("songBa"),
        ]);
        stack.insert(["dirB", "dirB1"].into(), vec![song("songB1a"), song("songB1b")]);
        stack.insert(["dirC"].into(), vec![]);
        stack.insert(["dirD"].into(), vec![song("songD1")]);

        let result = stack.walk_dir(["dirB"].into()).collect_vec();

        assert_eq!(result, vec![&song("songB1a"), &song("songB1b"), &song("songBa")],);
    }

    #[test]
    fn empty_stack_yields_no_songs() {
        let stack = DirStack::<DirOrSong, ListState>::new(vec![]);

        let result = stack.walk_current().collect_vec();

        assert!(result.is_empty());
    }

    #[test]
    fn stack_with_only_dirs_yields_no_songs() {
        let stack = DirStack::<DirOrSong, ListState>::new(vec![
            DirOrSong::name_only("dirA".to_owned()),
            DirOrSong::name_only("dirB".to_owned()),
            DirOrSong::name_only("dirC".to_owned()),
        ]);

        let result = stack.walk_current().collect_vec();

        assert!(result.is_empty());
    }

    #[test]
    fn stack_with_only_empty_dirs_yields_no_songs() {
        let mut stack = DirStack::<DirOrSong, ListState>::new(vec![
            DirOrSong::name_only("dirA".to_owned()),
            DirOrSong::name_only("dirB".to_owned()),
            DirOrSong::name_only("dirC".to_owned()),
        ]);

        stack.insert(["dirA"].into(), vec![]);
        stack.insert(["dirB"].into(), vec![]);
        stack.insert(["dirC"].into(), vec![]);

        let result = stack.walk_current().collect_vec();

        assert!(result.is_empty());
    }

    #[test]
    fn single_song_is_listed() {
        let stack = DirStack::<DirOrSong, ListState>::new(vec![song("onlySong")]);

        let result = stack.walk_current().collect_vec();

        assert_eq!(result, vec![&song("onlySong")]);
    }

    #[test]
    fn single_dir_with_songs_is_listed() {
        let mut stack =
            DirStack::<DirOrSong, ListState>::new(vec![DirOrSong::name_only("dirA".to_owned())]);

        stack.insert(["dirA"].into(), vec![song("songA1"), song("songA2")]);

        let result = stack.walk_current().collect_vec();

        assert_eq!(result, vec![&song("songA1"), &song("songA2")]);
    }

    #[test]
    fn walk_called_on_file_yields_only_that_file() {
        let mut stack = DirStack::<DirOrSong, ListState>::new(vec![
            DirOrSong::name_only("dirA".to_owned()),
            song("song1"),
            DirOrSong::name_only("dirB".to_owned()),
        ]);
        stack.insert(["dirA"].into(), vec![song("songA1"), song("songA2")]);
        stack.insert(["dirB"].into(), vec![song("songB1")]);
        stack.current_mut().select_idx(0, 0); // select dirA
        stack.enter();
        stack.current_mut().select_idx(1, 0); //select songA2

        let result =
            stack.current().selected().unwrap().walk(&stack, stack.path().to_owned()).collect_vec();

        assert_eq!(result, vec![&song("songA2")]);
    }

    #[test]
    fn walk_called_on_empty_dir_yields_no_songs() {
        let mut stack = DirStack::<DirOrSong, ListState>::new(vec![
            DirOrSong::name_only("dirA".to_owned()),
            song("song1"),
            DirOrSong::name_only("dirB".to_owned()),
        ]);

        stack.insert(["dirA"].into(), vec![song("songA1"), song("songA2")]);
        stack.insert(["dirB"].into(), vec![]);
        stack.current_mut().select_idx(2, 0); // select dirB
        assert_eq!(stack.current().selected().unwrap().as_path(), "dirB");

        let result =
            stack.current().selected().unwrap().walk(&stack, stack.path().to_owned()).collect_vec();

        assert!(result.is_empty());
    }

    #[test]
    fn walk_called_on_dir_yields_songs_in_that_dir() {
        let mut stack = DirStack::<DirOrSong, ListState>::new(vec![
            DirOrSong::name_only("dirA".to_owned()),
            song("song1"),
            DirOrSong::name_only("dirB".to_owned()),
        ]);
        stack.insert(["dirA"].into(), vec![song("songA1"), song("songA2")]);
        stack.insert(["dirB"].into(), vec![song("songB1")]);
        stack.current_mut().select_idx(0, 0); // select dirA
        assert_eq!(stack.current().selected().unwrap().as_path(), "dirA");

        let test = stack.current().selected().unwrap();
        let result = test.walk(&stack, stack.path().to_owned()).collect_vec();

        assert_eq!(result, vec![&song("songA1"), &song("songA2")]);
    }

    #[test]
    fn walk_called_on_dir_yields_songs_in_that_dir_and_its_subdirs() {
        let mut stack = DirStack::<DirOrSong, ListState>::new(vec![
            DirOrSong::name_only("dirA".to_owned()),
            song("song1"),
            DirOrSong::name_only("dirB".to_owned()),
        ]);
        stack.insert(["dirA"].into(), vec![
            song("songA1"),
            DirOrSong::name_only("dirA1".to_owned()),
            song("songA2"),
        ]);
        stack.insert(["dirA", "dirA1"].into(), vec![song("songA1a"), song("songA1b")]);
        stack.insert(["dirB"].into(), vec![song("songB1")]);
        stack.current_mut().select_idx(0, 0); // select dirA
        assert_eq!(stack.current().selected().unwrap().as_path(), "dirA");

        let test = stack.current().selected().unwrap();
        let result = test.walk(&stack, stack.path().to_owned()).collect_vec();

        assert_eq!(result, vec![
            &song("songA1"),
            &song("songA1a"),
            &song("songA1b"),
            &song("songA2"),
        ]);
    }
}
