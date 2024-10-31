use crossterm::event::{KeyCode, KeyEvent as CKeyEvent};

use crate::{
    config::keys::{CommonAction, GlobalAction, QueueActions},
    context::AppContext,
};

#[cfg(debug_assertions)]
use crate::config::keys::LogsActions;

#[derive(Debug, Clone)]
pub struct KeyEvent {
    inner: CKeyEvent,
    already_handled: bool,
}

impl From<CKeyEvent> for KeyEvent {
    fn from(value: CKeyEvent) -> Self {
        Self {
            inner: value,
            already_handled: false,
        }
    }
}

impl KeyEvent {
    pub fn code(&self) -> KeyCode {
        self.inner.code
    }

    pub fn stop_propagation(&mut self) {
        self.already_handled = true;
    }

    pub fn as_common_action(&self, context: &AppContext) -> Option<CommonAction> {
        if self.already_handled {
            None
        } else {
            context.config.keybinds.navigation.get(&self.inner.into()).copied()
        }
    }

    pub fn as_global_action(&self, context: &AppContext) -> Option<GlobalAction> {
        if self.already_handled {
            None
        } else {
            context.config.keybinds.global.get(&self.inner.into()).copied()
        }
    }

    #[cfg(debug_assertions)]
    pub fn as_logs_action(&self, context: &AppContext) -> Option<LogsActions> {
        if self.already_handled {
            None
        } else {
            context.config.keybinds.logs.get(&self.inner.into()).copied()
        }
    }

    pub fn as_queue_action(&self, context: &AppContext) -> Option<QueueActions> {
        if self.already_handled {
            None
        } else {
            context.config.keybinds.queue.get(&self.inner.into()).copied()
        }
    }
}
