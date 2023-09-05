use crate::mpd::errors::MpdError;
use crate::mpd::{FromMpd, LineHandled};

#[derive(Debug)]
pub enum IdleEvent {
    Player, // the player has been started, stopped or seeked or tags of the currently playing song have changed (e.g. received from stream)
    Mixer,  // the volume has been changed
    Playlist, // the queue (i.e. the current playlist) has been modified
    Options, // options like repeat, random, crossfade, replay gain
    Database, // the song database has been modified after update.
    Update, // a database update has started or finished. If the database was modified during the update, the database event is also emitted.
    StoredPlaylist, // a stored playlist has been modified, renamed, created or deleted
    Output, // an audio output has been added, removed or modified (e.g. renamed, enabled or disabled)
    Partition, // a partition was added, removed or changed
    Sticker, // the sticker database has been modified.
    Subscription, // a client has subscribed or unsubscribed to a channel
    Message, // a message was received on a channel this client is subscribed to; this event is only emitted when the queue is empty
    Neighbor, // a neighbor was found or lost
    Mount,   // the mount list has changed
}

#[derive(Debug, Default)]
pub struct IdleEvents(pub Vec<IdleEvent>);

impl FromMpd for IdleEvents {
    fn finish(self) -> Result<Self, crate::mpd::errors::MpdError> {
        Ok(self)
    }

    fn next_internal(&mut self, _key: &str, value: String) -> Result<LineHandled, MpdError> {
        match value.as_str() {
            "mixer" => self.0.push(IdleEvent::Mixer),
            "player" => self.0.push(IdleEvent::Player),
            "options" => self.0.push(IdleEvent::Options),
            "database" => self.0.push(IdleEvent::Database),
            "update" => self.0.push(IdleEvent::Update),
            "stored_playlist" => self.0.push(IdleEvent::StoredPlaylist),
            "playlist" => self.0.push(IdleEvent::Playlist),
            "output" => self.0.push(IdleEvent::Output),
            "partition" => self.0.push(IdleEvent::Partition),
            "sticker" => self.0.push(IdleEvent::Sticker),
            "subscription" => self.0.push(IdleEvent::Subscription),
            "message" => self.0.push(IdleEvent::Message),
            "neighbor" => self.0.push(IdleEvent::Neighbor),
            "mount" => self.0.push(IdleEvent::Mount),
            _ => return Ok(LineHandled::No { value }),
        };
        Ok(LineHandled::Yes)
    }
}
