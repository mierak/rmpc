use crate::mpd::{FromMpd, LineHandled, errors::MpdError};

#[derive(Debug, Clone, Copy, strum::Display)]
#[strum(serialize_all = "snake_case")]
pub enum IdleEvent {
    Player,   /* the player has been started, stopped or seeked or tags of
               * the currently playing song have changed (e.g.
               * received from stream) */
    Mixer,    // the volume has been changed
    Playlist, // the queue (i.e. the current playlist) has been modified
    Options,  // options like repeat, random, crossfade, replay gain
    Database, // the song database has been modified after update.
    Update,   /* a database update has started or finished. If the database
               * was modified during the update, the database
               * event is also emitted. */
    StoredPlaylist, /* a stored playlist has been modified, renamed,
                     * created or deleted */
    Output,       /* an audio output has been added, removed or modified (e.g.
                   * renamed, enabled or disabled) */
    Partition,    // a partition was added, removed or changed
    Sticker,      // the sticker database has been modified.
    Subscription, // a client has subscribed or unsubscribed to a channel
    Message,      /* a message was received on a channel this client is
                   * subscribed to; this event is only
                   * emitted when the queue is empty */
    Neighbor, // a neighbor was found or lost
    Mount,    // the mount list has changed
}

impl FromMpd for Vec<IdleEvent> {
    fn next_internal(&mut self, _key: &str, value: String) -> Result<LineHandled, MpdError> {
        match value.as_str() {
            "mixer" => self.push(IdleEvent::Mixer),
            "player" => self.push(IdleEvent::Player),
            "options" => self.push(IdleEvent::Options),
            "database" => self.push(IdleEvent::Database),
            "update" => self.push(IdleEvent::Update),
            "stored_playlist" => self.push(IdleEvent::StoredPlaylist),
            "playlist" => self.push(IdleEvent::Playlist),
            "output" => self.push(IdleEvent::Output),
            "partition" => self.push(IdleEvent::Partition),
            "sticker" => self.push(IdleEvent::Sticker),
            "subscription" => self.push(IdleEvent::Subscription),
            "message" => self.push(IdleEvent::Message),
            "neighbor" => self.push(IdleEvent::Neighbor),
            "mount" => self.push(IdleEvent::Mount),
            _ => return Ok(LineHandled::No { value }),
        };
        Ok(LineHandled::Yes)
    }
}
