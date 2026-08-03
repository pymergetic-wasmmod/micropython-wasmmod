//! Wired `pm_mpy_bluetooth_*` accessors.
// symmetry: done

use super::bluetooth::bluetooth_export;
use py_rs::pm::mpy::pm_mpy_obj_t;

/// `pm_mpy_bluetooth_BLE` — return the `BLE` export from `bluetooth`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_bluetooth_BLE() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(bluetooth_export("BLE"))
}

/// `pm_mpy_bluetooth_UUID` — return the `UUID` export from `bluetooth`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_bluetooth_UUID() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(bluetooth_export("UUID"))
}

/// `pm_mpy_bluetooth_FLAG_READ` — return the `FLAG_READ` export from `bluetooth`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_bluetooth_FLAG_READ() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(bluetooth_export("FLAG_READ"))
}

/// `pm_mpy_bluetooth_FLAG_WRITE` — return the `FLAG_WRITE` export from `bluetooth`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_bluetooth_FLAG_WRITE() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(bluetooth_export("FLAG_WRITE"))
}

/// `pm_mpy_bluetooth_FLAG_NOTIFY` — return the `FLAG_NOTIFY` export from `bluetooth`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_bluetooth_FLAG_NOTIFY() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(bluetooth_export("FLAG_NOTIFY"))
}

/// `pm_mpy_bluetooth_FLAG_INDICATE` — return the `FLAG_INDICATE` export from `bluetooth`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_bluetooth_FLAG_INDICATE() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(bluetooth_export("FLAG_INDICATE"))
}

/// `pm_mpy_bluetooth_FLAG_WRITE_NO_RESPONSE` — return the `FLAG_WRITE_NO_RESPONSE` export from `bluetooth`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_bluetooth_FLAG_WRITE_NO_RESPONSE() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(bluetooth_export("FLAG_WRITE_NO_RESPONSE"))
}
