macro_rules! try_ret {
        ( $e:expr, $msg:literal ) => {
            match $e {
                Ok(x) => x,
                Err(e) => {
                    log::error!(error:? = e; $msg);
                    return Err(anyhow::anyhow!("Message: '{}', inner error: '{:?}'", $msg, e))
                },
            }
        };
    }
macro_rules! try_cont {
        ( $e:expr, $msg:literal ) => {
            match $e {
                Ok(x) => x,
                Err(e) => {
                    log::warn!(error:? = e; $msg);
                    continue
                },
            }
        };
    }

macro_rules! try_skip {
        ( $e:expr, $msg:literal ) => {
            match $e {
                Ok(_) => {},
                Err(e) => {
                    log::warn!(error:? = e; $msg);
                },
            }
        };
    }

macro_rules! status_info {
        ($($t:tt)*) => {{
            log::info!($($t)*);
            log::info!(target: "{status_bar}", $($t)*);
        }};
    }

macro_rules! status_error {
        ($($t:tt)*) => {{
            log::error!($($t)*);
            log::error!(target: "{status_bar}", $($t)*);
        }};
    }

macro_rules! status_warn {
        ($($t:tt)*) => {{
            log::warn!($($t)*);
            log::warn!(target: "{status_bar}", $($t)*);
        }};
    }

macro_rules! modal {
    ( $i:ident, $e:expr ) => {{
        $i.app_event_sender
            .send(crate::AppEvent::UiEvent(crate::ui::UiAppEvent::Modal(
                crate::ui::ModalWrapper(Box::new($e)),
            )))?;
    }};
}

macro_rules! pop_modal {
    ( $i:ident ) => {{
        $i.app_event_sender
            .send(crate::AppEvent::UiEvent(crate::ui::UiAppEvent::PopModal))?;
    }};
}

pub(crate) use modal;
pub(crate) use pop_modal;

pub(crate) use status_error;
pub(crate) use status_info;
pub(crate) use status_warn;
pub(crate) use try_cont;
pub(crate) use try_ret;
pub(crate) use try_skip;
