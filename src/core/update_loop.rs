use anyhow::Result;
use std::time::Duration;

use crossbeam::channel::{unbounded, Sender, TryRecvError};

use crate::{
    mpd::mpd_client::MpdClient,
    shared::{
        events::ClientRequest,
        mpd_query::{MpdQuery, MpdQueryResult},
    },
};

enum LoopEvent {
    Start,
    Stop,
}

#[derive(Debug)]
pub struct UpdateLoop {
    event_tx: Option<Sender<LoopEvent>>,
}

impl UpdateLoop {
    pub fn try_new(work_tx: Sender<ClientRequest>, status_update_interval_ms: Option<u64>) -> Result<Self> {
        let (tx, rx) = unbounded::<LoopEvent>();

        // send stop event at the start to not start the loop immedietally
        if let Err(err) = tx.send(LoopEvent::Stop) {
            log::error!(error:? = err; "Failed to properly initialize status update loop");
        }

        let Some(update_interval) = status_update_interval_ms.map(Duration::from_millis) else {
            return Ok(Self { event_tx: None });
        };
        std::thread::Builder::new().name("update".to_string()).spawn(move || {
            loop {
                match rx.try_recv() {
                    Ok(LoopEvent::Stop) => loop {
                        if let Ok(LoopEvent::Start) = rx.recv() {
                            break;
                        }
                    },
                    Err(TryRecvError::Disconnected) => {
                        log::error!("Render loop channel is disconnected");
                    }
                    Ok(LoopEvent::Start) | Err(TryRecvError::Empty) => {} // continue with the update loop
                }

                std::thread::sleep(update_interval);
                if let Err(err) = work_tx.send(ClientRequest::Query(MpdQuery {
                    id: "global_status_update",
                    target: None,
                    replace_id: None,
                    callback: Box::new(move |client| Ok(MpdQueryResult::Status(client.get_status()?))),
                })) {
                    log::error!(error:? = err; "Failed to send status update request");
                }
            }
        })?;
        Ok(Self { event_tx: Some(tx) })
    }

    pub fn start(&mut self) -> Result<()> {
        if let Some(tx) = &self.event_tx {
            Ok(tx.send(LoopEvent::Start)?)
        } else {
            Ok(())
        }
    }

    pub fn stop(&mut self) -> Result<()> {
        if let Some(tx) = &self.event_tx {
            Ok(tx.send(LoopEvent::Stop)?)
        } else {
            Ok(())
        }
    }
}
