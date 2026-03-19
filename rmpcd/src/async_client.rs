use std::{
    io::Write,
    sync::{
        Arc,
        Condvar,
        Mutex as StdMutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};

use rmpc_mpd::{
    address::{MpdAddress, MpdPassword},
    commands::IdleEvent,
    mpd_client::MpdClient as _,
};
use tokio::{
    sync::{Mutex, mpsc, oneshot},
    time::timeout,
};
use tracing::{error, info, trace, warn};

pub type MpdClient = rmpc_mpd::client::Client<'static>;
pub type MpdError = rmpc_mpd::errors::MpdError;
pub type MpdStream = rmpc_mpd::client::TcpOrUnixStream;

type CmdFn = Box<dyn FnOnce(&mut MpdClient) -> Result<(), MpdError> + Send + 'static>;

enum Msg {
    Run { f: CmdFn, done: oneshot::Sender<Result<(), MpdError>> },
    Shutdown { done: oneshot::Sender<()> },
}

#[derive(Debug)]
struct Shared {
    in_idle: AtomicBool,

    wake_flag: StdMutex<bool>,
    wake_cv: Condvar,
}

fn notify_interrupter(shared: &Shared) {
    let mut flag = shared.wake_flag.lock().expect("Failed to lock interrupter wake flag");
    *flag = true;
    shared.wake_cv.notify_one();
}

fn spawn_interrupter(interrupt_stream: Arc<StdMutex<MpdStream>>, shared: Arc<Shared>) {
    thread::spawn(move || {
        loop {
            let mut flag = shared.wake_flag.lock().expect("Failed to lock interrupter wake flag");
            while !*flag {
                flag = shared.wake_cv.wait(flag).expect("Failed to wait on interrupter wake CV");
            }
            *flag = false;
            drop(flag);

            if shared.in_idle.load(Ordering::Acquire) {
                let mut stream = interrupt_stream.lock().expect("Failed to lock interrupt stream");
                let _ = stream.write_all(b"noidle\n");
                let _ = stream.flush();
            }
        }
    });
}

fn handle_msg(msg: Msg, client: &mut MpdClient, shutting_down: &mut bool) {
    match msg {
        Msg::Run { f, done } => {
            let r = f(client);
            let _ = done.send(r);
        }
        Msg::Shutdown { done } => {
            *shutting_down = true;
            let _ = done.send(());
        }
    }
}

fn worker_loop(
    mut client: MpdClient,
    mut rx: mpsc::Receiver<Msg>,
    shared: Arc<Shared>,
    interrupt_stream_slot: Arc<StdMutex<MpdStream>>,
    on_idle: impl Fn(Vec<IdleEvent>) + Send + Sync + 'static,
    on_reconnect: impl Fn() + Send + Sync + 'static,
) {
    thread::spawn(move || {
        let mut shutting_down = false;
        let reconnect_base_delay = std::time::Duration::from_millis(500);
        let reconnect_max_delay = std::time::Duration::from_secs(16);
        let mut skip_recv = false;

        'outer: while !shutting_down {
            if !skip_recv {
                trace!("Checking for pending commands...");
                let first = match rx.blocking_recv() {
                    Some(m) => m,
                    None => break,
                };
                trace!("Received command, draining pending commands...");
                handle_msg(first, &mut client, &mut shutting_down);
                while let Ok(msg) = rx.try_recv() {
                    handle_msg(msg, &mut client, &mut shutting_down);
                    if shutting_down {
                        break;
                    }
                }
                if shutting_down {
                    break;
                }
            }
            skip_recv = false;

            shared.in_idle.store(true, Ordering::Release);
            let idle_result = client.idle(None);
            shared.in_idle.store(false, Ordering::Release);

            match idle_result {
                Ok(changes) => {
                    if !changes.is_empty() {
                        on_idle(changes);
                    }
                }
                Err(e) => {
                    warn!(error = ?e, "Lost MPD connection, attempting reconnect");

                    let mut delay = reconnect_base_delay;
                    loop {
                        std::thread::sleep(delay);
                        match client.reconnect() {
                            Ok(_) => {
                                skip_recv = true;
                                match client.stream.try_clone() {
                                    Ok(new_stream) => {
                                        *interrupt_stream_slot
                                            .lock()
                                            .expect("lock interrupter stream") = new_stream;
                                    }
                                    Err(e) => {
                                        error!(error = ?e, "Failed to clone stream after reconnect");
                                    }
                                }
                                info!("Reconnected to MPD");
                                on_reconnect();
                                continue 'outer;
                            }
                            Err(e) => {
                                warn!(error = ?e, wait = ?delay, "Reconnect failed, retrying");
                                delay = (delay * 2).min(reconnect_max_delay);
                            }
                        }
                    }
                }
            }
        }
    });
}

