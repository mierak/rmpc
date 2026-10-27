use zbus::{Connection, zvariant::OwnedFd};

use super::backend::{APP_NAME, Backend, REASON};

#[zbus::proxy(
    interface = "org.freedesktop.login1.Manager",
    default_service = "org.freedesktop.login1",
    default_path = "/org/freedesktop/login1",
    gen_blocking = false
)]
trait LoginManager {
    fn inhibit(&self, what: &str, who: &str, why: &str, mode: &str) -> zbus::Result<OwnedFd>;
}

pub(super) struct Logind {
    proxy: LoginManagerProxy<'static>,
    fd: Option<OwnedFd>,
}

impl Logind {
    pub(super) async fn new() -> zbus::Result<Self> {
        let conn = Connection::system().await?;
        Ok(Self { proxy: LoginManagerProxy::new(&conn).await?, fd: None })
    }
}

impl Backend for Logind {
    const NAME: &'static str = "org.freedesktop.login1";

    fn is_inhibited(&self) -> bool {
        self.fd.is_some()
    }

    async fn inhibit(&mut self) -> zbus::Result<()> {
        self.fd = Some(self.proxy.inhibit("idle", APP_NAME, REASON, "block").await?);
        Ok(())
    }

    #[allow(clippy::unused_async_trait_impl)]
    async fn un_inhibit(&mut self) -> zbus::Result<()> {
        self.fd = None;
        Ok(())
    }
}
