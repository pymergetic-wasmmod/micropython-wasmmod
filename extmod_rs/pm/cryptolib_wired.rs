//! Wired `pm_mpy_cryptolib_*` accessors.
// symmetry: done

use super::cryptolib::cryptolib_export;
use py_rs::pm::mpy::pm_mpy_obj_t;

/// `pm_mpy_cryptolib_aes` — return the `aes` export from `cryptolib`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_cryptolib_aes() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(cryptolib_export("aes"))
}

/// `pm_mpy_cryptolib_MODE_ECB` — return the `MODE_ECB` export from `cryptolib`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_cryptolib_MODE_ECB() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(cryptolib_export("MODE_ECB"))
}

/// `pm_mpy_cryptolib_MODE_CBC` — return the `MODE_CBC` export from `cryptolib`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_cryptolib_MODE_CBC() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(cryptolib_export("MODE_CBC"))
}

/// `pm_mpy_cryptolib_MODE_CTR` — return the `MODE_CTR` export from `cryptolib`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_cryptolib_MODE_CTR() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(cryptolib_export("MODE_CTR"))
}
