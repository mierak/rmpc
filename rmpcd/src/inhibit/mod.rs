use tracing::warn;

use self::{
    backend::{Backend, set_backend},
    logind::Logind,
    screensaver::ScreenSaver,
};

mod backend;
mod logind;
mod screensaver;

pub struct IdleInhibitor {
    screensaver: Option<ScreenSaver>,
    logind: Option<Logind>,
}

impl IdleInhibitor {
    pub fn disabled() -> Self {
        Self { screensaver: None, logind: None }
    }

    pub async fn new() -> Self {
        let (screensaver, logind) = tokio::join!(ScreenSaver::new(), Logind::new());
        let inhibitor = Self {
            screensaver: screensaver.inspect_err(log_unavailable::<ScreenSaver>).ok(),
            logind: logind.inspect_err(log_unavailable::<Logind>).ok(),
        };

        if !inhibitor.is_enabled() {
            warn!("No idle inhibition backend available, idle inhibition is disabled");
        }

        inhibitor
    }

    fn is_enabled(&self) -> bool {
        self.screensaver.is_some() || self.logind.is_some()
    }

    pub async fn set(&mut self, playing: bool) {
        if !self.is_enabled() {
            return;
        }

        let mut any_inhibited = false;
        if let Some(backend) = &mut self.screensaver {
            any_inhibited |= set_backend(backend, playing).await;
        }
        if let Some(backend) = &mut self.logind {
            any_inhibited |= set_backend(backend, playing).await;
        }

        if playing && !any_inhibited {
            warn!("Failed to inhibit idle, no backend accepted the request");
        }
    }
}

fn log_unavailable<B: Backend>(err: &zbus::Error) {
    warn!(err = ?err, backend = B::NAME, "Idle inhibition backend unavailable");
}
