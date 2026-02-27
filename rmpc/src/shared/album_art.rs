use crate::{
    config::{album_art::ImageMethod, tabs::PaneType},
    ctx::Ctx,
    shared::{events::WorkRequest, mpd_client_ext::MpdClientExt as _, mpd_query::MpdQueryResult},
};

pub const ALBUM_ART: &str = "album_art";

/// returns none if album art is supposed to be hidden
pub fn fetch_album_art(ctx: &Ctx) -> Option<()> {
    if matches!(ctx.config.album_art.method, ImageMethod::None) {
        return None;
    }

    let (_, current_song) = ctx.find_current_song_in_queue()?;

    if let Some(loader) = &ctx.config.album_art.custom_loader {
        ctx.work_sender
            .send(WorkRequest::LoadAlbumArt {
                file: current_song.file.clone(),
                loader: loader.clone(),
            })
            .ok();
        return Some(());
    }

    let disabled_protos = &ctx.config.album_art.disabled_protocols;
    let song_uri = current_song.file.as_str();
    if disabled_protos.iter().any(|proto| song_uri.starts_with(proto)) {
        log::debug!(uri = song_uri; "Not downloading album art because the protocol is disabled");
        return None;
    }

    let song_uri = song_uri.to_owned();
    request_album_art_from_mpd(song_uri, ctx);

    Some(())
}

pub fn request_album_art_from_mpd(file: String, ctx: &Ctx) {
    let order = ctx.config.album_art.order;
    ctx.query().id(ALBUM_ART).replace_id(ALBUM_ART).target(PaneType::AlbumArt).query(move |client| {
            let start = std::time::Instant::now();
            log::debug!(file = file.as_str(); "Searching for album art");
            let result = client.find_album_art(&file, order)?;
            log::debug!(elapsed:? = start.elapsed(), size = result.as_ref().map(|v|v.len()); "Found album art");

            Ok(MpdQueryResult::AlbumArt(result))
        });
}
