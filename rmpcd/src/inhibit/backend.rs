use tracing::{debug, trace, warn};

pub(super) const APP_NAME: &str = "rmpcd";
pub(super) const REASON: &str = "Playing music";

pub(super) trait Backend {
    const NAME: &'static str;

    fn is_inhibited(&self) -> bool;

    async fn inhibit(&mut self) -> zbus::Result<()>;

    async fn un_inhibit(&mut self) -> zbus::Result<()>;
}

pub(super) async fn set_backend<B: Backend>(backend: &mut B, playing: bool) -> bool {
    match (playing, backend.is_inhibited()) {
        (true, false) => match backend.inhibit().await {
            Ok(()) => trace!(backend = B::NAME, "Idle inhibited"),
            Err(err) => debug!(err = ?err, backend = B::NAME, "Failed to inhibit idle"),
        },
        (false, true) => match backend.un_inhibit().await {
            Ok(()) => trace!(backend = B::NAME, "Idle inhibition released"),
            Err(err) => warn!(err = ?err, backend = B::NAME, "Failed to release idle inhibition"),
        },
        _ => {}
    }
    backend.is_inhibited()
}
