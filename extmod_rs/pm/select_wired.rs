//! Wired `pm_mpy_select_*` accessors.
// symmetry: done

use super::select::select_export;
use py_rs::pm::mpy::pm_mpy_obj_t;

/// `pm_mpy_select_select` — return the `select` export from `select`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_select_select() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(select_export("select"))
}

/// `pm_mpy_select_poll` — return the `poll` export from `select`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_select_poll() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(select_export("poll"))
}

/// `pm_mpy_select_POLLIN` — return the `POLLIN` export from `select`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_select_POLLIN() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(select_export("POLLIN"))
}

/// `pm_mpy_select_POLLOUT` — return the `POLLOUT` export from `select`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_select_POLLOUT() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(select_export("POLLOUT"))
}

/// `pm_mpy_select_POLLERR` — return the `POLLERR` export from `select`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_select_POLLERR() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(select_export("POLLERR"))
}

/// `pm_mpy_select_POLLHUP` — return the `POLLHUP` export from `select`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_select_POLLHUP() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(select_export("POLLHUP"))
}
