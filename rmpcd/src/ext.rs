use tokio::sync::mpsc::UnboundedSender;
use tracing::error;

pub trait SenderExt<T> {
    fn send_safe(&self, message: T);
}

impl<T> SenderExt<T> for UnboundedSender<T> {
    fn send_safe(&self, message: T) {
        if let Err(err) = self.send(message) {
            error!(err = ?err, "Failed to send message");
        }
    }
}
