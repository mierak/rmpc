use crossterm::event::{KeyCode, KeyEvent as CKeyEvent, KeyModifiers};

#[cfg(debug_assertions)]
use crate::config::keys::LogsActions;
use crate::{
    config::keys::{CommonAction, GlobalAction, Key, QueueActions},
    ctx::Ctx,
};

#[derive(Debug, Clone)]
pub struct KeyEvent {
    pub(crate) inner: CKeyEvent,
    already_handled: bool,
}

impl From<CKeyEvent> for KeyEvent {
    fn from(value: CKeyEvent) -> Self {
        Self { inner: value, already_handled: false }
    }
}

impl From<CKeyEvent> for Key {
    fn from(value: CKeyEvent) -> Self {
        let should_insert_shift = matches!(value.code, KeyCode::Char(c) if c.is_uppercase());

        let mut modifiers = value.modifiers;
        if should_insert_shift {
            modifiers.insert(KeyModifiers::SHIFT);
        }

        let key = if modifiers.contains(KeyModifiers::SHIFT) {
            if let KeyCode::Char(c) = value.code {
                KeyCode::Char(c.to_ascii_uppercase())
            } else {
                value.code
            }
        } else {
            value.code
        };

        Self { key, modifiers }
    }
}

impl KeyEvent {
    pub fn code(&self) -> KeyCode {
        self.inner.code
    }

    pub fn stop_propagation(&mut self) {
        self.already_handled = true;
    }

    pub fn abandon(&mut self) {
        self.already_handled = false;
    }

    pub fn as_common_action<'ctx>(&mut self, ctx: &'ctx Ctx) -> Option<&'ctx CommonAction> {
        if self.already_handled {
            None
        } else if let Some(action) = ctx.config.keybinds.navigation.get(&self.inner.into()) {
            self.already_handled = true;
            Some(action)
        } else {
            None
        }
    }

    pub fn as_global_action<'ctx>(&mut self, ctx: &'ctx Ctx) -> Option<&'ctx GlobalAction> {
        if self.already_handled {
            None
        } else if let Some(action) = ctx.config.keybinds.global.get(&self.inner.into()) {
            self.already_handled = true;
            Some(action)
        } else {
            None
        }
    }

    #[cfg(debug_assertions)]
    pub fn as_logs_action(&mut self, ctx: &Ctx) -> Option<LogsActions> {
        if self.already_handled {
            None
        } else if let Some(action) = ctx.config.keybinds.logs.get(&self.inner.into()) {
            self.already_handled = true;
            Some(*action)
        } else {
            None
        }
    }

    pub fn as_queue_action(&mut self, ctx: &Ctx) -> Option<QueueActions> {
        if self.already_handled {
            None
        } else if let Some(action) = ctx.config.keybinds.queue.get(&self.inner.into()) {
            self.already_handled = true;
            Some(*action)
        } else {
            None
        }
    }
}
