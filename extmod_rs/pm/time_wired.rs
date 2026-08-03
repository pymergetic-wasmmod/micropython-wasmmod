//! Wired `pm_mpy_time_*` accessors.
// symmetry: done

use super::time::time_export;
use py_rs::pm::mpy::pm_mpy_obj_t;

/// `pm_mpy_time_gmtime` — return the `gmtime` export from `time`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_time_gmtime() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(time_export("gmtime"))
}

/// `pm_mpy_time_localtime` — return the `localtime` export from `time`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_time_localtime() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(time_export("localtime"))
}

/// `pm_mpy_time_mktime` — return the `mktime` export from `time`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_time_mktime() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(time_export("mktime"))
}

/// `pm_mpy_time_time` — return the `time` export from `time`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_time_time() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(time_export("time"))
}

/// `pm_mpy_time_time_ns` — return the `time_ns` export from `time`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_time_time_ns() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(time_export("time_ns"))
}

/// `pm_mpy_time_sleep` — return the `sleep` export from `time`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_time_sleep() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(time_export("sleep"))
}

/// `pm_mpy_time_sleep_ms` — return the `sleep_ms` export from `time`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_time_sleep_ms() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(time_export("sleep_ms"))
}

/// `pm_mpy_time_sleep_us` — return the `sleep_us` export from `time`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_time_sleep_us() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(time_export("sleep_us"))
}

/// `pm_mpy_time_ticks_ms` — return the `ticks_ms` export from `time`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_time_ticks_ms() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(time_export("ticks_ms"))
}

/// `pm_mpy_time_ticks_us` — return the `ticks_us` export from `time`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_time_ticks_us() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(time_export("ticks_us"))
}

/// `pm_mpy_time_ticks_cpu` — return the `ticks_cpu` export from `time`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_time_ticks_cpu() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(time_export("ticks_cpu"))
}

/// `pm_mpy_time_ticks_add` — return the `ticks_add` export from `time`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_time_ticks_add() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(time_export("ticks_add"))
}

/// `pm_mpy_time_ticks_diff` — return the `ticks_diff` export from `time`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_time_ticks_diff() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(time_export("ticks_diff"))
}
