//! Wired `pm_mpy_re_*` accessors.
// symmetry: done

use super::re::re_export;
use py_rs::pm::mpy::pm_mpy_obj_t;

#[no_mangle]
pub unsafe extern "C" fn pm_mpy_re_compile() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(re_export("compile"))
}

#[no_mangle]
pub unsafe extern "C" fn pm_mpy_re_match() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(re_export("match"))
}

#[no_mangle]
pub unsafe extern "C" fn pm_mpy_re_search() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(re_export("search"))
}

#[no_mangle]
pub unsafe extern "C" fn pm_mpy_re_sub() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(re_export("sub"))
}

#[no_mangle]
pub unsafe extern "C" fn pm_mpy_re_DEBUG() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(re_export("DEBUG"))
}
