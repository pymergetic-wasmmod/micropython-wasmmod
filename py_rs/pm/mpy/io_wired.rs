//! Wired `pm_mpy_io_*` accessors.
// symmetry: done

use super::io::io_export;
use super::types::pm_mpy_obj_t;

/// `pm_mpy_io_open` — return the `open` export from `io`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_io_open() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(io_export("open"))
}

/// `pm_mpy_io_IOBase` — return the `IOBase` export from `io`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_io_IOBase() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(io_export("IOBase"))
}

/// `pm_mpy_io_StringIO` — return the `StringIO` export from `io`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_io_StringIO() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(io_export("StringIO"))
}

/// `pm_mpy_io_BytesIO` — return the `BytesIO` export from `io`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_io_BytesIO() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(io_export("BytesIO"))
}

/// `pm_mpy_io_BufferedWriter` — return the `BufferedWriter` export from `io`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_io_BufferedWriter() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(io_export("BufferedWriter"))
}
