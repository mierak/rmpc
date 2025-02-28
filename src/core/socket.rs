use std::{
    io::{BufRead, BufReader},
    os::unix::net::UnixListener,
    path::PathBuf,
    sync::Arc,
};

use anyhow::{Context, Result};
use crossbeam::channel::Sender;

use crate::{
    AppEvent,
    WorkRequest,
    config::Config,
    shared::{
        macros::try_cont,
        socket::{SocketCommand, SocketCommandExecute, get_socket_path},
    },
};

pub(crate) fn init(
    event_tx: Sender<AppEvent>,
    work_tx: Sender<WorkRequest>,
    config: Arc<Config>,
) -> Result<SocketGuard> {
    let pid = std::process::id();
    let addr = get_socket_path(pid);
    let guard = SocketGuard(addr.clone());
    let listener = UnixListener::bind(&addr).context("Failed to bind to unix socket")?;

    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let stream = try_cont!(stream, "Failed to connect to socket client");
            let mut reader = BufReader::new(stream);

            let mut buf = String::new();
            try_cont!(reader.read_line(&mut buf), "Failed to read from socket client");
            let command: SocketCommand =
                try_cont!(serde_json::from_str(&buf), "Failed to parse socket command");

            log::debug!(command:?, addr:?; "Got command from unix socket");
            try_cont!(
                command.execute(&event_tx, &work_tx, &config),
                "Socket command execution failed"
            );
        }
    });
    Ok(guard)
}

/// The guard handles deletion of the unix domain socket upon dropping
#[must_use]
pub struct SocketGuard(PathBuf);
impl Drop for SocketGuard {
    fn drop(&mut self) {
        // Ingore, the app is exiting, theres nothing else we can do
        _ = std::fs::remove_file(&self.0);
    }
}
