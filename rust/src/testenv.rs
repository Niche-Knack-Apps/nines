//! Serialising tests that touch process-global state.
//!
//! Environment variables belong to the process, not to a test, and cargo runs
//! tests in threads. One test setting `NINES_THEME` to prove an override works
//! will, at the same moment, change what an unrelated test detects.
//!
//! One lock for the whole crate, deliberately. Two guards would each serialise
//! their own users and race each other, which is the bug this exists to stop.
//! Variables are restored when the guard drops, including on a panic, so one
//! failing test does not cascade.

use std::collections::HashMap;
use std::ffi::OsString;
use std::sync::{Mutex, MutexGuard, OnceLock};

fn lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

/// Exclusive access to the environment, restored on drop.
pub struct Env {
    _guard: MutexGuard<'static, ()>,
    saved: HashMap<&'static str, Option<OsString>>,
}

impl Env {
    /// Poisoning is recovered from: it means another test panicked while
    /// holding the guard, and the variables are restored either way.
    pub fn acquire() -> Self {
        let guard = lock().lock().unwrap_or_else(|e| e.into_inner());
        Self {
            _guard: guard,
            saved: HashMap::new(),
        }
    }

    pub fn set(&mut self, key: &'static str, value: impl AsRef<std::ffi::OsStr>) {
        self.remember(key);
        std::env::set_var(key, value);
    }

    pub fn remove(&mut self, key: &'static str) {
        self.remember(key);
        std::env::remove_var(key);
    }

    fn remember(&mut self, key: &'static str) {
        self.saved
            .entry(key)
            .or_insert_with(|| std::env::var_os(key));
    }
}

impl Drop for Env {
    fn drop(&mut self) {
        for (key, value) in self.saved.drain() {
            match value {
                Some(value) => std::env::set_var(key, value),
                None => std::env::remove_var(key),
            }
        }
    }
}
