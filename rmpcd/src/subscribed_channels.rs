use std::{collections::HashMap, sync::Mutex as StdMutex};

#[derive(Debug, Default)]
pub(crate) struct SubscribedChannels {
    channels: StdMutex<HashMap<String, usize>>,
}

impl SubscribedChannels {
    pub(crate) fn increment(&self, channel: &str) -> usize {
        let mut channels = self.channels.lock().expect("Failed to lock subscribed channels");
        let count = channels.entry(channel.to_owned()).or_insert(0);
        *count += 1;
        *count
    }

    pub(crate) fn decrement(&self, channel: &str) -> Option<usize> {
        let mut channels = self.channels.lock().expect("Failed to lock subscribed channels");
        let count = channels.get_mut(channel)?;

        *count = count.saturating_sub(1);
        let remaining = *count;
        if remaining == 0 {
            channels.remove(channel);
        }

        Some(remaining)
    }

    pub(crate) fn list(&self) -> Vec<String> {
        let channels = self.channels.lock().expect("Failed to lock subscribed channels");
        channels.keys().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::SubscribedChannels;

    #[test]
    fn first_increment_returns_one() {
        let channels = SubscribedChannels::default();

        assert_eq!(channels.increment("a"), 1);
    }

    #[test]
    fn repeat_increment() {
        let channels = SubscribedChannels::default();

        assert_eq!(channels.increment("a"), 1);
        assert_eq!(channels.increment("a"), 2);
        assert_eq!(channels.increment("a"), 3);
    }

    #[test]
    fn distinct_channels_count_independently() {
        let channels = SubscribedChannels::default();
        channels.increment("a");

        assert_eq!(channels.increment("a"), 2);
        assert_eq!(channels.increment("b"), 1);
    }

    #[test]
    fn decrement_returns_remaining() {
        let channels = SubscribedChannels::default();
        channels.increment("a");
        channels.increment("a");

        assert_eq!(channels.decrement("a"), Some(1));
        assert_eq!(channels.list(), vec!["a".to_string()]);
    }

    #[test]
    fn last_decrement_returns_zero_and_removes_key() {
        let channels = SubscribedChannels::default();
        channels.increment("a");

        assert_eq!(channels.decrement("a"), Some(0));
        assert!(channels.list().is_empty());
    }

    #[test]
    fn decrement_unknown_channel_returns_none() {
        let channels = SubscribedChannels::default();
        channels.increment("a");

        assert_eq!(channels.decrement("b"), None);
        assert_eq!(channels.list(), vec!["a".to_string()]);
    }

    #[test]
    fn decrement_below_zero_returns_none() {
        let channels = SubscribedChannels::default();
        channels.increment("a");

        assert_eq!(channels.decrement("a"), Some(0));
        assert_eq!(channels.decrement("a"), None);
    }

    #[test]
    fn increment_after_decrement_to_zero() {
        let channels = SubscribedChannels::default();
        channels.increment("a");
        channels.decrement("a");

        assert_eq!(channels.increment("a"), 1);
    }
}
