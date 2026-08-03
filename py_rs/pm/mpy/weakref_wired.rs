//! Wired `pm_mpy_weakref_*` accessors.
// symmetry: done

use super::types::pm_mpy_obj_t;
use super::weakref::weakref_export;

/// `pm_mpy_weakref_ref` — return the `ref` export from `weakref`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_weakref_ref() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(weakref_export("ref"))
}

/// `pm_mpy_weakref_finalize` — return the `finalize` export from `weakref`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_weakref_finalize() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(weakref_export("finalize"))
}
