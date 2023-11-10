pub mod macros {
    macro_rules! try_ret {
        ( $e:expr, $msg:literal ) => {
            match $e {
                Ok(x) => x,
                Err(e) => {
                    tracing::error!(message = $msg, error = ?e);
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
                    tracing::warn!(message = $msg, error = ?e);
                    continue
                },
            }
        };
    }
    #[allow(unused_imports)]
    pub(crate) use try_cont;
    pub(crate) use try_ret;
}
