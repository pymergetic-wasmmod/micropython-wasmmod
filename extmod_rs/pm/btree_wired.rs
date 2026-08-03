//! Wired `pm_mpy_btree_*` accessors.
// symmetry: done

use super::btree::btree_export;
use py_rs::pm::mpy::pm_mpy_obj_t;

/// `pm_mpy_btree_open` — return the `open` export from `btree`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_btree_open() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(btree_export("open"))
}

/// `pm_mpy_btree_INCL` — return the `INCL` export from `btree`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_btree_INCL() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(btree_export("INCL"))
}

/// `pm_mpy_btree_DESC` — return the `DESC` export from `btree`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_btree_DESC() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(btree_export("DESC"))
}
