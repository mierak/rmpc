use std::{cell::RefCell, sync::Arc, time::Duration};

use itertools::Itertools;

use crate::{
    config::{Config, keys::Key},
    ctx::Ctx,
    shared::{
        events::AppEvent,
        id::{self, Id},
        keys::{actions::Actions, trie::KeyTreeNode},
    },
    ui::input::{InputMode, InputModeDiscriminants},
};

#[derive(Debug)]
pub struct KeyResolver {
    normal_root: KeyTreeNode,
    insert_root: KeyTreeNode,
    timeout_id: Id,
    buffer: RefCell<Vec<Key>>,
    normal_timeout: Duration,
    insert_timeout: Duration,
}

#[derive(Debug)]
enum TraverseResult {
    Exact(Arc<Vec<Actions>>),
    Ambiguous(Arc<Vec<Actions>>),
    Prefix,
    Mismatch,
}

impl KeyResolver {
    pub fn new(cfg: &Config) -> Self {
        Self {
            normal_root: KeyTreeNode::build_trie(&cfg.keybinds, InputModeDiscriminants::Normal),
            insert_root: KeyTreeNode::build_trie(&cfg.keybinds, InputModeDiscriminants::Insert),
            timeout_id: id::new(),
            buffer: RefCell::new(Vec::new()),
            normal_timeout: Duration::from_millis(cfg.normal_timeout_ms),
            insert_timeout: Duration::from_millis(cfg.insert_timeout_ms),
        }
    }

    pub fn buffer_to_string(&self) -> String {
        self.buffer.borrow().iter().map(|k| k.to_string()).join("")
    }

    pub fn handle_timeout(&self, ctx: &Ctx) {
        log::trace!(q:? = self.buffer; "Key timeout occurred");
        let mut buf = self.buffer.borrow_mut();
        if buf.is_empty() {
            return;
        }

        let root = match ctx.input.mode() {
            InputMode::Normal => &self.normal_root,
            InputMode::Insert(_) => &self.insert_root,
        };
        match ctx.input.mode() {
            InputMode::Normal => match self.traverse(&buf, root) {
                TraverseResult::Exact(action) => {
                    self.execute_action(action, ctx);
                }
                TraverseResult::Ambiguous(action) => {
                    self.execute_action(action, ctx);
                }
                // Nothing to do here, just clear the buffer
                TraverseResult::Mismatch => {}
                TraverseResult::Prefix => {}
            },
            InputMode::Insert(_) => match self.traverse(&buf, root) {
                TraverseResult::Exact(action) => {
                    self.flush_insert_buffer(Some(action), std::mem::take(&mut buf), ctx);
                }
                TraverseResult::Ambiguous(action) => {
                    self.flush_insert_buffer(Some(action), std::mem::take(&mut buf), ctx);
                }
                TraverseResult::Mismatch => {
                    self.flush_insert_buffer(None, std::mem::take(&mut buf), ctx);
                }
                TraverseResult::Prefix => {
                    self.flush_insert_buffer(None, std::mem::take(&mut buf), ctx);
                }
            },
        }

        buf.clear();
    }

    pub fn handle_key_event(&self, key: Key, ctx: &Ctx) {
        self.cancel_timeout(ctx);

        let mut buf = self.buffer.borrow_mut();
        buf.push(key);

        match ctx.input.mode() {
            InputMode::Normal => match self.traverse(&buf, &self.normal_root) {
                TraverseResult::Exact(action) => {
                    self.execute_action(action, ctx);
                    buf.clear();
                }
                TraverseResult::Ambiguous(_action) => {
                    self.schedule_timeout(ctx);
                }
                TraverseResult::Mismatch => {
                    buf.clear();
                }
                TraverseResult::Prefix => {
                    self.schedule_timeout(ctx);
                }
            },
            InputMode::Insert(_) => match self.traverse(&buf, &self.insert_root) {
                TraverseResult::Exact(action) => {
                    self.flush_insert_buffer(Some(action), std::mem::take(&mut buf), ctx);
                }
                TraverseResult::Ambiguous(_action) => {
                    self.schedule_timeout(ctx);
                }
                TraverseResult::Mismatch => {
                    self.flush_insert_buffer(None, std::mem::take(&mut buf), ctx);
                }
                TraverseResult::Prefix => {
                    self.schedule_timeout(ctx);
                }
            },
        }
    }

