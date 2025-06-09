use std::{
    collections::VecDeque,
    io::{self, Write},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread::Builder,
    time::Duration,
};

use anyhow::Result;
use crossbeam::{
    channel::{Receiver, Sender, bounded},
    select,
};
use drop_guard::ClientDropGuard;

use crate::{
    config::Config,
    mpd::{client::Client, commands::idle::IdleEvent, errors::MpdError, mpd_client::MpdClient},
    shared::{
        events::{AppEvent, ClientRequest, WorkDone},
        macros::{status_error, try_break, try_skip},
    },
};

pub fn init(
    client_rx: Receiver<ClientRequest>,
    event_tx: Sender<AppEvent>,
    client: Client<'static>,
    config: Arc<Config>,
) -> io::Result<std::thread::JoinHandle<()>> {
    std::thread::Builder::new()
        .name("client".to_owned())
        .spawn(move || client_task(&client_rx, &event_tx, client, &config))
}

static HEALTHY: AtomicBool = AtomicBool::new(true);

macro_rules! health {
    ($e:expr, $msg:literal) => {
        {
            if HEALTHY.load(Ordering::Relaxed) == false {
                log::error!("Client is not healhty. Trying to end threads and reconnect");
                break;
            }

            match $e {
                Ok(v) => v,
                Err(e) => {
                    log::error!(error:? = e; $msg);
                    HEALTHY.store(false, Ordering::Relaxed);
                    break;
                }
            }
        }
    };
}

fn should_skip_request(buffer: &VecDeque<ClientRequest>, request: &ClientRequest) -> bool {
    buffer.iter().any(|request2| {
        if let (ClientRequest::Query(q1), ClientRequest::Query(q2)) = (&request, &request2) {
            q1.should_be_skipped(q2)
        } else {
            false
        }
    })
}

