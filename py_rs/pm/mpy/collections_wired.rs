//! Wired `pm_mpy_collections_*` accessors.
// symmetry: done

use super::collections::collections_export;
use super::types::pm_mpy_obj_t;

/// `pm_mpy_collections_deque` — return the `deque` export from `collections`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_collections_deque() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(collections_export("deque"))
}

/// `pm_mpy_collections_namedtuple` — return the `namedtuple` export from `collections`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_collections_namedtuple() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(collections_export("namedtuple"))
}

/// `pm_mpy_collections_OrderedDict` — return the `OrderedDict` export from `collections`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_collections_OrderedDict() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(collections_export("OrderedDict"))
}
