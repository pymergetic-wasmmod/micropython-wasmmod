//! Wired `pm_mpy_string_*` accessors.
// symmetry: done

use super::string::string_export;
use super::types::pm_mpy_obj_t;

/// `pm_mpy_string_Template` — return the `Template` export from `string`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_string_Template() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(string_export("Template"))
}

/// `pm_mpy_string_Interpolation` — return the `Interpolation` export from `string`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_string_Interpolation() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(string_export("Interpolation"))
}