fn client_task(
    request_rx: &Receiver<ClientRequest>,
    event_tx: &Sender<AppEvent>,
    client: Client<'_>,
    config: &Config,
) {
    // TODO probably a good idea to drop the channels on each reconnect loop
    let mut first_loop = true;
    let (client_received_tx, client_received_rx) = &bounded::<()>(0);
    let (client_return_tx, client_return_rx) = &bounded::<Client<'_>>(1);

    std::thread::scope(|s| {
        client_return_tx.send(client).expect("Client init to succeed");

        loop {
            log::trace!(first_loop; "Starting worker threads");

            HEALTHY.store(true, Ordering::Relaxed);

            let _ = client_received_rx.try_iter().collect::<Vec<_>>();

            log::trace!(first_loop; "Trying to get returned client");
            let mut client = match client_return_rx.recv() {
                Ok(client) => client,
                Err(err) => {
                    log::error!(err:?; "Did not receive client from the return channel");
                    break;
                }
            };
            let is_client_ok =
                check_connection(first_loop, &mut client, request_rx, event_tx, config);
            first_loop = false;

            if is_client_ok {
                let mut client_write =
                    client.stream.try_clone().expect("Client write clone to succeed");

                let idle = Builder::new()
                    .name("idle".to_string())
                    .spawn_scoped(s, move || {
                        'outer: loop {
                            log::trace!("Waiting to acquire client");
                            let client = health!(client_return_rx.recv(), "Failed to receive client from request thread");
                            let mut client = ClientDropGuard::new(client_return_tx, client);
                            let timeout = config.mpd_idle_read_timeout_ms;
                            log::trace!(timeout:?; "Successfully acquired client, setting read timeout");

                            health!(client.set_read_timeout(config.mpd_idle_read_timeout_ms), "Failed to set read timeout for idle client");

                            log::trace!("Read timeout set, entering idle state");
                            let mut idle_client = health!(client.enter_idle(), "Failed to enter idle state");

                            log::trace!("Sending client received confirmation");
                            health!(client_received_tx.send_timeout((), Duration::from_secs(3)), "Failed to send client received confirmation");

                            log::trace!("Idle confirmation sent, waiting for events");
                            let events: Vec<IdleEvent> = loop {
                                match idle_client.read_response() {
                                    Ok(events) => break events,
                                    Err(MpdError::TimedOut(err)) => {
                                        if !HEALTHY.load(Ordering::Relaxed) {
                                            log::warn!(err:?; "Not healthy. Reading idle events timed out");
                                            break 'outer;
                                        }
                                    }
                                    Err(err) => {
                                        log::error!(err:?; "Encountered error while reading idle events");
                                        break 'outer
                                    }
                                }
                            };

                            log::trace!(events:?; "Got idle events");
                            for ev in events {
                                if let Err(err) = event_tx.send(AppEvent::IdleEvent(ev)) {
                                    log::error!(err:?; "Failed to send idle event");
                                    break 'outer;
                                }
                            }

                            log::trace!("Stopping idle, dropping client");
                            drop(client);
                            log::trace!("Client dropped, waiting for confirmation");
                            health!(client_received_rx.recv_timeout(Duration::from_secs(3)), "Did not receive confirmation from worker thread");
                            log::trace!("Confirmation received");
                        }
                        log::trace!("idle loop ended");
                    })
                    .expect("failed to spawn thread");

                try_skip!(client_return_tx.send(client), "Failed to request for client idle");
                try_break!(client_received_rx.recv(), "Idle confirmation failed");

                let work = Builder::new()
                    .name("request".to_string())
                    .spawn_scoped(s, move || {
                        let mut buffer = VecDeque::new();

                        loop {
                            log::trace!("Waiting for client requests");
                            let msg = select! {
                                recv(request_rx) -> msg => {
                                    health!(msg, "Failed to receive client request")
                                }
                                recv(client_return_rx) -> client => {
                                    // TODO
                                    let client = health!(client, "Failed to receive client request");
                                    ClientDropGuard::new(client_return_tx, client);
                                    break;
                                }
                            };
                            buffer.push_back(msg);

                            log::trace!(buffer:?; "Got requests. Trying to receive client from idle thread");
                            health!(client_write.write_all(b"noidle\n"), "Failed to write noidle command to MPD");
                            log::trace!("Sent noidle command to MPD");

                            let client = health!(client_return_rx.recv(), "Failed to receive client from idle thread");
                            let mut client = ClientDropGuard::new(client_return_tx, client);
                            log::trace!("Successfully received client from idle thread. Sending confirmation.");

                            health!(client_received_tx.send_timeout((), Duration::from_secs(3)), "Failed to send client received confirmation");

                            log::trace!(timeout:? = config.mpd_read_timeout; "Setting read timeout");
                            health!(client.set_read_timeout(Some(config.mpd_read_timeout)), "Failed to set read timeout");

                            while let Some(request) = buffer.pop_front() {
                                while let Ok(request) = request_rx.try_recv() {
                                    log::trace!(count = buffer.len(), buffer:?; "Got more requests");
                                    buffer.push_back(request);
                                }

                                if should_skip_request(&buffer, &request) {
                                    log::trace!(request:?; "Skipping duplicated request");
                                    continue;
                                }

                                match handle_client_request(&mut client, request) {
                                    Ok(result) => {
                                        try_break!(
                                            event_tx.send(AppEvent::WorkDone(Ok(result))),
                                            "Failed to send work done success event"
                                        );
                                    }
                                    Err(err) => {
                                        match err.downcast_ref::<MpdError>() {
                                            Some(MpdError::TimedOut(err)) => {
                                                status_error!(err:?; "Reading response from MPD timed out, will try to reconnect");
                                                health!(client.reconnect(), "Failed to reconnect");
                                                health!(client.set_write_timeout(Some(config.mpd_write_timeout)), "Failed to set write timeout");
                                                client_write = health!(client.stream.try_clone(), "Client write clone to succeed");
                                            },
                                            _ => {
                                                try_break!(
                                                    event_tx.send(AppEvent::WorkDone(Err(err))),
                                                    "Failed to send work done error event"
                                                );
                                            },
                                        }
                                    }
                                }
                            }

                            log::trace!("All requests processed, returning client to idle thread");
                            drop(client);
                            log::trace!("Client returned to idle thread. Waiting for confirmation");
                            health!(client_received_rx.recv_timeout(Duration::from_secs(3)), "Did not receive confirmation from idle thread");
                        }
                        log::debug!("Work loop ended. Shutting down MPD client.");
                        HEALTHY.store(false, Ordering::Relaxed);
                        try_skip!(client_write.shutdown_both(), "Failed to shutdown MPD client");
                    })
                    .expect("failed to spawn thread");

                idle.join().expect("idle thread not to panic");
                work.join().expect("work thread not to panic");
            } else {
                client_return_tx.send(client).expect("To be able to return the client");
            }

            let wait_time = std::time::Duration::from_secs(1);
            log::debug!(wait_time:?; "Lost connection to MPD, waiting before trying again");
            try_skip!(
                event_tx.send(AppEvent::LostConnection),
                "Failed to send lost connection event"
            );
            std::thread::sleep(wait_time);
        }
    });
}

