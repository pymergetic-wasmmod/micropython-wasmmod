//! Wired `pm_mpy_marshal_*` accessors.
// symmetry: done

use super::marshal::marshal_export;
use py_rs::pm::mpy::pm_mpy_obj_t;

/// `pm_mpy_marshal_dumps` — return the `dumps` export from `marshal`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_marshal_dumps() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(marshal_export("dumps"))
}

/// `pm_mpy_marshal_loads` — return the `loads` export from `marshal`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_marshal_loads() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(marshal_export("loads"))
}