    fn schedule_timeout(&self, ctx: &Ctx) {
        let timeout = match ctx.input.mode() {
            InputMode::Normal => self.normal_timeout,
            InputMode::Insert(_) => self.insert_timeout,
        };
        ctx.scheduler.schedule_replace(self.timeout_id, timeout, |(tx, _)| {
            log::trace!("Key sequence timeout reached, sending timeout event");
            Ok(tx.send(AppEvent::KeyTimeout)?)
        });
    }

    fn cancel_timeout(&self, ctx: &Ctx) {
        ctx.scheduler.cancel(self.timeout_id);
    }

    fn execute_action(&self, action: Arc<Vec<Actions>>, ctx: &Ctx) {
        if let Err(err) = ctx.app_event_sender.send(AppEvent::ActionResolved(action.into())) {
            log::error!(err:?; "Failed to send ActionResolved event");
        }
    }

    fn flush_insert_buffer(&self, action: Option<Arc<Vec<Actions>>>, buf: Vec<Key>, ctx: &Ctx) {
        if let Err(err) =
            ctx.app_event_sender.send(AppEvent::InsertModeFlush((action.map(|a| a.into()), buf)))
        {
            log::error!(err:?; "Failed to send InsertModeFlush event");
        }
    }

    fn traverse(&self, keys: &[Key], root: &KeyTreeNode) -> TraverseResult {
        let mut curr = root;
        for key in keys {
            match curr.get(key) {
                Some(next) => curr = next,
                None => return TraverseResult::Mismatch,
            }
        }

        match (curr.action().is_empty(), curr.is_empty()) {
            (false, true) => TraverseResult::Exact(Arc::clone(curr.action())),
            (false, false) => TraverseResult::Ambiguous(Arc::clone(curr.action())),
            (true, false) => TraverseResult::Prefix,
            (true, true) => {
                log::warn!(keys:?; "Key sequence leads to a node with no action and no children");
                TraverseResult::Mismatch
            }
        }
    }
}

#[cfg(test)]
mod test {
    use std::collections::HashMap;

    use crossbeam::channel::{Receiver, RecvTimeoutError, Sender};
    use crossterm::event::{KeyCode, KeyModifiers};
    use rstest::rstest;

    use super::*;
    use crate::{
        config::keys::{CommonAction, KeyConfig},
        shared::{
            events::{ClientRequest, WorkRequest},
            keys::ActionEvent,
        },
        tests::fixtures::{app_event_channel, client_request_channel, ctx, work_request_channel},
        ui::input::BufferId,
    };

    fn k(ch: char) -> Key {
        Key { key: KeyCode::Char(ch), modifiers: KeyModifiers::NONE }
    }

    fn resolver() -> KeyResolver {
        let mut cfg = KeyConfig {
            global: HashMap::new(),
            navigation: HashMap::new(),
            albums: HashMap::new(),
            artists: HashMap::new(),
            directories: HashMap::new(),
            search: HashMap::new(),
            #[cfg(debug_assertions)]
            logs: HashMap::new(),
            queue: HashMap::new(),
        };

        cfg.navigation.insert(vec![k('g')].into(), CommonAction::Down);
        cfg.navigation.insert(vec![k('g'), k('d')].into(), CommonAction::DownHalf);
        cfg.navigation.insert(vec![k('x')].into(), CommonAction::Up);
        cfg.navigation.insert(vec![k('g'), k('a'), k('a')].into(), CommonAction::Right);

        let mut insert_cfg = KeyConfig {
            global: HashMap::new(),
            navigation: HashMap::new(),
            albums: HashMap::new(),
            artists: HashMap::new(),
            directories: HashMap::new(),
            search: HashMap::new(),
            #[cfg(debug_assertions)]
            logs: HashMap::new(),
            queue: HashMap::new(),
        };

        insert_cfg.navigation.insert(vec![k('g')].into(), CommonAction::Close);
        insert_cfg.navigation.insert(vec![k('g'), k('d')].into(), CommonAction::Close);
        insert_cfg.navigation.insert(vec![k('x')].into(), CommonAction::Close);
        insert_cfg.navigation.insert(vec![k('g'), k('a'), k('a')].into(), CommonAction::Close);

        KeyResolver {
            normal_root: KeyTreeNode::build_trie(&cfg, InputModeDiscriminants::Normal),
            insert_root: KeyTreeNode::build_trie(&insert_cfg, InputModeDiscriminants::Insert),
            timeout_id: id::new(),
            buffer: RefCell::new(Vec::new()),
            normal_timeout: Duration::from_millis(1000),
            insert_timeout: Duration::from_millis(1000),
        }
    }

