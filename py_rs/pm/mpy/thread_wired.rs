//! Wired `pm_mpy_thread_*` accessors.
// symmetry: done

use super::thread::thread_export;
use super::types::pm_mpy_obj_t;

/// `pm_mpy_thread_LockType` — return the `LockType` export from `_thread`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_thread_LockType() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(thread_export("LockType"))
}

/// `pm_mpy_thread_get_ident` — return the `get_ident` export from `_thread`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_thread_get_ident() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(thread_export("get_ident"))
}

/// `pm_mpy_thread_stack_size` — return the `stack_size` export from `_thread`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_thread_stack_size() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(thread_export("stack_size"))
}

/// `pm_mpy_thread_start_new_thread` — return the `start_new_thread` export from `_thread`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_thread_start_new_thread() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(thread_export("start_new_thread"))
}

/// `pm_mpy_thread_exit` — return the `exit` export from `_thread`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_thread_exit() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(thread_export("exit"))
}

/// `pm_mpy_thread_allocate_lock` — return the `allocate_lock` export from `_thread`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_thread_allocate_lock() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(thread_export("allocate_lock"))
}
