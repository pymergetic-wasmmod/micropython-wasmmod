//! Wired `pm_mpy_heapq_*` accessors.
// symmetry: done

use super::heapq::heapq_export;
use py_rs::pm::mpy::pm_mpy_obj_t;

#[no_mangle]
pub unsafe extern "C" fn pm_mpy_heapq_heappush() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(heapq_export("heappush"))
}

#[no_mangle]
pub unsafe extern "C" fn pm_mpy_heapq_heappop() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(heapq_export("heappop"))
}

#[no_mangle]
pub unsafe extern "C" fn pm_mpy_heapq_heapify() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(heapq_export("heapify"))
}
