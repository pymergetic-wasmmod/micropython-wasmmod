//! Import hooks for the public MetalPython ABI.
// symmetry: stub

use super::types::{pm_mpy_obj_t, pm_mpy_status_t};

#[no_mangle]
pub unsafe extern "C" fn pm_mpy_import_import_module(
    _name: *const core::ffi::c_char,
) -> pm_mpy_obj_t {
    pm_mpy_obj_t::NULL
}

#[no_mangle]
pub unsafe extern "C" fn pm_mpy_import___import__(
    _n_args: usize,
    _args: *const pm_mpy_obj_t,
    _out: *mut pm_mpy_obj_t,
) -> pm_mpy_status_t {
    pm_mpy_status_t::Runtime
}

#[no_mangle]
pub unsafe extern "C" fn pm_mpy_import_stat(_path: *const core::ffi::c_char) -> i32 {
    -1
}
