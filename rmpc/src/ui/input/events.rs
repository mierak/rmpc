use crossterm::event::{KeyCode, KeyModifiers};

use crate::config::keys::Key;

#[derive(Debug)]
pub enum InputResultEvent {
    Push,
    Pop,
    Confirm,
    NoChange,
    Cancel,
}

#[derive(Debug, Clone, Copy)]
pub enum InputEvent {
    Push(char),

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
    pub fn from_key_event(ev: Key) -> Option<Self> {
        match ev.key {
            // Movement
            KeyCode::Left if ev.modifiers.contains(KeyModifiers::CONTROL) => {
                Some(InputEvent::BackWord)
            }
            KeyCode::Right if ev.modifiers.contains(KeyModifiers::CONTROL) => {
                Some(InputEvent::ForwardWord)
            }
            KeyCode::Left => Some(InputEvent::Back),
            KeyCode::Right => Some(InputEvent::Forward),
            KeyCode::Char('b') if ev.modifiers.contains(KeyModifiers::CONTROL) => {
                Some(InputEvent::Back)
            }
            KeyCode::Char('f') if ev.modifiers.contains(KeyModifiers::CONTROL) => {
                Some(InputEvent::Forward)
            }
            KeyCode::Char('b') if ev.modifiers.contains(KeyModifiers::ALT) => {
                Some(InputEvent::BackWord)
            }
            KeyCode::Char('f') if ev.modifiers.contains(KeyModifiers::ALT) => {
                Some(InputEvent::ForwardWord)
            }
            KeyCode::Char('a') if ev.modifiers.contains(KeyModifiers::CONTROL) => {
                Some(InputEvent::Start)
            }
            KeyCode::Char('e') if ev.modifiers.contains(KeyModifiers::CONTROL) => {
                Some(InputEvent::End)
            }

            // Delete
            KeyCode::Char('h') if ev.modifiers.contains(KeyModifiers::CONTROL) => {
                Some(InputEvent::PopLeft)
            }
            KeyCode::Char('d') if ev.modifiers.contains(KeyModifiers::CONTROL) => {
                Some(InputEvent::PopRight)
            }
            KeyCode::Char('u') if ev.modifiers.contains(KeyModifiers::CONTROL) => {
                Some(InputEvent::DeleteToStart)
            }
            KeyCode::Char('k') if ev.modifiers.contains(KeyModifiers::CONTROL) => {
                Some(InputEvent::DeleteToEnd)
            }
            KeyCode::Char('w') if ev.modifiers.contains(KeyModifiers::CONTROL) => {
                Some(InputEvent::PopWordLeft)
            }
            KeyCode::Backspace if ev.modifiers.contains(KeyModifiers::ALT) => {
                Some(InputEvent::PopWordLeft)
            }
            KeyCode::Char('d') if ev.modifiers.contains(KeyModifiers::ALT) => {
                Some(InputEvent::PopWordRight)
            }
            KeyCode::Backspace => Some(InputEvent::PopLeft),

            // Other
            KeyCode::Char(c) => Some(InputEvent::Push(c)),
            _ => None,
        }
    }
}
