use std::sync::atomic::{AtomicUsize, Ordering};

static LAST_ID: AtomicUsize = AtomicUsize::new(1);

#[derive(Debug, derive_more::Deref, Clone, Copy, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct Id(usize);

pub fn new() -> Id {
    Id(LAST_ID.fetch_add(1, Ordering::Relaxed))
}
