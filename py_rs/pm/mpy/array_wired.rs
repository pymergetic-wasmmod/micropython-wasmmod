//! Wired `pm_mpy_array_*` accessors.
// symmetry: done

use super::array::array_export;
use super::types::pm_mpy_obj_t;

/// `pm_mpy_array_array` — return the `array` export from `array`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_array_array() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(array_export("array"))
}
