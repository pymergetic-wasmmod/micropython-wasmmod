//! Wired `pm_mpy_hashlib_*` accessors.
// symmetry: done

use super::hashlib::hashlib_export;
use py_rs::pm::mpy::pm_mpy_obj_t;

/// `pm_mpy_hashlib_sha256` — return the `sha256` export from `hashlib`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_hashlib_sha256() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(hashlib_export("sha256"))
}

/// `pm_mpy_hashlib_sha1` — return the `sha1` export from `hashlib`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_hashlib_sha1() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(hashlib_export("sha1"))
}

/// `pm_mpy_hashlib_md5` — return the `md5` export from `hashlib`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_hashlib_md5() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(hashlib_export("md5"))
}
