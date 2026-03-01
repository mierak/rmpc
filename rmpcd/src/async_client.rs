use std::{
    io::Write,
    sync::{
        Arc,
        Condvar,
        Mutex as StdMutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
};

use rmpc_mpd::{commands::IdleEvent, mpd_client::MpdClient as _};
use tokio::sync::{mpsc, oneshot};

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

fn spawn_interrupter(mut interrupt_stream: MpdStream, shared: Arc<Shared>) {
    thread::spawn(move || {
        loop {
            let mut flag = shared.wake_flag.lock().expect("Failed to lock interrupter wake flag");
            while !*flag {
                flag = shared.wake_cv.wait(flag).expect("Failed to wait on interrupter wake CV");
            }
            *flag = false;
            drop(flag);

            if shared.in_idle.load(Ordering::Acquire) {
                let _ = interrupt_stream.write_all(b"noidle\n");
                let _ = interrupt_stream.flush();
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
    on_idle: impl Fn(Vec<IdleEvent>) + Send + Sync + 'static,
) {
    thread::spawn(move || {
        let mut shutting_down = false;

        while !shutting_down {
            let first = match rx.blocking_recv() {
                Some(m) => m,
                None => break,
            };

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

            shared.in_idle.store(true, Ordering::Release);

            let idle_result = client.idle(None);

            shared.in_idle.store(false, Ordering::Release);

            match idle_result {
                Ok(changes) => {
                    if !changes.is_empty() {
                        on_idle(changes);
                    }
                }
                Err(_e) => {}
            }
        }
    });
}

#[derive(Debug)]
pub struct AsyncClient {
    tx: mpsc::Sender<Msg>,
    shared: Arc<Shared>,
}

impl AsyncClient {
    pub fn new(
        client: MpdClient,
        on_idle: impl Fn(Vec<IdleEvent>) + Send + Sync + 'static,
    ) -> Self {
        let (tx, rx) = mpsc::channel(64);

        let interrupt_stream = client.stream.try_clone().expect("Client stream is not cloneable");

        let shared = Arc::new(Shared {
            in_idle: AtomicBool::new(false),
            wake_flag: StdMutex::new(false),
            wake_cv: Condvar::new(),
        });

        spawn_interrupter(interrupt_stream, shared.clone());
        worker_loop(client, rx, shared.clone(), on_idle);

        Self { tx, shared }
    }

    #[tracing::instrument(skip(f))]
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

        done_rx.await.expect("worker dropped response")?;
        typed_rx.await.expect("typed response dropped")
    }

    pub async fn shutdown(&self) {
        let (done_tx, done_rx) = oneshot::channel();

        notify_interrupter(&self.shared);

        let _ = self.tx.send(Msg::Shutdown { done: done_tx }).await;
        let _ = done_rx.await;
    }
}
