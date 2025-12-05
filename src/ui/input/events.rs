use crossterm::event::{KeyCode, KeyModifiers};

use crate::{config::keys::CommonAction, ctx::Ctx, shared::key_event::KeyEvent};

#[derive(Debug)]
pub enum InputResultEvent {
    Push(String),
    Pop(String),
    Confirm(String),
    NoChange,
    Cancel,
}

#[derive(Debug, Clone, Copy)]
pub enum InputEvent {
    Push(char),
    Confirm,
    Cancel,

    // Delete
    PopLeft,
    PopRight,
    PopWordLeft,
    PopWordRight,
    DeleteToStart,
    DeleteToEnd,

    // Movement
    Forward,
    Back,
    Start,
    End,
    ForwardWord,
    BackWord,
}

impl InputEvent {
    pub fn from_key_event(ev: &mut KeyEvent, ctx: &Ctx) -> Option<Self> {
        match ev.as_common_action(ctx) {
            Some(CommonAction::Confirm) => {
                ev.abandon();
                Some(InputEvent::Confirm)
            }
            Some(CommonAction::Close) => {
                ev.abandon();
                Some(InputEvent::Cancel)
            }
            _ => {
                ev.abandon();
                match ev.code() {
                    // Movement
                    KeyCode::Left if ev.inner.modifiers.contains(KeyModifiers::CONTROL) => {
                        Some(InputEvent::BackWord)
                    }
                    KeyCode::Right if ev.inner.modifiers.contains(KeyModifiers::CONTROL) => {
                        Some(InputEvent::ForwardWord)
                    }
                    KeyCode::Left => Some(InputEvent::Back),
                    KeyCode::Right => Some(InputEvent::Forward),
                    KeyCode::Char('b') if ev.inner.modifiers.contains(KeyModifiers::CONTROL) => {
                        Some(InputEvent::Back)
                    }
                    KeyCode::Char('f') if ev.inner.modifiers.contains(KeyModifiers::CONTROL) => {
                        Some(InputEvent::Forward)
                    }
                    KeyCode::Char('b') if ev.inner.modifiers.contains(KeyModifiers::ALT) => {
                        Some(InputEvent::BackWord)
                    }
                    KeyCode::Char('f') if ev.inner.modifiers.contains(KeyModifiers::ALT) => {
                        Some(InputEvent::ForwardWord)
                    }
                    KeyCode::Char('a') if ev.inner.modifiers.contains(KeyModifiers::CONTROL) => {
                        Some(InputEvent::Start)
                    }
                    KeyCode::Char('e') if ev.inner.modifiers.contains(KeyModifiers::CONTROL) => {
                        Some(InputEvent::End)
                    }

                    // Delete
                    KeyCode::Char('h') if ev.inner.modifiers.contains(KeyModifiers::CONTROL) => {
                        Some(InputEvent::PopLeft)
                    }
                    KeyCode::Char('d') if ev.inner.modifiers.contains(KeyModifiers::CONTROL) => {
                        Some(InputEvent::PopRight)
                    }
                    KeyCode::Char('u') if ev.inner.modifiers.contains(KeyModifiers::CONTROL) => {
                        Some(InputEvent::DeleteToStart)
                    }
                    KeyCode::Char('k') if ev.inner.modifiers.contains(KeyModifiers::CONTROL) => {
                        Some(InputEvent::DeleteToEnd)
                    }
                    KeyCode::Char('w') if ev.inner.modifiers.contains(KeyModifiers::CONTROL) => {
                        Some(InputEvent::PopWordLeft)
                    }
                    KeyCode::Backspace if ev.inner.modifiers.contains(KeyModifiers::ALT) => {
                        Some(InputEvent::PopWordLeft)
                    }
                    KeyCode::Char('d') if ev.inner.modifiers.contains(KeyModifiers::ALT) => {
                        Some(InputEvent::PopWordRight)
                    }
                    KeyCode::Backspace => Some(InputEvent::PopLeft),

                    // Other
                    KeyCode::Char(c) => Some(InputEvent::Push(c)),
                    _ => None,
                }
            }
        }
    }
}