mod drop_guard {
    use std::ops::DerefMut;

    use crossbeam::channel::Sender;

    use crate::mpd::client::Client;

    pub struct ClientDropGuard<'sender, 'client> {
        tx: &'sender Sender<Client<'client>>,
        client: Option<Client<'client>>,
    }

    impl<'sender, 'client> ClientDropGuard<'sender, 'client> {
        pub fn new(tx: &'sender Sender<Client<'client>>, client: Client<'client>) -> Self {
            Self { tx, client: Some(client) }
        }
    }

    impl Drop for ClientDropGuard<'_, '_> {
        fn drop(&mut self) {
            if let Some(client) = self.client.take() {
                log::warn!("Sending back client on drop");
                if let Err(err) = self.tx.send(client) {
                    log::error!(error:? = err; "ERROR DOPYCE, Failed to send client back on drop");
                }
            }
        }
    }

    impl<'client> std::ops::Deref for ClientDropGuard<'_, 'client> {
        type Target = Client<'client>;

        fn deref(&self) -> &Self::Target {
            self.client.as_ref().expect("Cannot deref because client was None")
        }
    }

    impl DerefMut for ClientDropGuard<'_, '_> {
        fn deref_mut(&mut self) -> &mut Self::Target {
            self.client.as_mut().expect("Cannot deref_mut because client was None")
        }
    }

    pub struct DropGuard<'a> {
        pub tx: &'a Sender<()>,
        pub name: &'a str,
    }

    impl Drop for DropGuard<'_> {
        fn drop(&mut self) {
            log::warn!(name = self.name; "sending drop notification");
            self.tx.send(()).expect("send to succeed");
        }
    }
}

fn check_connection(
    first_loop: bool,
    client: &mut Client<'_>,
    client_rx: &Receiver<ClientRequest>,
    event_tx: &Sender<AppEvent>,
    config: &Config,
) -> bool {
    if first_loop {
        true
    } else if client.reconnect().is_ok() {
        // empty the work queue after reconnect as they might no longer be
        // relevant
        let _ = client_rx.try_iter().collect::<Vec<_>>();
        try_skip!(event_tx.send(AppEvent::Reconnected), "Failed to send reconnected event");
        if let Err(err) = client.set_read_timeout(Some(config.mpd_read_timeout)) {
            log::error!(error:? = err; "Failed to set read timeout");
            return false;
        }
        if let Err(err) = client.set_write_timeout(Some(config.mpd_write_timeout)) {
            log::error!(error:? = err; "Failed to set write timeout");
            return false;
        }
        true
    } else {
        false
    }
}

fn handle_client_request(client: &mut Client<'_>, request: ClientRequest) -> Result<WorkDone> {
    match request {
        ClientRequest::Query(query) => Ok(WorkDone::MpdCommandFinished {
            id: query.id,
            target: query.target,
            data: (query.callback)(client)?,
        }),
        ClientRequest::Command(command) => {
            (command.callback)(client)?;
            Ok(WorkDone::None)
        }
        ClientRequest::QuerySync(query) => {
            let result = (query.callback)(client)?;
            query.tx.send(result)?;
            Ok(WorkDone::None)
        }
    }
}
