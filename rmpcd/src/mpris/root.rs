use zbus::interface;

pub struct Root {}

#[interface(name = "org.mpris.MediaPlayer2")]
impl Root {
    #[zbus()]
    fn raise(&self) {}

    #[zbus()]
    fn quit(&self) {}

    #[zbus(property)]
    fn can_quit(&self) -> bool {
        false
    }

    #[zbus(property)]
    fn can_set_fullscreen(&self) -> bool {
        false
    }

    #[zbus(property)]
    fn can_raise(&self) -> bool {
        false
    }

    #[zbus(property)]
    fn has_track_list(&self) -> bool {
        true
    }

    #[zbus(property)]
    fn identity(&self) -> &'static str {
        "Music Player Daemon"
    }

    #[zbus(property)]
    fn desktop_entry(&self) -> &'static str {
        "rmpc"
    }

    #[zbus(property)]
    fn supported_uri_schemes(&self) -> &[&str] {
        &[]
    }

    #[zbus(property)]
    fn supported_mime_types(&self) -> &[&str] {
        &[]
    }
}
