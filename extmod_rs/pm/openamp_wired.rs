//! Wired `pm_mpy_openamp_*` accessors.
// symmetry: done

use super::openamp::openamp_export;
use py_rs::pm::mpy::pm_mpy_obj_t;

/// `pm_mpy_openamp___del__` — return the `__del__` export from `openamp`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_openamp___del__() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(openamp_export("__del__"))
}

/// `pm_mpy_openamp_send` — return the `send` export from `openamp`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_openamp_send() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(openamp_export("send"))
}

/// `pm_mpy_openamp_is_ready` — return the `is_ready` export from `openamp`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_openamp_is_ready() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(openamp_export("is_ready"))
}

/// `pm_mpy_openamp_ENDPOINT_ADDR_ANY` — return the `ENDPOINT_ADDR_ANY` export from `openamp`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_openamp_ENDPOINT_ADDR_ANY() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(openamp_export("ENDPOINT_ADDR_ANY"))
}

/// `pm_mpy_openamp_new_service_callback` — return the `new_service_callback` export from `openamp`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_openamp_new_service_callback() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(openamp_export("new_service_callback"))
}

/// `pm_mpy_openamp_Endpoint` — return the `Endpoint` export from `openamp`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_openamp_Endpoint() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(openamp_export("Endpoint"))
}

/// `pm_mpy_openamp_RemoteProc` — return the `RemoteProc` export from `openamp`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_openamp_RemoteProc() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(openamp_export("RemoteProc"))
}
