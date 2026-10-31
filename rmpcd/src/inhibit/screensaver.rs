use zbus::Connection;

use super::backend::{APP_NAME, Backend, REASON};

#[zbus::proxy(
    interface = "org.freedesktop.ScreenSaver",
    default_service = "org.freedesktop.ScreenSaver",
    default_path = "/org/freedesktop/ScreenSaver"
)]
trait ScreenSaverInterface {
    fn inhibit(&self, application_name: &str, reason_for_inhibit: &str) -> zbus::Result<u32>;
    fn un_inhibit(&self, cookie: u32) -> zbus::Result<()>;
}

pub(super) struct ScreenSaver {
    proxy: ScreenSaverInterfaceProxy<'static>,
    cookie: Option<u32>,
}

impl ScreenSaver {
    pub(super) async fn new() -> zbus::Result<Self> {
        let conn = Connection::session().await?;
        Ok(Self { proxy: ScreenSaverInterfaceProxy::new(&conn).await?, cookie: None })
    }
}

impl Backend for ScreenSaver {
    const NAME: &'static str = "org.freedesktop.ScreenSaver";

    fn is_inhibited(&self) -> bool {
        self.cookie.is_some()
    }

    async fn inhibit(&mut self) -> zbus::Result<()> {
        self.cookie = Some(self.proxy.inhibit(APP_NAME, REASON).await?);
        Ok(())
    }

    async fn un_inhibit(&mut self) -> zbus::Result<()> {
        if let Some(cookie) = self.cookie.take() {
            self.proxy.un_inhibit(cookie).await?;
        }
        Ok(())
    }
}
