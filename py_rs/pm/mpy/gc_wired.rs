//! Wired `pm_mpy_gc_*` accessors.
// symmetry: done

use super::gc::gc_export;
use super::types::pm_mpy_obj_t;

/// `pm_mpy_gc_collect` — return the `collect` export from `gc`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_gc_collect() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(gc_export("collect"))
}

/// `pm_mpy_gc_disable` — return the `disable` export from `gc`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_gc_disable() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(gc_export("disable"))
}

/// `pm_mpy_gc_enable` — return the `enable` export from `gc`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_gc_enable() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(gc_export("enable"))
}

/// `pm_mpy_gc_isenabled` — return the `isenabled` export from `gc`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_gc_isenabled() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(gc_export("isenabled"))
}

/// `pm_mpy_gc_mem_free` — return the `mem_free` export from `gc`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_gc_mem_free() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(gc_export("mem_free"))
}

/// `pm_mpy_gc_mem_alloc` — return the `mem_alloc` export from `gc`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_gc_mem_alloc() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(gc_export("mem_alloc"))
}

/// `pm_mpy_gc_threshold` — return the `threshold` export from `gc`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_gc_threshold() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(gc_export("threshold"))
}
