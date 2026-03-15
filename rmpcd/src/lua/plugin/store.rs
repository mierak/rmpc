use crate::lua::plugin::{Triggers, triggers::TRIGGER_COUNT};

pub struct PluginStore<T> {
    items: Vec<T>,
    by_flag: [Vec<usize>; TRIGGER_COUNT],
}

impl<T> PluginStore<T> {
    pub fn new() -> Self {
        Self { items: Vec::new(), by_flag: std::array::from_fn(|_| Vec::new()) }
    }

    pub fn insert(&mut self, triggers: Triggers, item: T) {
        let idx = self.items.len();
        self.items.push(item);

        for (flag, slot) in [
            (Triggers::SongChange, flag_to_slot(Triggers::SongChange)),
            (Triggers::StateChange, flag_to_slot(Triggers::StateChange)),
            (Triggers::Message, flag_to_slot(Triggers::Message)),
            (Triggers::Idle, flag_to_slot(Triggers::Idle)),
            (Triggers::Shutdown, flag_to_slot(Triggers::Shutdown)),
        ] {
            if triggers.contains(flag) {
                self.by_flag[slot].push(idx);
            }
        }
    }

    pub fn all(&self) -> impl Iterator<Item = &T> {
        self.items.iter()
    }

    pub fn into_iter(self) -> impl Iterator<Item = T> {
        self.items.into_iter()
    }

    pub fn iter_with(&self, flag: Triggers) -> impl Iterator<Item = &T> {
        let slot = flag_to_slot(flag);
        self.by_flag[slot].iter().map(|&i| &self.items[i])
    }
}

#[inline]
fn flag_to_slot(flag: Triggers) -> usize {
    // It is a logical error to call this with more flags
    debug_assert!(flag.bits().is_power_of_two());
    flag.bits().trailing_zeros() as usize
}
