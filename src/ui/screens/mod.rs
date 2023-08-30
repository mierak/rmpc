use anyhow::Result;
use async_trait::async_trait;
use crossterm::event::KeyEvent;
use ratatui::{
    prelude::{Backend, Rect},
    widgets::ListState,
    Frame,
};
use strum::{Display, EnumIter, EnumVariantNames};

use crate::{
    mpd::{client::Client, errors::MpdError},
    state::State,
};

use super::{MyState, Render, SharedUiState};

pub mod albums;
pub mod artists;
pub mod directories;
pub mod logs;
pub mod queue;

#[derive(Debug, Display, EnumVariantNames, Default, Clone, Copy, EnumIter, PartialEq)]
pub enum Screens {
    #[default]
    Queue,
    Logs,
    Directories,
    Artists,
    Albums,
}

#[async_trait]
pub trait Screen {
    fn render<B: Backend>(
        &mut self,
        frame: &mut Frame<B>,
        area: Rect,
        app: &mut crate::state::State,
        shared_state: &mut SharedUiState,
    ) -> Result<()>;

    /// For any cleanup operations, ran when the screen hides
    async fn on_hide(
        &mut self,
        _client: &mut Client<'_>,
        _app: &mut crate::state::State,
        _shared_state: &mut SharedUiState,
    ) -> Result<()> {
        Ok(())
    }

    /// For work that needs to be done BEFORE the first render
    async fn before_show(
        &mut self,
        _client: &mut Client<'_>,
        _app: &mut crate::state::State,
        _shared: &mut SharedUiState,
    ) -> Result<()> {
        Ok(())
    }

    async fn handle_key(
        &mut self,
        key: KeyEvent,
        _client: &mut Client<'_>,
        _app: &mut State,
        _shared: &mut SharedUiState,
    ) -> Result<Render, MpdError>;
}

impl Screens {
    pub fn next(&self) -> Self {
        match self {
            Screens::Queue => Screens::Logs,
            Screens::Logs => Screens::Directories,
            Screens::Directories => Screens::Artists,
            Screens::Artists => Screens::Albums,
            Screens::Albums => Screens::Queue,
        }
    }

    pub fn prev(&self) -> Self {
        match self {
            Screens::Queue => Screens::Albums,
            Screens::Albums => Screens::Artists,
            Screens::Artists => Screens::Directories,
            Screens::Directories => Screens::Logs,
            Screens::Logs => Screens::Queue,
        }
    }
}

#[derive(Debug, Default)]
struct DirStack<T: std::fmt::Debug> {
    current: (Vec<T>, MyState<ListState>),
    others: Vec<(Vec<T>, MyState<ListState>)>,
}

impl<T: std::fmt::Debug> DirStack<T> {
    fn new(root: Vec<T>) -> Self {
        let mut val = Self {
            others: Vec::new(),
            current: (Vec::new(), MyState::default()),
        };
        let mut root_state = MyState::default();

        val.push(Vec::new());

        if !root.is_empty() {
            root_state.select(Some(0));
            // root.sort();
        };

        val.current = (root, root_state);
        val
    }

    fn push(&mut self, head: Vec<T>) {
        let mut new_state = MyState::default();
        if !head.is_empty() {
            new_state.select(Some(0));
        };
        let current_head = std::mem::replace(&mut self.current, (head, new_state));
        self.others.push(current_head);
    }

    fn pop(&mut self) -> Option<(Vec<T>, MyState<ListState>)> {
        if self.others.len() > 1 {
            let top = self.others.pop().expect("There should always be at least two elements");
            Some(std::mem::replace(&mut self.current, top))
        } else {
            None
        }
    }

    fn get_selected(&self) -> Option<&T> {
        if let Some(sel) = self.current.1.get_selected() {
            self.current.0.get(sel)
        } else {
            None
        }
    }

    fn next(&mut self) {
        self.current.1.next()
    }

    fn prev(&mut self) {
        self.current.1.prev()
    }
}
