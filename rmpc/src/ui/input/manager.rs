use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
};

use ratatui::{style::Style, text::Span};

use crate::ui::input::{BufferId, InputEvent, InputResultEvent, buffer::InputBuffer};

#[derive(Debug, Default, Clone, Copy, strum::EnumDiscriminants)]
#[strum_discriminants(derive(strum::Display, strum::AsRefStr))]
pub enum InputMode {
    #[default]
    Normal,
    Insert(BufferId),
}

#[derive(derive_more::Debug, Default)]
pub struct InputManager {
    mode: Cell<InputMode>,
    buffers: RefCell<HashMap<BufferId, InputBuffer>>,
}

macro_rules! buffer {
    ($self:expr, $id:expr) => {
        $self.buffers.borrow_mut().entry($id).or_insert(InputBuffer::default())
    };
}

impl InputManager {
    pub fn value(&self, id: BufferId) -> String {
        buffer!(self, id).value().to_owned()
    }

    pub fn create_buffer(&self, id: BufferId, initial_value: Option<&str>) {
        self.buffers.borrow_mut().remove(&id);
        self.buffers.borrow_mut().entry(id).or_insert(InputBuffer::new(initial_value));
    }

    pub fn as_spans(
        &self,
        id: BufferId,
        available_width: impl Into<usize>,
        style: Style,
        is_active: bool,
    ) -> Vec<Span<'static>> {
        buffer!(self, id).as_spans(None, available_width, style, is_active)
    }

    pub fn as_spans_prefixed(
        &self,
        id: BufferId,
        prefix: &'static str,
        available_width: impl Into<usize>,
        style: Style,
        is_active: bool,
    ) -> Vec<Span<'static>> {
        buffer!(self, id).as_spans(Some(prefix), available_width, style, is_active)
    }

    pub fn is_active(&self, id: BufferId) -> bool {
        match self.mode.get() {
            InputMode::Insert(active_id) => active_id == id,
            InputMode::Normal => false,
        }
    }

    pub fn destroy_buffer(&self, id: BufferId) {
        self.normal_mode();
        self.buffers.borrow_mut().remove(&id);
    }

    pub fn set_buffer(&self, value: String, id: BufferId) {
        buffer!(self, id).set_value(value);
    }

    pub fn clear_buffer(&self, id: BufferId) {
        buffer!(self, id).clear();
    }

    pub fn clear_all_buffers(&self) {
        self.normal_mode();
        self.buffers.borrow_mut().clear();
    }

    pub fn mode(&self) -> InputMode {
        self.mode.get()
    }

    pub fn is_insert_mode(&self) -> bool {
        matches!(self.mode.get(), InputMode::Insert(_))
    }

    pub fn is_normal_mode(&self) -> bool {
        matches!(self.mode.get(), InputMode::Normal)
    }

    pub fn insert_mode(&self, id: BufferId) {
        self.mode.replace(InputMode::Insert(id));
    }

    pub fn normal_mode(&self) {
        self.mode.replace(InputMode::Normal);
    }

    pub fn handle_input(&self, ev: Option<InputEvent>) -> Option<InputResultEvent> {
        let InputMode::Insert(id) = self.mode.get() else {
            return None;
        };

        Some(buffer!(self, id).handle_input(ev))
    }
}
