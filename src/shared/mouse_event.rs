use std::time::{Duration, Instant};

use crossterm::event::{
    MouseButton,
    MouseEvent as CTMouseEvent,
    MouseEventKind as CTMouseEventKind,
};
use ratatui::layout::{Position, Rect};

// maybe make the timeout configurable?
const DOUBLE_CLICK_TIMEOUT: Duration = Duration::from_millis(500);

#[derive(Debug, Default, Clone, Copy)]
pub struct MouseEventTracker {
    last_left_click: Option<TimedMouseEvent>,
    drag_start_position: Option<Position>,
}

impl MouseEventTracker {
    pub fn track_and_get(&mut self, event: CTMouseEvent) -> Option<MouseEvent> {
        self.crossterm_ev_to_mouse_event(event).inspect(|ev| match ev.kind {
            MouseEventKind::LeftClick => {
                self.last_left_click = (*ev).into();
                self.drag_start_position = Some((*ev).into());
            }
            MouseEventKind::DoubleClick => {
                self.last_left_click = None;
                self.drag_start_position = None;
            }
            MouseEventKind::Drag { .. } => {
                // keep the drag start position until drag ends
            }
            _ => {
                self.drag_start_position = None;
            }
        })
    }

    pub fn crossterm_ev_to_mouse_event(&self, value: CTMouseEvent) -> Option<MouseEvent> {
        let x = value.column;
        let y = value.row;

        match value.kind {
            CTMouseEventKind::Down(MouseButton::Left) => {
                if self.last_left_click.is_some_and(|c| c.is_doubled(x, y)) {
                    Some(MouseEvent { x, y, kind: MouseEventKind::DoubleClick })
                } else {
                    Some(MouseEvent { x, y, kind: MouseEventKind::LeftClick })
                }
            }
            CTMouseEventKind::Down(MouseButton::Right) => {
                Some(MouseEvent { x, y, kind: MouseEventKind::RightClick })
            }
            CTMouseEventKind::Down(MouseButton::Middle) => {
                Some(MouseEvent { x, y, kind: MouseEventKind::MiddleClick })
            }
            CTMouseEventKind::ScrollDown => {
                Some(MouseEvent { x, y, kind: MouseEventKind::ScrollDown })
            }
            CTMouseEventKind::ScrollUp => Some(MouseEvent { x, y, kind: MouseEventKind::ScrollUp }),
            CTMouseEventKind::Up(_) => None,
            CTMouseEventKind::Drag(MouseButton::Left) => Some(MouseEvent {
                x,
                y,
                kind: MouseEventKind::Drag {
                    drag_start_position: self.drag_start_position.unwrap_or(Position { x, y }),
                },
            }),
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
    DoubleClick,
    MiddleClick,
    RightClick,
    ScrollDown,
    ScrollUp,
    Drag { drag_start_position: Position },
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
            Some(TimedMouseEvent { time: Instant::now(), x: value.x, y: value.y })
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

/// calculate the target index for scrollbar interaction based on mouse
/// position.
pub fn calculate_scrollbar_index(
    event: MouseEvent,
    scrollbar_area: Rect,
    content_len: usize,
) -> Option<usize> {
    let clicked_y = event.y.saturating_sub(scrollbar_area.y);
    let scrollbar_height = scrollbar_area.height;

    if content_len <= scrollbar_height as usize || scrollbar_height == 0 {
        return None;
    }

    let target_idx = if clicked_y >= scrollbar_height.saturating_sub(1) {
        // clicking at the bottom selects the last item
        content_len.saturating_sub(1)
    } else {
        let position_ratio = f64::from(clicked_y) / f64::from(scrollbar_height.saturating_sub(1));
        let target = (position_ratio * (content_len.saturating_sub(1)) as f64).round() as usize;
        target.min(content_len.saturating_sub(1))
    };

    Some(target_idx)
}

/// check if a mouse event should interact with the scrollbar, considering drag
/// start position
pub fn is_scrollbar_interaction(event: MouseEvent, scrollbar_area: Rect) -> bool {
    let scrollbar_x = scrollbar_area.right().saturating_sub(1);

    match event.kind {
        MouseEventKind::LeftClick => {
            // For clicks, require exact x position on scrollbar
            event.x == scrollbar_x && scrollbar_area.contains(event.into())
        }
        MouseEventKind::Drag { drag_start_position } => {
            // For drags, check if drag started on scrollbar or current position is on
            // scrollbar
            (drag_start_position.x == scrollbar_x && scrollbar_area.contains(drag_start_position))
                || (event.x == scrollbar_x && scrollbar_area.contains(event.into()))
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_scrollbar_index_basic() {
        let scrollbar_area = Rect::new(10, 5, 1, 10);
        let content_len = 20;

        let event = MouseEvent { x: 10, y: 5, kind: MouseEventKind::LeftClick };
        assert_eq!(calculate_scrollbar_index(event, scrollbar_area, content_len), Some(0));

        let event = MouseEvent { x: 10, y: 14, kind: MouseEventKind::LeftClick };
        assert_eq!(calculate_scrollbar_index(event, scrollbar_area, content_len), Some(19));

        let event = MouseEvent { x: 10, y: 9, kind: MouseEventKind::LeftClick };
        let result = calculate_scrollbar_index(event, scrollbar_area, content_len);
        assert!(result.is_some());
        let index = result.expect("result should be Some as confirmed by assertion");
        assert!(index > 0 && index < 19);
    }

    #[test]
    fn test_calculate_scrollbar_index_content_fits() {
        let scrollbar_area = Rect::new(10, 5, 1, 10);
        let content_len = 8; // Less than scrollbar height

        let event = MouseEvent { x: 10, y: 7, kind: MouseEventKind::LeftClick };
        assert_eq!(calculate_scrollbar_index(event, scrollbar_area, content_len), None);
    }

    #[test]
    fn test_calculate_scrollbar_index_edge_cases() {
        let scrollbar_area = Rect::new(10, 5, 1, 10);
        let content_len = 15;

        let event = MouseEvent { x: 10, y: 20, kind: MouseEventKind::LeftClick };
        assert_eq!(calculate_scrollbar_index(event, scrollbar_area, content_len), Some(14));

        let zero_height_area = Rect::new(10, 5, 1, 0);
        let event = MouseEvent { x: 10, y: 5, kind: MouseEventKind::LeftClick };
        assert_eq!(calculate_scrollbar_index(event, zero_height_area, content_len), None);
    }

    #[test]
    fn test_scrollbar_interaction_detection() {
        let scrollbar_area = Rect::new(10, 5, 1, 10);
        let scrollbar_x = scrollbar_area.right().saturating_sub(1); // x = 10

        let click_event = MouseEvent { x: scrollbar_x, y: 7, kind: MouseEventKind::LeftClick };
        assert!(is_scrollbar_interaction(click_event, scrollbar_area,));

        let click_event = MouseEvent { x: scrollbar_x + 1, y: 7, kind: MouseEventKind::LeftClick };
        assert!(!is_scrollbar_interaction(click_event, scrollbar_area,));

        let drag_start = Position { x: scrollbar_x, y: 7 };
        let drag_event = MouseEvent {
            x: scrollbar_x + 1,
            y: 9,
            kind: MouseEventKind::Drag { drag_start_position: drag_start },
        };
        assert!(is_scrollbar_interaction(drag_event, scrollbar_area));

        let drag_start_off = Position { x: scrollbar_x + 1, y: 7 };
        let drag_event = MouseEvent {
            x: scrollbar_x + 1,
            y: 9,
            kind: MouseEventKind::Drag { drag_start_position: drag_start_off },
        };
        assert!(!is_scrollbar_interaction(drag_event, scrollbar_area));

        let drag_event = MouseEvent {
            x: scrollbar_x,
            y: 9,
            kind: MouseEventKind::Drag { drag_start_position: drag_start },
        };
        assert!(is_scrollbar_interaction(drag_event, scrollbar_area));
    }

    #[test]
    fn test_calculate_scrollbar_index_drag_behavior() {
        let scrollbar_area = Rect::new(10, 5, 1, 10);
        let content_len = 20;

        let drag_event = MouseEvent {
            x: 10,
            y: 9,
            kind: MouseEventKind::Drag { drag_start_position: Position { x: 10, y: 9 } },
        };
        let result = calculate_scrollbar_index(drag_event, scrollbar_area, content_len);
        assert!(result.is_some());
        let index = result.expect("result should be Some as confirmed by assertion");
        assert!(index > 0 && index < 19);
    }
}
