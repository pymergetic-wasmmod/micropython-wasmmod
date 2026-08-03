//! Serialize unit tests that touch global GC, qstr pool, or static type init.
// symmetry: done

use std::sync::{Mutex, MutexGuard};

static LOCK: Mutex<()> = Mutex::new(());

/// Hold for the whole test body when using `gc`, `qstr`, or `mpstate` from parallel `cargo test`.
pub fn lock() -> MutexGuard<'static, ()> {
    LOCK.lock().expect("vm test lock poisoned")
}
