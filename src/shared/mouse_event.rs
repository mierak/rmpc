use std::time::{Duration, Instant};

use crossterm::event::{MouseButton, MouseEvent as CTMouseEvent, MouseEventKind as CTMouseEventKind};
use ratatui::layout::Position;

// maybe make the timout configurable?
const DOUBLE_CLICK_TIMEOUT: Duration = Duration::from_millis(500);

#[derive(Debug, Default, Clone, Copy)]
pub struct MouseEventTracker {
    last_left_click: Option<TimedMouseEvent>,
}

impl MouseEventTracker {
    pub fn track_and_get(&mut self, event: CTMouseEvent) -> Option<MouseEvent> {
        self.crossterm_ev_to_mouse_event(event).inspect(|ev| match ev.kind {
            MouseEventKind::LeftClick => {
                self.last_left_click = (*ev).into();
            }
            MouseEventKind::DoubleClick => {
                self.last_left_click = None;
            }
            _ => {}
        })
    }

    pub fn crossterm_ev_to_mouse_event(&self, value: CTMouseEvent) -> Option<MouseEvent> {
        let x = value.column;
        let y = value.row;

        match value.kind {
            CTMouseEventKind::Down(MouseButton::Left) => {
                if self.last_left_click.is_some_and(|c| c.is_doubled(x, y)) {
                    Some(MouseEvent {
                        x,
                        y,
                        kind: MouseEventKind::DoubleClick,
                    })
                } else {
                    Some(MouseEvent {
                        x,
                        y,
                        kind: MouseEventKind::LeftClick,
                    })
                }
            }
            CTMouseEventKind::Down(MouseButton::Right) => Some(MouseEvent {
                x,
                y,
                kind: MouseEventKind::RightClick,
            }),
            CTMouseEventKind::Down(MouseButton::Middle) => Some(MouseEvent {
                x,
                y,
                kind: MouseEventKind::MiddleClick,
            }),
            CTMouseEventKind::ScrollDown => Some(MouseEvent {
                x,
                y,
                kind: MouseEventKind::ScrollDown,
            }),
            CTMouseEventKind::ScrollUp => Some(MouseEvent {
                x,
                y,
                kind: MouseEventKind::ScrollUp,
            }),
            CTMouseEventKind::Up(_) => None,
            CTMouseEventKind::Drag(_) => None,
            CTMouseEventKind::Moved => None,
            CTMouseEventKind::ScrollLeft => None,
            CTMouseEventKind::ScrollRight => None,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct MouseEvent {
    pub x: u16,
    pub y: u16,
    pub kind: MouseEventKind,
}

#[derive(Debug, Clone, Copy)]
pub enum MouseEventKind {
    LeftClick,
    MiddleClick,
    RightClick,
    DoubleClick,
    ScrollUp,
    ScrollDown,
}

#[derive(Debug, Clone, Copy)]
pub struct TimedMouseEvent {
    x: u16,
    y: u16,
    time: Instant,
}

impl From<MouseEvent> for Option<TimedMouseEvent> {
    fn from(value: MouseEvent) -> Option<TimedMouseEvent> {
        if matches!(value.kind, MouseEventKind::LeftClick) {
            Some(TimedMouseEvent {
                time: Instant::now(),
                x: value.x,
                y: value.y,
            })
        } else {
            None
        }
    }
}

impl TimedMouseEvent {
    pub fn is_doubled(&self, x: u16, y: u16) -> bool {
        if self.x != x || self.y != y {
            return false;
        }

        self.time.elapsed() < DOUBLE_CLICK_TIMEOUT
    }
}

impl From<MouseEvent> for Position {
    fn from(value: MouseEvent) -> Self {
        Self { x: value.x, y: value.y }
    }
}
