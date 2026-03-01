use anyhow::Result;
use rmpc_mpd::{
    client::Client,
    errors::{ErrorCode, MpdError, MpdFailureResponse},
    mpd_client::{AlbumArtOrder, MpdClient},
};

pub trait MpdExt {
    fn find_album_art(
        &mut self,
        path: &str,
        order: AlbumArtOrder,
    ) -> Result<Option<Vec<u8>>, MpdError>;
}

impl MpdExt for Client<'_> {
    fn find_album_art(
        &mut self,
        path: &str,
        order: AlbumArtOrder,
    ) -> Result<Option<Vec<u8>>, MpdError> {
        // path is already escaped in albumart() and read_picture()
        let first_result = match order {
            AlbumArtOrder::FileFirst | AlbumArtOrder::FileOnly => self.albumart(path),
            AlbumArtOrder::EmbeddedFirst | AlbumArtOrder::EmbeddedOnly => self.read_picture(path),
        };
        match first_result {
            Ok(Some(v)) => Ok(Some(v)),
            Ok(None) | Err(MpdError::Mpd(MpdFailureResponse { code: ErrorCode::NoExist, .. })) => {
                let second_result = match order {
                    AlbumArtOrder::FileFirst => self.read_picture(path),
                    AlbumArtOrder::EmbeddedFirst => self.albumart(path),
                    AlbumArtOrder::EmbeddedOnly | AlbumArtOrder::FileOnly => {
                        tracing::debug!(
                            "No album art found and no secondary method configured, falling back to placeholder image"
                        );
                        Ok(None)
                    }
                };
                match second_result {
                    Ok(Some(p)) => Ok(Some(p)),
                    Ok(None) => {
                        tracing::debug!("No album art found, falling back to placeholder image");
                        Ok(None)
                    }
                    Err(MpdError::Mpd(MpdFailureResponse { code: ErrorCode::NoExist, .. })) => {
                        tracing::debug!("No album art found, falling back to placeholder image");
                        Ok(None)
                    }
                    Err(e) => {
                        tracing::error!(error = ?e, "Failed to read picture. {}", e);
                        Ok(None)
                    }
                }
            }
            Err(e) => {
                tracing::error!(error = ?e, "Failed to read picture. {}", e);
                Ok(None)
            }
        }
    }
}
