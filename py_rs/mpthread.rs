//! rewrite of py/mpthread.h
// symmetry: done

use std::sync::OnceLock;

use crate::mpconfig;
use crate::mpstate::{self, ThreadState};

/// Opaque mutex storage (`mp_thread_mutex_t` / `pthread_mutex_t` on unix).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct ThreadMutex {
    pub(crate) storage: [u64; 8],
}

impl Default for ThreadMutex {
    fn default() -> Self {
        Self { storage: [0; 8] }
    }
}

/// Opaque recursive mutex storage (`mp_thread_recursive_mutex_t`).
#[repr(C)]
#[derive(Clone, Copy)]
pub struct ThreadRecursiveMutex {
    storage: [u64; 8],
}

/// Port threading backend registered by `ports_rs/*/mpthreadport`.
pub struct ThreadPort {
    pub get_state: fn() -> *mut ThreadState,
    pub set_state: fn(*mut ThreadState),
    pub create: fn(
        entry: extern "C" fn(*mut core::ffi::c_void) -> *mut core::ffi::c_void,
        arg: *mut core::ffi::c_void,
        stack_size: &mut usize,
    ) -> u64,
    pub get_id: fn() -> u64,
    pub start: fn(),
    pub finish: fn(),
    pub mutex_init: fn(*mut ThreadMutex),
    pub mutex_lock: fn(*const ThreadMutex, bool) -> i32,
    pub mutex_unlock: fn(*const ThreadMutex),
    pub recursive_mutex_init: fn(*mut ThreadRecursiveMutex),
    pub recursive_mutex_lock: fn(*const ThreadRecursiveMutex, bool) -> i32,
    pub recursive_mutex_unlock: fn(*const ThreadRecursiveMutex),
}

static PORT: OnceLock<ThreadPort> = OnceLock::new();

/// Register the host port threading implementation (call once from `mpthreadport::init`).
pub fn register_port(port: ThreadPort) {
    let _ = PORT.set(port);
}

fn port() -> Option<&'static ThreadPort> {
    PORT.get()
}

/// `mp_thread_get_state`.
#[inline]
pub fn get_state() -> *mut ThreadState {
    if !mpconfig::PY_THREAD {
        return mpstate::main_thread_ptr();
    }
    if let Some(p) = port() {
        let ts = (p.get_state)();
        if !ts.is_null() {
            return ts;
        }
    }
    mpstate::main_thread_ptr()
}

/// `mp_thread_set_state`.
#[inline]
pub fn set_state(state: *mut ThreadState) {
    if mpconfig::PY_THREAD {
        if let Some(p) = port() {
            (p.set_state)(state);
        }
    }
}

/// `mp_thread_create`.
pub fn create(
    entry: extern "C" fn(*mut core::ffi::c_void) -> *mut core::ffi::c_void,
    arg: *mut core::ffi::c_void,
    stack_size: &mut usize,
) -> u64 {
    if !mpconfig::PY_THREAD {
        return 0;
    }
    (port().expect("thread port not registered").create)(entry, arg, stack_size)
}

/// `mp_thread_get_id`.
pub fn get_id() -> u64 {
    if !mpconfig::PY_THREAD {
        return 0;
    }
    port().map(|p| (p.get_id)()).unwrap_or(0)
}

/// `mp_thread_start`.
pub fn start() {
    if let Some(p) = port() {
        (p.start)();
    }
}

/// `mp_thread_finish`.
pub fn finish() {
    if let Some(p) = port() {
        (p.finish)();
    }
}

/// `mp_thread_mutex_init`.
pub fn mutex_init(mutex: &mut ThreadMutex) {
    if let Some(p) = port() {
        (p.mutex_init)(mutex);
    }
}

/// `mp_thread_mutex_lock` — returns 1 on success, 0 if busy (no wait), negative errno otherwise.
pub fn mutex_lock(mutex: &ThreadMutex, wait: bool) -> i32 {
    if !mpconfig::PY_THREAD {
        return 1;
    }
    port().map(|p| (p.mutex_lock)(mutex, wait)).unwrap_or(1)
}

/// `mp_thread_mutex_unlock`.
pub fn mutex_unlock(mutex: &ThreadMutex) {
    if let Some(p) = port() {
        (p.mutex_unlock)(mutex);
    }
}

/// `mp_thread_recursive_mutex_init`.
pub fn recursive_mutex_init(mutex: &mut ThreadRecursiveMutex) {
    if let Some(p) = port() {
        (p.recursive_mutex_init)(mutex);
    }
}

/// `mp_thread_recursive_mutex_lock`.
pub fn recursive_mutex_lock(mutex: &ThreadRecursiveMutex, wait: bool) -> i32 {
    if !mpconfig::PY_THREAD {
        return 1;
    }
    port()
        .map(|p| (p.recursive_mutex_lock)(mutex, wait))
        .unwrap_or(1)
}

/// `mp_thread_recursive_mutex_unlock`.
pub fn recursive_mutex_unlock(mutex: &ThreadRecursiveMutex) {
    if let Some(p) = port() {
        (p.recursive_mutex_unlock)(mutex);
    }
}

/// `MP_THREAD_GIL_ENTER`.
#[inline]
pub fn thread_gil_enter() {
    if mpconfig::PY_THREAD && mpconfig::PY_THREAD_GIL {
        mpstate::with_vm(|vm| {
            let _ = mutex_lock(&mut vm.gil_mutex, true);
        });
    }
}

/// `MP_THREAD_GIL_EXIT`.
#[inline]
pub fn thread_gil_exit() {
    if mpconfig::PY_THREAD && mpconfig::PY_THREAD_GIL {
        mpstate::with_vm(|vm| {
            mutex_unlock(&vm.gil_mutex);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gil_macros_noop_without_gil() {
        thread_gil_enter();
        thread_gil_exit();
    }
}
