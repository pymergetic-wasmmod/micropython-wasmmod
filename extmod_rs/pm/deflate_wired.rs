//! Wired `pm_mpy_deflate_*` accessors.
// symmetry: done

use super::deflate::deflate_export;
use py_rs::pm::mpy::pm_mpy_obj_t;

/// `pm_mpy_deflate_DeflateIO` — return the `DeflateIO` export from `deflate`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_deflate_DeflateIO() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(deflate_export("DeflateIO"))
}

/// `pm_mpy_deflate_AUTO` — return the `AUTO` export from `deflate`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_deflate_AUTO() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(deflate_export("AUTO"))
}

/// `pm_mpy_deflate_RAW` — return the `RAW` export from `deflate`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_deflate_RAW() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(deflate_export("RAW"))
}

/// `pm_mpy_deflate_ZLIB` — return the `ZLIB` export from `deflate`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_deflate_ZLIB() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(deflate_export("ZLIB"))
}

/// `pm_mpy_deflate_GZIP` — return the `GZIP` export from `deflate`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_deflate_GZIP() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(deflate_export("GZIP"))
}
