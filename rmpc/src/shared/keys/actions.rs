#[cfg(debug_assertions)]
use crate::config::keys::LogsActions;
use crate::config::keys::{CommonAction, GlobalAction, QueueActions};

#[derive(Debug, Clone)]
pub enum Actions {
    Global(GlobalAction),
    Common(CommonAction),
    Queue(QueueActions),
    #[cfg(debug_assertions)]
    Logs(LogsActions),
}

impl From<GlobalAction> for Actions {
    fn from(value: GlobalAction) -> Self {
        Actions::Global(value)
    }
}

impl From<CommonAction> for Actions {
    fn from(value: CommonAction) -> Self {
        Actions::Common(value)
    }
}

impl From<QueueActions> for Actions {
    fn from(value: QueueActions) -> Self {
        Actions::Queue(value)
    }
}

#[cfg(debug_assertions)]
impl From<LogsActions> for Actions {
    fn from(value: LogsActions) -> Self {
        Actions::Logs(value)
    }
}

impl Actions {
    pub fn as_global(&self) -> Option<&GlobalAction> {
        if let Actions::Global(action) = self { Some(action) } else { None }
    }

    pub fn as_common(&self) -> Option<&CommonAction> {
        if let Actions::Common(action) = self { Some(action) } else { None }
    }

    pub fn as_queue(&self) -> Option<&QueueActions> {
        if let Actions::Queue(action) = self { Some(action) } else { None }
    }

    #[cfg(debug_assertions)]
    pub fn as_logs(&self) -> Option<&LogsActions> {
        if let Actions::Logs(action) = self { Some(action) } else { None }
    }
}
