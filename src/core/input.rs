use std::time::Duration;

use crossbeam::channel::Sender;
use crossterm::event::Event;

use crate::shared::{events::AppEvent, mouse_event::MouseEventTracker};

pub fn init(event_tx: Sender<AppEvent>) -> std::io::Result<std::thread::JoinHandle<()>> {
    std::thread::Builder::new().name("input".to_owned()).spawn(move || input_poll_task(&event_tx))
}

fn input_poll_task(event_tx: &Sender<AppEvent>) {
    let mut mouse_event_tracker = MouseEventTracker::default();
    loop {
        match crossterm::event::poll(Duration::from_millis(250)) {
            Ok(true) => match crossterm::event::read() {
                Ok(Event::Mouse(mouse)) => {
                    if let Some(ev) = mouse_event_tracker.track_and_get(mouse)
                        && let Err(err) = event_tx.send(AppEvent::UserMouseInput(ev))
                    {
                        log::error!(error:? = err; "Failed to send user mouse input");
                    }
                }
                Ok(Event::Key(key)) => {
                    if let Err(err) = event_tx.send(AppEvent::UserKeyInput(key)) {
                        log::error!(error:? = err; "Failed to send user input");
                    }
                }
                Ok(Event::Resize(columns, rows)) => {
                    if let Err(err) = event_tx.send(AppEvent::Resized { columns, rows }) {
                        log::error!(error:? = err; "Failed to render request after resize");
                    }
                }
                Ok(ev) => {
                    log::warn!(ev:?; "Unexpected event");
                }
                Err(err) => {
                    log::warn!(error:? = err; "Failed to read input event");
                }
            },
            Ok(_) => {}
            Err(e) => log::warn!(error:? = e; "Error when polling for event"),
        }
    }
}