#[derive(derive_more::Debug)]
pub struct AsyncClient {
    rx: Mutex<Option<mpsc::Receiver<Msg>>>,
    tx: mpsc::Sender<Msg>,
    shared: Arc<Shared>,
    #[debug(skip)]
    on_idle: Mutex<Option<Box<dyn Fn(Vec<IdleEvent>) + Send + Sync>>>,
    #[debug(skip)]
    on_reconnect: Mutex<Option<Box<dyn Fn() + Send + Sync>>>,
}

impl AsyncClient {
    pub fn new(
        on_idle: impl Fn(Vec<IdleEvent>) + Send + Sync + 'static,
        on_reconnect: impl Fn() + Send + Sync + 'static,
    ) -> Self {
        let (tx, rx) = mpsc::channel(64);

        let shared = Arc::new(Shared {
            in_idle: AtomicBool::new(false),
            wake_flag: StdMutex::new(false),
            wake_cv: Condvar::new(),
        });

        Self {
            rx: Mutex::new(Some(rx)),
            tx,
            shared,
            on_idle: Mutex::new(Some(Box::new(on_idle))),
            on_reconnect: Mutex::new(Some(Box::new(on_reconnect))),
        }
    }

    pub async fn connect(
        &self,
        address: MpdAddress,
        password: Option<MpdPassword>,
    ) -> anyhow::Result<()> {
        let client = MpdClient::init(address, password, "", None, false)?;
        let on_idle = self.on_idle.lock().await.take().expect("on_idle already taken");
        let on_reconnect =
            self.on_reconnect.lock().await.take().expect("on_reconnect already taken");
        let rx = self.rx.lock().await.take().expect("Receiver already taken");

        let interrupt_stream = client.stream.try_clone().expect("Client stream is not cloneable");
        let interrupt_stream_slot = Arc::new(StdMutex::new(interrupt_stream));

        spawn_interrupter(Arc::clone(&interrupt_stream_slot), self.shared.clone());
        worker_loop(client, rx, self.shared.clone(), interrupt_stream_slot, on_idle, on_reconnect);

        Ok(())
    }

    #[tracing::instrument(skip(self, f))]
    pub async fn run<F, T>(&self, f: F) -> Result<T, MpdError>
    where
        F: FnOnce(&mut MpdClient) -> Result<T, MpdError> + Send + 'static,
        T: Send + 'static,
    {
        let (typed_tx, typed_rx) = oneshot::channel::<Result<T, MpdError>>();
        let wrapper = Box::new(move |client: &mut MpdClient| -> Result<(), MpdError> {
            let r = f(client);
            let _ = typed_tx.send(r);
            Ok(())
        });

        let (done_tx, done_rx) = oneshot::channel();

        notify_interrupter(&self.shared);

        self.tx.send(Msg::Run { f: wrapper, done: done_tx }).await.expect("worker stopped");

        timeout(Duration::from_secs(10), done_rx)
            .await
            .map_err(|err| {
                MpdError::Generic(format!("Timed out waiting for done response: {err}"))
            })?
            .map_err(|err| {
                MpdError::Generic(format!("Failed to receive done response: {err}"))
            })??;

        timeout(Duration::from_secs(10), typed_rx)
            .await
            .map_err(|err| {
                MpdError::Generic(format!("Timed out waiting for typed response: {err}"))
            })?
            .map_err(|err| MpdError::Generic(format!("Failed to receive typed response: {err}")))?
    }

    pub async fn shutdown(&self) {
        let (done_tx, done_rx) = oneshot::channel();

        notify_interrupter(&self.shared);

        let _ = self.tx.send(Msg::Shutdown { done: done_tx }).await;
        let _ = done_rx.await;
    }
}
