use std::fs::TryLockError;

use crate::paths::Paths;

#[must_use]
pub fn acquire() -> std::fs::File {
    let lock_handle = match std::fs::File::options()
        .write(true)
        .create(true)
        .truncate(true)
        .open(Paths::runtime_dir().join("instance.lock"))
    {
        Ok(v) => v,
        Err(err) => {
            eprintln!("Failed to open lock file: {err:?}");
            std::process::exit(1);
        }
    };

    match lock_handle.try_lock() {
        Ok(()) => {}
        Err(TryLockError::WouldBlock) => {
            eprintln!("Another instance of rmpcd is already running, exiting...");
            std::process::exit(1);
        }
        Err(err) => {
            eprintln!("Failed to acquire instance lock: {err:?}");
            std::process::exit(1);
        }
    }

    lock_handle
}
