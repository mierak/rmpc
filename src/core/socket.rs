use std::{
    io::{BufRead, BufReader},
    os::unix::net::UnixListener,
    path::PathBuf,
};

use anyhow::Result;
use crossbeam::channel::Sender;

use crate::{
    AppEvent,
    WorkRequest,
    config::Config,
    shared::socket::{SocketCommand, SocketCommandExecute, get_socket_path},
};

pub(crate) fn init(
    event_tx: Sender<AppEvent>,
    work_tx: Sender<WorkRequest>,
    config: &'static Config,
) -> SocketGuard {
    let pid = std::process::id();
    let addr = get_socket_path(pid);
    let guard = SocketGuard(addr.clone());
    let listener = UnixListener::bind(&addr).unwrap();

    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let stream = stream.unwrap();
            let mut reader = BufReader::new(stream);

            let mut buf = String::new();
            reader.read_line(&mut buf).unwrap();
            let command: SocketCommand = serde_json::from_str(&buf).unwrap();
            log::debug!(command:?; "got command");
            command.execute(&event_tx, &work_tx, config).unwrap();
        }
    });
    guard
}

/// The guard handles deletion of the unix domain socket upon dropping
pub struct SocketGuard(PathBuf);
impl Drop for SocketGuard {
    fn drop(&mut self) {
        // Ingore, the app is exiting, theres nothing else we can do
        _ = std::fs::remove_file(&self.0);
    }
}
