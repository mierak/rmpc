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

pub mod iter {
    use std::iter::Fuse;

    pub struct ZipLongest2<A, B, C>
    where
        A: Iterator,
        B: Iterator,
        C: Iterator,
    {
        iter_a: Fuse<A>,
        iter_b: Fuse<B>,
        iter_c: Fuse<C>,
    }

    impl<A, B, C> Iterator for ZipLongest2<A, B, C>
    where
        A: Iterator,
        B: Iterator,
        C: Iterator,
    {
        type Item = (
            Option<<A as Iterator>::Item>,
            Option<<B as Iterator>::Item>,
            Option<<C as Iterator>::Item>,
        );

        fn next(&mut self) -> Option<Self::Item> {
            match (self.iter_a.next(), self.iter_b.next(), self.iter_c.next()) {
                (None, None, None) => None,
                item => Some(item),
            }
        }
    }

    pub trait IntoZipLongest2: Iterator {
        fn zip_longest2<B: Iterator, C: Iterator>(self, b: B, c: C) -> ZipLongest2<Self, B, C>
        where
            Self: Sized;
    }

    impl<A: Iterator> IntoZipLongest2 for A {
        fn zip_longest2<B: Iterator, C: Iterator>(self, b: B, c: C) -> ZipLongest2<Self, B, C>
        where
            Self: Sized,
        {
            ZipLongest2 {
                iter_a: self.fuse(),
                iter_b: b.fuse(),
                iter_c: c.fuse(),
            }
        }
    }
}