    #[test]
    fn traverse_exact_on_leaf() {
        let resolver = resolver();
        let res = resolver.traverse(&[k('x')], &resolver.normal_root);
        match res {
            TraverseResult::Exact(other) => match other.as_slice() {
                [Actions::Common(CommonAction::Up)] => {}
                _ => panic!("expected Exact(Up), got {other:?}"),
            },
            other => panic!("expected Exact(Up), got {other:?}"),
        }
    }

    #[test]
    fn traverse_ambiguous_on_action_with_children() {
        let resolver = resolver();
        let res = resolver.traverse(&[k('g')], &resolver.normal_root);
        match res {
            TraverseResult::Ambiguous(other) => match other.as_slice() {
                [Actions::Common(CommonAction::Down)] => {}
                _ => panic!("expected Ambiguous(Down), got {other:?}"),
            },
            other => panic!("expected Ambiguous(Down), got {other:?}"),
        }
    }

    #[test]
    fn traverse_exact_on_longer_sequence_leaf() {
        let resolver = resolver();
        let res = resolver.traverse(&[k('g'), k('d')], &resolver.normal_root);
        match res {
            TraverseResult::Exact(other) => match other.as_slice() {
                [Actions::Common(CommonAction::DownHalf)] => {}
                _ => panic!("expected Exact(DownHalf), got {other:?}"),
            },
            other => panic!("expected Exact(DownHalf), got {other:?}"),
        }
    }

    #[test]
    fn traverse_prefix_when_valid_path_without_action() {
        let resolver = resolver();
        let res = resolver.traverse(&[k('g'), k('a')], &resolver.normal_root);
        match res {
            TraverseResult::Prefix => {}
            other => panic!("expected Prefix, got {other:?}"),
        }
    }

    #[test]
    fn traverse_mismatch_on_invalid_key() {
        let resolver = resolver();
        let res = resolver.traverse(&[k('z')], &resolver.normal_root);
        match res {
            TraverseResult::Mismatch => {}
            other => panic!("expected Mismatch, got {other:?}"),
        }
    }

    #[test]
    fn traverse_mismatch_on_invalid_key_with_longer_sequence() {
        let resolver = resolver();
        let res = resolver.traverse(&[k('g'), k('z')], &resolver.normal_root);
        match res {
            TraverseResult::Mismatch => {}
            other => panic!("expected Mismatch, got {other:?}"),
        }
    }

    #[rstest]
    fn handle_key_event_exact_clears_and_executes(
        app_event_channel: (Sender<AppEvent>, Receiver<AppEvent>),
        work_request_channel: (Sender<WorkRequest>, Receiver<WorkRequest>),
        client_request_channel: (Sender<ClientRequest>, Receiver<ClientRequest>),
    ) {
        let ctx = ctx(app_event_channel.clone(), work_request_channel, client_request_channel);
        let resolver = resolver();

        resolver.handle_key_event(k('x'), &ctx);

        assert!(resolver.buffer.borrow().is_empty(), "buffer should be cleared after exact action");

        match app_event_channel.1.recv_timeout(Duration::from_millis(100)) {
            Ok(AppEvent::ActionResolved(ActionEvent { actions, .. })) => match actions.as_slice() {
                [Actions::Common(CommonAction::Up)] => {}
                _ => panic!("expected ActionResolved(Up), got {actions:?}"),
            },
            other => panic!("expected ActionResolved(Up), got {other:?}"),
        }
    }

    #[rstest]
    fn handle_key_event_ambiguous_schedules_timeout_and_keeps_buffer(
        app_event_channel: (Sender<AppEvent>, Receiver<AppEvent>),
        work_request_channel: (Sender<WorkRequest>, Receiver<WorkRequest>),
        client_request_channel: (Sender<ClientRequest>, Receiver<ClientRequest>),
    ) {
        let ctx = ctx(app_event_channel.clone(), work_request_channel, client_request_channel);
        let resolver = resolver();

        resolver.handle_key_event(k('g'), &ctx);
        assert_eq!(resolver.buffer_to_string(), "g");

        resolver.handle_timeout(&ctx);

        assert!(resolver.buffer.borrow().is_empty(), "buffer should be cleared after timeout");
        match app_event_channel.1.recv_timeout(Duration::from_millis(100)) {
            Ok(AppEvent::ActionResolved(ActionEvent { actions, .. })) => match actions.as_slice() {
                [Actions::Common(CommonAction::Down)] => {}
                _ => panic!("expected ActionResolved(Down), got {actions:?}"),
            },
            other => panic!("expected ActionResolved(Down), got {other:?}"),
        }
    }

