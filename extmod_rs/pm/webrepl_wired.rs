//! Wired `pm_mpy_webrepl_*` accessors.
// symmetry: done

use super::webrepl::webrepl_export;
use py_rs::pm::mpy::pm_mpy_obj_t;

#[no_mangle]
pub unsafe extern "C" fn pm_mpy_webrepl__webrepl() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(webrepl_export("_webrepl"))
}

#[no_mangle]
pub unsafe extern "C" fn pm_mpy_webrepl_password() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(webrepl_export("password"))
}
