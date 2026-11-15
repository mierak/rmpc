use std::sync::Arc;

use anyhow::{Context, Result, anyhow};
use tokio::sync::{RwLock, mpsc::UnboundedSender};
use tracing::error;
use zbus::names::WellKnownName;

use crate::{async_client::AsyncClient, ctx::Ctx};

mod metadata;
mod notify;
mod player;
mod root;
mod seek;
mod tracklist;
pub use notify::{Change, notify_consumer};
pub use player::Player;
pub use root::Root;
pub use tracklist::Tracklist;

const BUS_NAME: &str = "org.mpris.MediaPlayer2.mpd";
const DEFAULT_IDENTITY: &str = "Music Player Daemon";

fn bus_name(instance: Option<&str>) -> Result<WellKnownName<'static>> {
    let Some(instance) = instance else {
        return Ok(WellKnownName::try_from(BUS_NAME)?);
    };

    WellKnownName::try_from(format!("{BUS_NAME}.{instance}")).with_context(|| {
        format!(
            "Invalid mpris_instance '{instance}': use ASCII letters, digits, '_' or '-', and do \
             not start with a digit"
        )
    })
}

fn default_identity(instance: Option<&str>) -> String {
    match instance {
        Some(instance) => format!("{DEFAULT_IDENTITY} ({instance})"),
        None => DEFAULT_IDENTITY.to_owned(),
    }
}

pub async fn setup(
    client: Arc<AsyncClient>,
    mpd_state: Arc<RwLock<Ctx>>,
    instance: Option<String>,
    identity: Option<String>,
) -> Result<UnboundedSender<Change>> {
    let name = bus_name(instance.as_deref())?;
    let root = Root { identity: identity.unwrap_or_else(|| default_identity(instance.as_deref())) };
    let tracklist = Tracklist::new(mpd_state.clone());
    let player = Player::new(mpd_state.clone(), client);

    let conn = zbus::connection::Builder::session()
        .name(name.clone())
        .replace_existing_names(false)
        .allow_name_replacements(false)
        .serve_at("/org/mpris/MediaPlayer2", root)
        .serve_at("/org/mpris/MediaPlayer2", player)
        .serve_at("/org/mpris/MediaPlayer2", tracklist)
        .build()
        .await
        .map_err(|err| match err {
            zbus::Error::NameTaken => anyhow!(
                "Failed to start mpris: {name} is already owned by another player. Set \
                 mpris_instance to run more than one rmpcd with MPRIS"
            ),
            err => anyhow::Error::new(err).context("Failed to start mpris"),
        })?;

    let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<Change>();
    let conn_clone = conn.clone();

    tokio::spawn(async move {
        if let Err(err) = notify_consumer(&conn_clone, rx, mpd_state).await {
            error!(err = ?err, "Failed to start dbus notify consumer");
        }
    });

    Ok(tx)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bus_name_includes_instance() {
        assert_eq!(
            bus_name(Some("foo")).expect("valid").as_str(),
            "org.mpris.MediaPlayer2.mpd.foo"
        );
    }

    #[test]
    fn identity_includes_instance() {
        assert_eq!(default_identity(Some("foo")), "Music Player Daemon (foo)");
    }
}
