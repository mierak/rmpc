use std::{ops::Not, sync::Arc};

#[cfg(debug_assertions)]
use crate::config::keys::LogsActions;
use crate::{
    config::keys::{CommonAction, GlobalAction, QueueActions},
    shared::keys::actions::Actions,
};

#[derive(Debug)]
pub struct ActionEvent {
    pub actions: Arc<Vec<Actions>>,
    already_handled: bool,
}

impl From<Arc<Vec<Actions>>> for ActionEvent {
    fn from(value: Arc<Vec<Actions>>) -> Self {
        Self { actions: value, already_handled: false }
    }
}

impl ActionEvent {
    pub fn abandon(&mut self) {
        self.already_handled = false;
    }

    pub fn claim_global(&mut self) -> Option<&GlobalAction> {
        let result = self
            .already_handled
            .not()
            .then(|| self.actions.iter().find_map(|act| act.as_global()))
            .flatten();
        if result.is_some() {
            self.already_handled = true;
        }
        result
    }

    pub fn claim_common(&mut self) -> Option<&CommonAction> {
        let result = self
            .already_handled
            .not()
            .then(|| self.actions.iter().find_map(|act| act.as_common()))
            .flatten();
        if result.is_some() {
            self.already_handled = true;
        }
        result
    }

    pub fn claim_queue(&mut self) -> Option<&QueueActions> {
        let result = self
            .already_handled
            .not()
            .then(|| self.actions.iter().find_map(|act| act.as_queue()))
            .flatten();
        if result.is_some() {
            self.already_handled = true;
        }
        result
    }

    #[cfg(debug_assertions)]
    pub fn claim_logs(&mut self) -> Option<&LogsActions> {
        let result = self
            .already_handled
            .not()
            .then(|| self.actions.iter().find_map(|act| act.as_logs()))
            .flatten();
        if result.is_some() {
            self.already_handled = true;
        }
        result
    }
}
