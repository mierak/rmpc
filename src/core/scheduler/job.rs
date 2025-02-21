use std::time::{Duration, Instant};

use anyhow::Result;

use crate::shared::{id::Id, macros::try_skip};

#[derive(derive_more::Debug)]
pub(super) struct Job<T> {
    pub(super) id: Id,
    #[debug(skip)]
    pub(super) callback: Box<dyn FnOnce(&T) -> Result<()> + Send + 'static>,
    pub(super) run_at: Instant,
}

impl<T> Job<T> {
    pub(super) fn new(
        id: Id,
        timeout: Duration,
        now: Instant,
        callback: impl FnOnce(&T) -> Result<()> + Send + 'static,
    ) -> Self {
        Self { id, run_at: now + timeout, callback: Box::new(callback) }
    }

    pub(super) fn run(self, args: &T) {
        try_skip!((self.callback)(args), "Scheduled job failed");
    }
}

impl<T> PartialEq for Job<T> {
    fn eq(&self, other: &Self) -> bool {
        self.run_at == other.run_at
    }
}
impl<T> Eq for Job<T> {}

impl<T> PartialOrd for Job<T> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl<T> Ord for Job<T> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.run_at.cmp(&other.run_at)
    }
}
