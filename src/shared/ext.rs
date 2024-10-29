pub mod error {
    use itertools::Itertools;

    use crate::mpd::errors::MpdError;

    pub trait ErrorExt {
        fn to_status(&self) -> String;
    }

    impl ErrorExt for anyhow::Error {
        fn to_status(&self) -> String {
            self.chain().map(|e| e.to_string().replace('\n', "")).join(" ")
        }
    }
    impl ErrorExt for MpdError {
        fn to_status(&self) -> String {
            match self {
                MpdError::Parse(e) => format!("Failed to parse: {e}"),
                MpdError::UnknownCode(e) => format!("Unkown code: {e}"),
                MpdError::Generic(e) => format!("Generic error: {e}"),
                MpdError::ClientClosed => "Client closed".to_string(),
                MpdError::Mpd(e) => format!("MPD Error: {e}"),
                MpdError::ValueExpected(e) => format!("Expected Value but got '{e}'"),
                MpdError::UnsupportedMpdVersion(e) => format!("Unsuported MPD version: {e}"),
            }
        }
    }
}

pub mod duration {
    pub trait DurationExt {
        fn to_string(&self) -> String;
    }

    impl DurationExt for std::time::Duration {
        fn to_string(&self) -> String {
            let secs = self.as_secs();
            let min = secs / 60;
            format!("{}:{:0>2}", min, secs - min * 60)
        }
    }
}

pub mod mpsc {
    #[allow(dead_code)]
    pub trait RecvLast<T> {
        fn recv_last(&self) -> Result<T, std::sync::mpsc::RecvError>;
        fn try_recv_last(&self) -> Result<T, std::sync::mpsc::TryRecvError>;
    }

    impl<T> RecvLast<T> for std::sync::mpsc::Receiver<T> {
        /// recv the last message in the channel and drop all the other ones
        fn recv_last(&self) -> Result<T, std::sync::mpsc::RecvError> {
            self.recv().map(|data| {
                let mut result = data;
                while let Ok(newer_data) = self.try_recv() {
                    result = newer_data;
                }
                result
            })
        }

        /// recv the last message in the channel in a non-blocking manner and drop all the other ones
        fn try_recv_last(&self) -> Result<T, std::sync::mpsc::TryRecvError> {
            self.try_recv().map(|data| {
                let mut result = data;
                while let Ok(newer_data) = self.try_recv() {
                    result = newer_data;
                }
                result
            })
        }
    }
}