    #[rstest]
    fn handle_key_event_prefix_schedules_timeout_and_keeps_buffer(
        app_event_channel: (Sender<AppEvent>, Receiver<AppEvent>),
        work_request_channel: (Sender<WorkRequest>, Receiver<WorkRequest>),
        client_request_channel: (Sender<ClientRequest>, Receiver<ClientRequest>),
    ) {
        let ctx = ctx(app_event_channel.clone(), work_request_channel, client_request_channel);
        let resolver = resolver();

        resolver.handle_key_event(k('g'), &ctx);
        resolver.handle_key_event(k('a'), &ctx);

        assert_eq!(resolver.buffer_to_string(), "ga");

        resolver.handle_timeout(&ctx);

        assert!(
            resolver.buffer.borrow().is_empty(),
            "buffer should be cleared after prefix timeout"
        );
        match app_event_channel.1.recv_timeout(Duration::from_millis(300)) {
            Err(RecvTimeoutError::Timeout) => {}
            other => panic!("expected Timeout, got {other:?}"),
        }
    }

    #[rstest]
    fn handle_key_event_mismatch_clears_buffer(ctx: Ctx) {
        let resolver = resolver();

        resolver.handle_key_event(k('g'), &ctx);
        resolver.handle_key_event(
            Key { key: KeyCode::Char('x'), modifiers: KeyModifiers::SHIFT },
            &ctx,
        );

        assert!(resolver.buffer.borrow().is_empty(), "buffer should be cleared on mismatch");
    }

    #[rstest]
    fn handle_key_event_longer_sequence_executes_leaf_immediately(
        app_event_channel: (Sender<AppEvent>, Receiver<AppEvent>),
        work_request_channel: (Sender<WorkRequest>, Receiver<WorkRequest>),
        client_request_channel: (Sender<ClientRequest>, Receiver<ClientRequest>),
    ) {
        let ctx = ctx(app_event_channel.clone(), work_request_channel, client_request_channel);
        let resolver = resolver();

        resolver.handle_key_event(k('g'), &ctx);
        resolver.handle_key_event(k('d'), &ctx);

        assert!(resolver.buffer.borrow().is_empty(), "buffer should be cleared after exact leaf");
        match app_event_channel.1.recv_timeout(Duration::from_millis(100)) {
            Ok(AppEvent::ActionResolved(ActionEvent { actions, .. })) => match actions.as_slice() {
                [Actions::Common(CommonAction::DownHalf)] => {}
                _ => panic!("expected ActionResolved(DownHalf), got {actions:?}"),
            },
            other => panic!("expected ActionResolved(DownHalf), got {other:?}"),
        }
    }

    #[rstest]
    fn timeout_on_ambiguous_executes_shorter_action(
        app_event_channel: (Sender<AppEvent>, Receiver<AppEvent>),
        work_request_channel: (Sender<WorkRequest>, Receiver<WorkRequest>),
        client_request_channel: (Sender<ClientRequest>, Receiver<ClientRequest>),
    ) {
        let ctx = ctx(app_event_channel.clone(), work_request_channel, client_request_channel);
        let resolver = resolver();

        resolver.handle_key_event(k('g'), &ctx);

        resolver.handle_timeout(&ctx);

        assert!(resolver.buffer.borrow().is_empty(), "buffer should be cleared after timeout");
        match app_event_channel.1.recv_timeout(Duration::from_millis(100)) {
            Ok(AppEvent::ActionResolved(ActionEvent { actions, .. })) => match actions.as_slice() {
                [Actions::Common(CommonAction::Down)] => {}
                _ => panic!("expected ActionResolved(Down), got {actions:?}"),
            },
            other => panic!("expected ActionResolved(Down), got {other:?}"),
        }
    }

    #[rstest]
    fn timeout_on_prefix_is_noop(
        app_event_channel: (Sender<AppEvent>, Receiver<AppEvent>),
        work_request_channel: (Sender<WorkRequest>, Receiver<WorkRequest>),
        client_request_channel: (Sender<ClientRequest>, Receiver<ClientRequest>),
    ) {
        let ctx = ctx(app_event_channel.clone(), work_request_channel, client_request_channel);
        let resolver = resolver();

        resolver.handle_key_event(k('g'), &ctx);
        resolver.handle_key_event(k('a'), &ctx);

        resolver.handle_timeout(&ctx);

        assert!(resolver.buffer.borrow().is_empty(), "buffer should be cleared after timeout");
        match app_event_channel.1.recv_timeout(Duration::from_millis(300)) {
            Err(RecvTimeoutError::Timeout) => {}
            other => panic!("expected Timeout, got {other:?}"),
        }
    }

