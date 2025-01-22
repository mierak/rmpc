use std::collections::VecDeque;
use std::io::{self, Write};
use std::thread::Builder;

use anyhow::Result;
use crossbeam::channel::{Receiver, Sender, bounded, unbounded};
use crossbeam::select;
use drop_guard::{ClientDropGuard, DropGuard};

use crate::mpd::client::Client;
use crate::mpd::commands::idle::IdleEvent;
use crate::mpd::mpd_client::MpdClient;
use crate::shared::events::{AppEvent, ClientRequest, WorkDone};
use crate::shared::macros::{try_break, try_skip};

pub fn init(
    client_rx: Receiver<ClientRequest>,
    event_tx: Sender<AppEvent>,
    client: Client<'static>,
) -> io::Result<std::thread::JoinHandle<()>> {
    std::thread::Builder::new()
        .name("client task".to_owned())
        .spawn(move || client_task(&client_rx, &event_tx, client))
}

fn client_task(
    client_rx: &Receiver<ClientRequest>,
    event_tx: &Sender<AppEvent>,
    client: Client<'_>,
) {
    let (req2idle_tx, req2idle_rx) = &bounded::<Client<'_>>(0);
    let (idle2req_tx, idle2req_rx) = &bounded::<Client<'_>>(0);
    let (idle_entered_tx, idle_entered_rx) = &bounded::<()>(0);
    let (thread_end_ctx, thread_end_rx) = &unbounded::<()>();

    let (client_return_tx, client_return_rx) = &bounded::<Client<'_>>(1);
    client_return_tx.send(client).expect("Client init to succeed");

    std::thread::scope(|s| {
        let mut first_loop = true;
        loop {
            log::trace!(first_loop; "Starting worker threads");

            let _ = req2idle_rx.try_iter().collect::<Vec<_>>();
            let _ = idle2req_rx.try_iter().collect::<Vec<_>>();
            let _ = thread_end_rx.try_iter().collect::<Vec<_>>();
            let _ = idle_entered_rx.try_iter().collect::<Vec<_>>();

            log::trace!(first_loop; "Trying to get returned client");
            let mut client = match client_return_rx.recv() {
                Ok(client) => client,
                Err(err) => {
                    log::error!(err:?; "Did not receive client from the return channel");
                    break;
                }
            };
            let is_client_ok = check_connection(first_loop, &mut client, client_rx, event_tx);
            first_loop = false;

            if is_client_ok {
                let mut client_write =
                    client.stream.try_clone().expect("Client write clone to succeed");

                let idle = Builder::new()
                    .name("idle".to_string())
                    .spawn_scoped(s, move || {
                        let _g = DropGuard {
                            name: "idle",
                            tx: thread_end_ctx,
                        };

                        'outer: loop {
                            select! {
                                recv(req2idle_rx) -> client => {
                                    let mut client = match client {
                                        Ok(c) => ClientDropGuard::new(client_return_tx, c),
                                        Err(err) => {
                                            log::error!(err:?; "idle recv error");
                                            break;
                                        },
                                    };

                                    log::trace!("Trying to acquire client lock for idle");

                                    let mut idle_client = try_break!(client.enter_idle(), "Failed to enter idle state");
                                    try_break!(idle_entered_tx.send(()), "Failed to send idle confirmation");
                                    let events: Vec<IdleEvent> = try_break!(idle_client.read_response(), "Failed to read idle events");

                                    log::trace!(events:?; "Got idle events");
                                    for ev in events {
                                        if let Err(err) = event_tx.send(AppEvent::IdleEvent(ev)) {
                                            log::error!(err:?; "Failed to send idle event");
                                            break 'outer;
                                        }
                                    }
                                    try_break!(idle2req_tx.send(client.consume()), "Failed to return client to request thread");
                                }
                                recv(thread_end_rx) -> _ => {
                                    log::debug!("recv drop idle");
                                    break;
                                }
                            }
                            log::trace!("Stopping idle");
                        }
                        log::trace!("idle loop ended");
                    })
                    .expect("failed to spawn thread");

                try_skip!(req2idle_tx.send(client), "Failed to request for client idle");
                try_break!(idle_entered_rx.recv(), "Idle confirmation failed");

                let work = Builder::new()
                    .name("request".to_string())
                    .spawn_scoped(s, move || {
                        let _g = DropGuard {
                            name: "request",
                            tx: thread_end_ctx,
                        };
                        let mut buffer = VecDeque::new();

                        loop {
                            log::trace!("Waiting for client requests");
                            select! {
                                recv(client_rx) -> msg => {
                                    let Ok(msg) = msg else {
                                        continue;
                                    };

                                    buffer.push_back(msg);

                                    log::trace!(buffer:?; "Trying to receive client from idle thread");
                                    try_break!(client_write.write_all(b"noidle\n"), "Failed to write noidle");
                                    let client = try_break!(idle2req_rx.recv(), "Failed to receive client from idle thread");
                                    let mut client = ClientDropGuard::new(client_return_tx, client);

                                    while let Some(request) = buffer.pop_front() {
                                        while let Ok(request) = client_rx.try_recv() {
                                            log::trace!(count = buffer.len(), buffer:?; "Got more requests");
                                            buffer.push_back(request);
                                        }
                                        if buffer.iter().any(|request2| {
                                            if let (ClientRequest::Query(q1), ClientRequest::Query(q2)) =
                                                (&request, &request2)
                                            {
                                                q1.should_be_skipped(q2)
                                            } else {
                                                false
                                            }
                                        }) {
                                            log::trace!(request:?; "Skipping duplicated request");
                                            continue;
                                        }

                                        match handle_client_request(&mut client, request,) {
                                            Ok(result) => {
                                                try_break!(
                                                    event_tx.send(AppEvent::WorkDone(Ok(result))),
                                                    "Failed to send work done success event"
                                                );
                                            }
                                            Err(err) => {
                                                try_break!(
                                                    event_tx.send(AppEvent::WorkDone(Err(err))),
                                                    "Failed to send work done error event"
                                                );
                                            }
                                        }
                                    }

                                    log::trace!("Returning client lock to idle");
                                    try_break!(req2idle_tx.send(client.consume()), "Failed to request for client idle");
                                    try_break!(idle_entered_rx.recv(), "Idle confirmation failed");
                                },
                                recv(thread_end_rx) -> _ => {
                                    log::debug!("recv drop idle");
                                    break;
                                }
                            }
                        }
                        log::trace!("work loop ended");
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

        pub fn consume(mut self) -> Client<'client> {
            self.client
                .take()
                .expect("ClientDropGuard not to be in inconsistent state. Cannot consume self because client was None.")
        }
    }

    impl Drop for ClientDropGuard<'_, '_> {
        fn drop(&mut self) {
            if let Some(client) = self.client.take() {
                log::trace!("Sending back client on drop");
                self.tx.send(client).expect("send to succeed");
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
            log::trace!(name = self.name; "sending drop notification");
            self.tx.send(()).expect("send to succeed");
        }
    }
}

fn check_connection(
    first_loop: bool,
    client: &mut Client<'_>,
    client_rx: &Receiver<ClientRequest>,
    event_tx: &Sender<AppEvent>,
) -> bool {
    if first_loop {
        true
    } else if client.reconnect().is_ok() {
        client.set_read_timeout(None).expect("Read timeout set to succeed");

        // empty the work queue after reconnect as they might no longer be
        // relevant
        let _ = client_rx.try_iter().collect::<Vec<_>>();
        try_skip!(event_tx.send(AppEvent::Reconnected), "Failed to send reconnected event");
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
