use crate::shared::id::{self, Id};

#[derive(Debug, derive_more::Deref, Clone, Copy, Eq, PartialEq, Hash)]
pub struct BufferId(Id);

impl BufferId {
    pub fn new() -> Self {
        BufferId(id::new())
    }
}