    #[rstest]
    fn ambiguous_then_mismatch_drops_action_and_clears_buffer(
        app_event_channel: (Sender<AppEvent>, Receiver<AppEvent>),
        work_request_channel: (Sender<WorkRequest>, Receiver<WorkRequest>),
        client_request_channel: (Sender<ClientRequest>, Receiver<ClientRequest>),
    ) {
        let ctx = ctx(app_event_channel.clone(), work_request_channel, client_request_channel);
        let resolver = resolver();

        resolver.handle_key_event(k('g'), &ctx);
        assert_eq!(resolver.buffer_to_string(), "g");

        resolver.handle_key_event(k('z'), &ctx);

        assert!(
            resolver.buffer.borrow().is_empty(),
            "buffer should be cleared on mismatch after ambiguous"
        );

        // No action should have been sent.
        match app_event_channel.1.recv_timeout(Duration::from_millis(200)) {
            Err(RecvTimeoutError::Timeout) => {}
            other => panic!("expected no action after mismatch, got {other:?}"),
        }
    }

    #[rstest]
    fn stray_timeout_with_empty_buffer_is_noop(
        app_event_channel: (Sender<AppEvent>, Receiver<AppEvent>),
        work_request_channel: (Sender<WorkRequest>, Receiver<WorkRequest>),
        client_request_channel: (Sender<ClientRequest>, Receiver<ClientRequest>),
    ) {
        let ctx = ctx(app_event_channel.clone(), work_request_channel, client_request_channel);
        let resolver = resolver();

        assert!(resolver.buffer.borrow().is_empty(), "precondition: buffer should be empty");

        resolver.handle_timeout(&ctx);

        match app_event_channel.1.recv_timeout(Duration::from_millis(200)) {
            Err(RecvTimeoutError::Timeout) => {}
            other => panic!("expected no action on stray timeout, got {other:?}"),
        }
    }

    #[rstest]
    fn insert_mode_non_candidate_key_flushes_without_action(
        app_event_channel: (Sender<AppEvent>, Receiver<AppEvent>),
        work_request_channel: (Sender<WorkRequest>, Receiver<WorkRequest>),
        client_request_channel: (Sender<ClientRequest>, Receiver<ClientRequest>),
    ) {
        let ctx = ctx(app_event_channel.clone(), work_request_channel, client_request_channel);
        ctx.input.insert_mode(BufferId::new());
        let resolver = resolver();

        resolver.handle_key_event(k('z'), &ctx);

        match app_event_channel.1.recv_timeout(Duration::from_millis(150)) {
            Ok(AppEvent::InsertModeFlush((None, buf))) => {
                assert_eq!(buf.len(), 1);
                assert_eq!(buf[0].to_string(), "z");
            }
            other => panic!("expected InsertModeFlush(None, ['z']), got {other:?}"),
        }

        assert!(resolver.buffer.borrow().is_empty(), "buffer should be empty after flush");
    }

    #[rstest]
    fn insert_mode_exact_sequence_flushes_with_action(
        app_event_channel: (Sender<AppEvent>, Receiver<AppEvent>),
        work_request_channel: (Sender<WorkRequest>, Receiver<WorkRequest>),
        client_request_channel: (Sender<ClientRequest>, Receiver<ClientRequest>),
    ) {
        let ctx = ctx(app_event_channel.clone(), work_request_channel, client_request_channel);
        ctx.input.insert_mode(BufferId::new());
        let resolver = resolver();

        resolver.handle_key_event(k('x'), &ctx);

        match app_event_channel.1.recv_timeout(Duration::from_millis(150)) {
            Ok(AppEvent::InsertModeFlush((Some(ActionEvent { actions, .. }), buf))) => {
                match actions.as_slice() {
                    [Actions::Common(CommonAction::Close)] => {
                        assert_eq!(buf.len(), 1);
                        assert_eq!(buf[0].to_string(), "x");
                    }
                    _ => panic!("expected InsertModeFlush(Some(Common(Close)), got {actions:?}"),
                }
            }

            other => panic!("expected InsertModeFlush(Some(Common(Close)), ['x']), got {other:?}"),
        }

        assert!(resolver.buffer.borrow().is_empty(), "buffer should be empty after flush");
    }

