//! Wired `pm_mpy_sys_*` accessors.
// symmetry: done

use super::sys::sys_export;
use super::types::pm_mpy_obj_t;

/// `pm_mpy_sys_argv` — return the `argv` export from `sys`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_sys_argv() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(sys_export("argv"))
}

/// `pm_mpy_sys_version` — return the `version` export from `sys`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_sys_version() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(sys_export("version"))
}

/// `pm_mpy_sys_version_info` — return the `version_info` export from `sys`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_sys_version_info() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(sys_export("version_info"))
}

/// `pm_mpy_sys_implementation` — return the `implementation` export from `sys`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_sys_implementation() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(sys_export("implementation"))
}

/// `pm_mpy_sys_platform` — return the `platform` export from `sys`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_sys_platform() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(sys_export("platform"))
}

/// `pm_mpy_sys_byteorder` — return the `byteorder` export from `sys`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_sys_byteorder() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(sys_export("byteorder"))
}

/// `pm_mpy_sys_maxsize` — return the `maxsize` export from `sys`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_sys_maxsize() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(sys_export("maxsize"))
}

/// `pm_mpy_sys_intern` — return the `intern` export from `sys`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_sys_intern() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(sys_export("intern"))
}

/// `pm_mpy_sys_exit` — return the `exit` export from `sys`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_sys_exit() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(sys_export("exit"))
}

/// `pm_mpy_sys_settrace` — return the `settrace` export from `sys`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_sys_settrace() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(sys_export("settrace"))
}

/// `pm_mpy_sys_stdin` — return the `stdin` export from `sys`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_sys_stdin() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(sys_export("stdin"))
}

/// `pm_mpy_sys_stdout` — return the `stdout` export from `sys`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_sys_stdout() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(sys_export("stdout"))
}

/// `pm_mpy_sys_stderr` — return the `stderr` export from `sys`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_sys_stderr() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(sys_export("stderr"))
}

/// `pm_mpy_sys_modules` — return the `modules` export from `sys`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_sys_modules() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(sys_export("modules"))
}

/// `pm_mpy_sys_exc_info` — return the `exc_info` export from `sys`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_sys_exc_info() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(sys_export("exc_info"))
}

/// `pm_mpy_sys_getsizeof` — return the `getsizeof` export from `sys`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_sys_getsizeof() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(sys_export("getsizeof"))
}

/// `pm_mpy_sys_executable` — return the `executable` export from `sys`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_sys_executable() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(sys_export("executable"))
}

/// `pm_mpy_sys_print_exception` — return the `print_exception` export from `sys`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_sys_print_exception() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(sys_export("print_exception"))
}

/// `pm_mpy_sys_atexit` — return the `atexit` export from `sys`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_sys_atexit() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(sys_export("atexit"))
}
