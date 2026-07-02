use std::{
    io::Write,
    sync::{
        Arc,
        Mutex as StdMutex,
        atomic::{AtomicU8, Ordering},
    },
    thread,
    time::Duration,
};

use rmpc_mpd::{
    address::{MpdAddress, MpdPassword},
    commands::IdleEvent,
    mpd_client::MpdClient as _,
    proto_client::ProtoClient as _,
};
use tokio::{
    sync::{Mutex, mpsc, oneshot},
    time::timeout,
};
use tracing::{error, info, trace, warn};

pub type MpdClient = rmpc_mpd::client::Client<'static>;
pub type MpdError = rmpc_mpd::errors::MpdError;
pub type MpdStream = rmpc_mpd::client::TcpOrUnixStream;

type OnIdle = Box<dyn Fn(Vec<IdleEvent>) + Send + Sync>;
type OnReconnect = Box<dyn Fn() + Send + Sync>;
type RunFn = Box<dyn FnOnce(&mut MpdClient) + Send>;

const IDLE_STATE_NOT_IDLE: u8 = 0;
const IDLE_STATE_IDLE: u8 = 1;
const IDLE_STATE_PREEMPTED: u8 = 2;

enum Msg {
    Run(RunFn),
    Shutdown(oneshot::Sender<()>),
    SkipToIdle,
}

struct InitData {
    rx: mpsc::UnboundedReceiver<Msg>,
    on_idle: OnIdle,
    on_reconnect: OnReconnect,
}

#[derive(derive_more::Debug)]
struct Shared {
    #[debug(skip)]
    interrupt_stream: StdMutex<Option<MpdStream>>,
    idle_state: AtomicU8,
}

fn preempt_idle(shared: &Shared) {
    if shared
        .idle_state
        .compare_exchange(
            IDLE_STATE_IDLE,
            IDLE_STATE_PREEMPTED,
            Ordering::AcqRel,
            Ordering::Acquire,
        )
        .is_ok()
        && let Ok(mut guard) = shared.interrupt_stream.lock()
        && let Some(stream) = guard.as_mut()
    {
        let _ = stream.write_all(b"noidle\n");
        let _ = stream.flush();
    }
}

fn handle_msg(msg: Msg, client: &mut MpdClient, shutting_down: &mut bool) {
    match msg {
        Msg::Run(f) => f(client),
        Msg::SkipToIdle => {}
        Msg::Shutdown(done) => {
            *shutting_down = true;
            let _ = done.send(());
        }
    }
}

fn worker_loop(
    mut client: MpdClient,
    mut rx: mpsc::UnboundedReceiver<Msg>,
    shared: Arc<Shared>,
    on_idle: OnIdle,
    on_reconnect: OnReconnect,
) {
    thread::spawn(move || {
        let mut shutting_down = false;
        let reconnect_base_delay = Duration::from_millis(500);
        let reconnect_max_delay = Duration::from_secs(16);
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

            let idle_result: Result<Vec<IdleEvent>, _> = if let Err(e) = client.enter_idle(None) {
                Err(e)
            } else {
                shared.idle_state.store(IDLE_STATE_IDLE, Ordering::Release);
                if !rx.is_empty() {
                    preempt_idle(&shared);
                }
                let r = client.read_response();
                shared.idle_state.store(IDLE_STATE_NOT_IDLE, Ordering::Release);
                r
            };

            match idle_result {
                Ok(events) => {
                    if !events.is_empty() {
                        on_idle(events);
                    }
                }
                Err(e) => {
                    warn!(error = ?e, "Lost MPD connection, attempting reconnect");
                    shared.idle_state.store(IDLE_STATE_NOT_IDLE, Ordering::Release);

                    let mut delay = reconnect_base_delay;
                    loop {
                        while let Ok(msg) = rx.try_recv() {
                            if let Msg::Shutdown(done) = msg {
                                let _ = done.send(());
                                break 'outer;
                            }
                        }

                        thread::sleep(delay);
                        match client.reconnect() {
                            Ok(_) => {
                                skip_recv = true;
                                match client.stream.try_clone() {
                                    Ok(new_stream) => {
                                        if let Ok(mut guard) = shared.interrupt_stream.lock() {
                                            *guard = Some(new_stream);
                                        }
                                    }
                                    Err(e) => {
                                        error!(error = ?e, "Failed to clone stream after reconnect, will attempt reconnect again");
                                        continue;
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
    tx: mpsc::UnboundedSender<Msg>,
    shared: Arc<Shared>,
    #[debug(skip)]
    init: Mutex<Option<InitData>>,
}

impl AsyncClient {
    pub fn new(
        on_idle: impl Fn(Vec<IdleEvent>) + Send + Sync + 'static,
        on_reconnect: impl Fn() + Send + Sync + 'static,
    ) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();

        let shared = Arc::new(Shared {
            interrupt_stream: StdMutex::new(None),
            idle_state: AtomicU8::new(IDLE_STATE_NOT_IDLE),
        });

        let init =
            InitData { rx, on_idle: Box::new(on_idle), on_reconnect: Box::new(on_reconnect) };

        Self { tx, shared, init: Mutex::new(Some(init)) }
    }

    pub async fn connect(
        &self,
        address: MpdAddress,
        password: Option<MpdPassword>,
        enable_keepalive: bool,
    ) -> anyhow::Result<()> {
        let InitData { rx, on_idle, on_reconnect } = self
            .init
            .lock()
            .await
            .take()
            .ok_or_else(|| anyhow::anyhow!("AsyncClient::connect called more than once"))?;

        let client = MpdClient::init(address, password, "", None, false, enable_keepalive)?;
        let interrupt_stream = client
            .stream
            .try_clone()
            .map_err(|e| anyhow::anyhow!("Failed to clone MPD stream: {e}"))?;
        *self.shared.interrupt_stream.lock().expect("Failed to lock interrupt stream") =
            Some(interrupt_stream);

        worker_loop(client, rx, self.shared.clone(), on_idle, on_reconnect);

        Ok(())
    }

    #[tracing::instrument(skip(self, f))]
    pub async fn run<F, T>(&self, f: F) -> Result<T, MpdError>
    where
        F: FnOnce(&mut MpdClient) -> Result<T, MpdError> + Send + 'static,
        T: Send + 'static,
    {
        let (resp_tx, resp_rx) = oneshot::channel::<Result<T, MpdError>>();
        let closure: RunFn = Box::new(move |client| {
            let _ = resp_tx.send(f(client));
        });

        self.tx.send(Msg::Run(closure)).map_err(|_| MpdError::Generic("worker stopped".into()))?;
        preempt_idle(&self.shared);

        timeout(Duration::from_secs(10), resp_rx)
            .await
            .map_err(|err| MpdError::Generic(format!("Timed out waiting for response: {err}")))?
            .map_err(|err| MpdError::Generic(format!("Worker dropped response: {err}")))?
    }

    pub fn skip_to_idle(&self) {
        if self.tx.send(Msg::SkipToIdle).is_err() {
            warn!("skip_to_idle: worker stopped");
            return;
        }
        preempt_idle(&self.shared);
    }

    pub async fn shutdown(&self) {
        let (done_tx, done_rx) = oneshot::channel();

        if self.tx.send(Msg::Shutdown(done_tx)).is_err() {
            warn!("shutdown: worker stopped");
            return;
        }
        preempt_idle(&self.shared);
        if done_rx.await.is_err() {
            warn!("shutdown: worker exited without acknowledging");
        }
    }
}
