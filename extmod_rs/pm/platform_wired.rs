//! Wired `pm_mpy_platform_*` accessors.
// symmetry: done

use super::platform::platform_export;
use py_rs::pm::mpy::pm_mpy_obj_t;

/// `pm_mpy_platform_platform` — return the `platform` export from `platform`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_platform_platform() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(platform_export("platform"))
}

/// `pm_mpy_platform_python_compiler` — return the `python_compiler` export from `platform`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_platform_python_compiler() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(platform_export("python_compiler"))
}

/// `pm_mpy_platform_libc_ver` — return the `libc_ver` export from `platform`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_platform_libc_ver() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(platform_export("libc_ver"))
}

/// `pm_mpy_platform_processor` — return the `processor` export from `platform`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_platform_processor() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(platform_export("processor"))
}
