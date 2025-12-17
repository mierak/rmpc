use std::{collections::HashMap, sync::Arc};

use itertools::Itertools;

use crate::{
    config::keys::{CommonAction, Key, KeyConfig},
    shared::keys::actions::Actions,
    ui::input::InputModeDiscriminants,
};

#[derive(Debug, Default)]
pub(super) struct KeyTreeNode {
    children: HashMap<Key, KeyTreeNode>,
    actions: Arc<Vec<Actions>>,
}

fn is_insert_mode_action(action: &CommonAction) -> bool {
    matches!(action, CommonAction::Close | CommonAction::Confirm)
}

impl KeyTreeNode {
    pub fn build_trie(cfg: &KeyConfig, mode: InputModeDiscriminants) -> Self {
        let mut root = KeyTreeNode::default();

        if matches!(mode, InputModeDiscriminants::Normal) {
            for seq in &cfg.global {
                root.insert(&seq.0.0, Actions::Global(seq.1.clone()));
            }
        }

        for seq in cfg.navigation.iter().filter(|(_, act)| {
            matches!(mode, InputModeDiscriminants::Normal) || is_insert_mode_action(act)
        }) {
            root.insert(&seq.0.0, Actions::Common(seq.1.clone()));
        }

        if matches!(mode, InputModeDiscriminants::Normal) {
            for seq in &cfg.queue {
                root.insert(&seq.0.0, Actions::Queue(*seq.1));
            }
        }

        if matches!(mode, InputModeDiscriminants::Normal) {
            #[cfg(debug_assertions)]
            for seq in &cfg.logs {
                root.insert(&seq.0.0, Actions::Logs(*seq.1));
            }
        }

        root
    }

    pub fn get(&self, key: &Key) -> Option<&KeyTreeNode> {
        self.children.get(key)
    }

    pub fn is_empty(&self) -> bool {
        self.children.is_empty()
    }

    pub fn action(&self) -> &Arc<Vec<Actions>> {
        &self.actions
    }

    fn insert(&mut self, keys: &[Key], action: Actions) {
        let mut node = self;
        for key in keys {
            node = node.children.entry(*key).or_default();
        }

        if !node.actions.is_empty() {
            log::warn!(
                actions:? = node.actions,
                action:?,
                seq = keys.iter().map(|k| k.to_string()).join("").as_str();
                "Multiple existing actions for key sequence"
            );
        }

        // Insert action to the final node in sequence.
        if let Some(actions) = Arc::get_mut(&mut node.actions) {
            actions.push(action);
        } else {
            // This should never happen since the trie is built from scratch on
            // initialization.
            log::warn!(
                seq = keys.iter().map(|k| k.to_string()).join("").as_str();
                "Failed to insert action for key sequence",
            );
        }
    }
}
