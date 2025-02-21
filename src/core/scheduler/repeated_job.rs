use std::time::{Duration, Instant};

use anyhow::Result;

use crate::shared::{id::Id, macros::try_skip};

#[derive(derive_more::Debug)]
pub(super) struct RepeatedJob<T> {
    pub(super) id: Id,
    #[debug(skip)]
    pub(super) callback: Box<dyn FnMut(&T) -> Result<()> + Send + 'static>,
    pub(super) interval: Duration,
    pub(super) run_at: Instant,
}

impl<T> RepeatedJob<T> {
    pub(super) fn new(
        id: Id,
        interval: Duration,
        now: Instant,
        callback: impl FnMut(&T) -> Result<()> + Send + 'static,
    ) -> Self {
        Self { id, interval, callback: Box::new(callback), run_at: now + interval }
    }

    pub(super) fn run(&mut self, args: &T, now: Instant) {
        try_skip!((self.callback)(args), "Repeated job failed");
        self.run_at = now + self.interval;
    }
}

impl<T> PartialEq for RepeatedJob<T> {
    fn eq(&self, other: &Self) -> bool {
        self.run_at == other.run_at
    }
}
impl<T> Eq for RepeatedJob<T> {}

impl<T> PartialOrd for RepeatedJob<T> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl<T> Ord for RepeatedJob<T> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.run_at.cmp(&other.run_at)
    }
}