    #[rstest]
    fn insert_mode_ambiguous_then_timeout_flushes_with_action(
        app_event_channel: (Sender<AppEvent>, Receiver<AppEvent>),
        work_request_channel: (Sender<WorkRequest>, Receiver<WorkRequest>),
        client_request_channel: (Sender<ClientRequest>, Receiver<ClientRequest>),
    ) {
        let ctx = ctx(app_event_channel.clone(), work_request_channel, client_request_channel);
        ctx.input.insert_mode(BufferId::new());
        let resolver = resolver();

        resolver.handle_key_event(k('g'), &ctx);
        resolver.handle_timeout(&ctx);

        match app_event_channel.1.recv_timeout(Duration::from_millis(150)) {
            Ok(AppEvent::InsertModeFlush((Some(ActionEvent { actions, .. }), buf))) => {
                match actions.as_slice() {
                    [Actions::Common(CommonAction::Close)] => {
                        assert_eq!(buf.len(), 1);
                        assert_eq!(buf[0].to_string(), "g");
                    }
                    _ => panic!("expected InsertModeFlush(Some(Common(Close)), got {actions:?}"),
                }
            }
            other => panic!("expected InsertModeFlush(Some(Common(Close)), ['g']), got {other:?}"),
        }

        assert!(resolver.buffer.borrow().is_empty(), "buffer should be empty after timeout flush");
    }

    #[rstest]
    fn insert_mode_prefix_then_timeout_flushes_without_action(
        app_event_channel: (Sender<AppEvent>, Receiver<AppEvent>),
        work_request_channel: (Sender<WorkRequest>, Receiver<WorkRequest>),
        client_request_channel: (Sender<ClientRequest>, Receiver<ClientRequest>),
    ) {
        let ctx = ctx(app_event_channel.clone(), work_request_channel, client_request_channel);
        ctx.input.insert_mode(BufferId::new());
        let resolver = resolver();

        resolver.handle_key_event(k('g'), &ctx);
        resolver.handle_key_event(k('a'), &ctx);

        resolver.handle_timeout(&ctx);

        match app_event_channel.1.recv_timeout(Duration::from_millis(200)) {
            Ok(AppEvent::InsertModeFlush((None, buf))) => {
                assert_eq!(buf.iter().map(|k| k.to_string()).join(""), "ga");
            }
            other => panic!("expected InsertModeFlush(None, ['g','a']), got {other:?}"),
        }

        assert!(resolver.buffer.borrow().is_empty(), "buffer should be empty after timeout flush");
    }

    #[rstest]
    fn insert_mode_ambiguous_then_mismatch_flushes_without_action(
        app_event_channel: (Sender<AppEvent>, Receiver<AppEvent>),
        work_request_channel: (Sender<WorkRequest>, Receiver<WorkRequest>),
        client_request_channel: (Sender<ClientRequest>, Receiver<ClientRequest>),
    ) {
        let ctx = ctx(app_event_channel.clone(), work_request_channel, client_request_channel);
        ctx.input.insert_mode(BufferId::new());
        let resolver = resolver();

        resolver.handle_key_event(k('g'), &ctx);
        resolver.handle_key_event(k('z'), &ctx);

        match app_event_channel.1.recv_timeout(Duration::from_millis(200)) {
            Ok(AppEvent::InsertModeFlush((None, buf))) => {
                assert_eq!(buf.iter().map(|k| k.to_string()).join(""), "gz");
            }
            other => panic!("expected InsertModeFlush(None, ['g','z']), got {other:?}"),
        }

        assert!(resolver.buffer.borrow().is_empty(), "buffer should be empty after mismatch flush");
    }

    #[rstest]
    fn insert_mode_stray_timeout_with_empty_buffer_is_noop(
        app_event_channel: (Sender<AppEvent>, Receiver<AppEvent>),
        work_request_channel: (Sender<WorkRequest>, Receiver<WorkRequest>),
        client_request_channel: (Sender<ClientRequest>, Receiver<ClientRequest>),
    ) {
        let ctx = ctx(app_event_channel.clone(), work_request_channel, client_request_channel);
        ctx.input.insert_mode(BufferId::new());
        let resolver = resolver();

        assert!(resolver.buffer.borrow().is_empty(), "precondition: buffer should be empty");

        resolver.handle_timeout(&ctx);

        match app_event_channel.1.recv_timeout(Duration::from_millis(200)) {
            Err(RecvTimeoutError::Timeout) => {}
            other => panic!("expected no event on stray timeout, got {other:?}"),
        }
    }
}
