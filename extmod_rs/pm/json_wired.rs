//! Wired `pm_mpy_json_*` accessors.
// symmetry: done

use super::json::json_export;
use py_rs::pm::mpy::pm_mpy_obj_t;

/// `pm_mpy_json_dump` — return the `dump` export from `json`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_json_dump() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(json_export("dump"))
}

/// `pm_mpy_json_dumps` — return the `dumps` export from `json`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_json_dumps() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(json_export("dumps"))
}

/// `pm_mpy_json_load` — return the `load` export from `json`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_json_load() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(json_export("load"))
}

/// `pm_mpy_json_loads` — return the `loads` export from `json`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_json_loads() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(json_export("loads"))
}
