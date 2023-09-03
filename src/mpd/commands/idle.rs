use anyhow::anyhow;
use anyhow::Context;

pub const COMMAND: &[u8; 4] = b"idle";

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

#[derive(Debug)]
pub struct IdleEvents(pub Vec<IdleEvent>);
impl TryFrom<String> for IdleEvents {
    type Error = anyhow::Error;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        let mut res = Vec::new();

        for line in value.lines() {
            let (key, value) = line
                .split_once(": ")
                .context(anyhow!("Invalid value '{}' whe parsing IdleEvent", line))?;
            match (key, value) {
                (_, "mixer") => res.push(IdleEvent::Mixer),
                (_, "player") => res.push(IdleEvent::Player),
                (_, "options") => res.push(IdleEvent::Options),
                (_, "database") => res.push(IdleEvent::Database),
                (_, "update") => res.push(IdleEvent::Update),
                (_, "stored_playlist") => res.push(IdleEvent::StoredPlaylist),
                (_, "playlist") => res.push(IdleEvent::Playlist),
                (_, "output") => res.push(IdleEvent::Output),
                (_, "partition") => res.push(IdleEvent::Partition),
                (_, "sticker") => res.push(IdleEvent::Sticker),
                (_, "subscription") => res.push(IdleEvent::Subscription),
                (_, "message") => res.push(IdleEvent::Message),
                (_, "neighbor") => res.push(IdleEvent::Neighbor),
                (_, "mount") => res.push(IdleEvent::Mount),
                _ => return Err(anyhow!("Cannot parse IdleEvent from string '{}'", value)),
            };
        }

        Ok(IdleEvents(res))
    }
}

impl std::str::FromStr for IdleEvents {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut res = Vec::new();

        for line in s.lines() {
            let (key, value) = line
                .split_once(": ")
                .context(anyhow!("Invalid value '{}' whe parsing IdleEvent", line))?;
            match (key, value) {
                (_, "mixer") => res.push(IdleEvent::Mixer),
                (_, "player") => res.push(IdleEvent::Player),
                (_, "options") => res.push(IdleEvent::Options),
                (_, "database") => res.push(IdleEvent::Database),
                (_, "update") => res.push(IdleEvent::Update),
                (_, "stored_playlist") => res.push(IdleEvent::StoredPlaylist),
                (_, "playlist") => res.push(IdleEvent::Playlist),
                (_, "output") => res.push(IdleEvent::Output),
                (_, "partition") => res.push(IdleEvent::Partition),
                (_, "sticker") => res.push(IdleEvent::Sticker),
                (_, "subscription") => res.push(IdleEvent::Subscription),
                (_, "message") => res.push(IdleEvent::Message),
                (_, "neighbor") => res.push(IdleEvent::Neighbor),
                (_, "mount") => res.push(IdleEvent::Mount),
                _ => return Err(anyhow!("Cannot parse IdleEvent from string '{}'", s)),
            };
        }
        Ok(Self(res))
    }
}
