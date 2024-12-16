use anyhow::{bail, Context, Result};
use std::{
    collections::VecDeque,
    io::{self, Write},
    sync::{Arc, Mutex},
    thread::Builder,
};

use crossbeam::{
    channel::{bounded, unbounded, Receiver, Sender},
    select,
};

use crate::shared::{
    events::{AppEvent, ClientRequest, WorkDone},
    macros::try_skip,
};
use crate::{
    mpd::{client::Client, commands::idle::IdleEvent, mpd_client::MpdClient},
    shared::macros::try_break,
};

pub fn init(
    client_rx: Receiver<ClientRequest>,
    client_tx: Sender<ClientRequest>,
    event_tx: Sender<AppEvent>,
    client: Client<'static>,
) -> io::Result<std::thread::JoinHandle<()>> {
    std::thread::Builder::new()
        .name("client task".to_owned())
        .spawn(move || client_task(&client_rx, &client_tx, &event_tx, client))
}

fn client_task(
    client_rx: &Receiver<ClientRequest>,
    client_tx: &Sender<ClientRequest>,
    event_tx: &Sender<AppEvent>,
    client: Client<'_>,
) {
    let (idle_initiate_tx, idle_initiate_rx) = &bounded::<()>(0);
    let (idle_confirm_tx, idle_confirm_rx) = &bounded::<()>(0);
    let (thread_end_ctx, thread_end_rx) = &unbounded::<()>();

    std::thread::scope(move |s| {
        let mut first_loop = true;
        let client = Arc::new(Mutex::new(client));

        loop {
            log::trace!(first_loop; "Starting worker threads");

            let _ = thread_end_rx.try_iter().collect::<Vec<_>>();
            let _ = idle_initiate_rx.try_iter().collect::<Vec<_>>();
            let _ = idle_confirm_rx.try_iter().collect::<Vec<_>>();

            let mut c = client.lock().expect("No other thread to hold client lock");
            let is_client_ok = check_connection(first_loop, &mut c, client_rx, event_tx);
            first_loop = false;

            if is_client_ok {
                let mut client_write = c.stream.try_clone().expect("Client write clone to succeed");
                drop(c);
                let client1 = client.clone();
                let client2 = client.clone();

                let idle = Builder::new()
                    .name("idle".to_string())
                    .spawn_scoped(s, move || {
                        let _g = DropGuard {
                            name: "idle",
                            tx: thread_end_ctx,
                        };

                        loop {
                            select! {
                                recv(idle_initiate_rx) -> msg => {
                                    if let Err(err) = msg {
                                        log::error!(err:?; "idle error");
                                        break;
                                    };
                                    if let Err(err) = listen_idle(&client1, idle_confirm_tx, event_tx,  client_tx) {
                                        log::error!(err:?; "idle error");
                                        break;
                                    }
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

                try_skip!(idle_initiate_tx.send(()), "Failed to request for client idle");
                try_skip!(idle_confirm_rx.recv(), "Idle confirmation failed");

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

                                    log::trace!(buffer:?; "Trying to acquire client lock");
                                    try_break!(client_write.write_all(b"noidle\n"), "Failed to write noidle");
                                    let mut client = try_break!(client2.lock(), "Failed to acquire client lock");

                                    while let Some(request) = buffer.pop_front() {
                                        while let Ok(request) = client_rx.try_recv() {
                                            log::trace!(count = buffer.len(), buffer:?; "Got more requests");
                                            buffer.push_back(request);
                                        }
                                        if buffer.iter().any(|request2| {
                                            if let (ClientRequest::MpdQuery(q1), ClientRequest::MpdQuery(q2)) =
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

                                    drop(client);
                                    log::trace!("Releasing client lock to idle");

                                    try_break!(idle_initiate_tx.send(()), "Failed to request for client idle");
                                    try_break!(idle_confirm_rx.recv(), "Idle confirmation failed");
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

fn listen_idle(
    client: &Arc<Mutex<Client<'_>>>,
    idle_confirm_tx: &Sender<()>,
    event_tx: &Sender<AppEvent>,
    client_tx: &Sender<ClientRequest>,
) -> Result<()> {
    log::trace!("Trying to acquire client lock for idle");
    let mut client = match client.lock() {
        Ok(c) => c,
        Err(err) => {
            log::error!(err:?; "Failed to acquire client lock");
            bail!("Failed to acquire client lock");
        }
    };

    let idle_client = client.enter_idle().context("Failed to enter idle state")?;
    idle_confirm_tx.send(()).context("Failed to send idle confirmation")?;
    let events: Vec<IdleEvent> = idle_client.read_response().context("Failed to read idle events")?;

    log::trace!(events:?; "Got idle events");
    for ev in events {
        event_tx
            .send(AppEvent::IdleEvent(ev))
            .context("Failed to send idle event")?;
    }
    if let Err(err) = client_tx.send(ClientRequest::CheckQueue) {
        log::error!(err:?; "Failed to send idle event");
        bail!("Failed to send idle event");
    };
    Ok(())
}

struct DropGuard<'a> {
    tx: &'a Sender<()>,
    name: &'a str,
}
impl Drop for DropGuard<'_> {
    fn drop(&mut self) {
        log::trace!(name = self.name; "sending drop notification");
        self.tx.send(()).expect("send to succeed");
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

        // empty the work queue after reconnect as they might no longer be relevant
        let _ = client_rx.try_iter().collect::<Vec<_>>();
        try_skip!(event_tx.send(AppEvent::Reconnected), "Failed to send reconnected event");
        true
    } else {
        false
    }
}

fn handle_client_request(client: &mut Client<'_>, request: ClientRequest) -> Result<WorkDone> {
    match request {
        ClientRequest::MpdQuery(query) => Ok(WorkDone::MpdCommandFinished {
            id: query.id,
            target: query.target,
            data: (query.callback)(client)?,
        }),
        ClientRequest::MpdCommand(command) => {
            (command.callback)(client)?;
            Ok(WorkDone::None)
        }
        ClientRequest::CheckQueue => Ok(WorkDone::None),
    }
}
