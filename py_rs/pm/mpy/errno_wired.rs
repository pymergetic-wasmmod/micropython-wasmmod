//! Wired `pm_mpy_errno_*` accessors.
// symmetry: done

use super::errno::errno_export;
use super::types::pm_mpy_obj_t;

/// `pm_mpy_errno_errorcode` — return the `errorcode` export from `errno`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_errno_errorcode() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(errno_export("errorcode"))
}

/// `pm_mpy_errno_EPERM` — return the `EPERM` export from `errno`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_errno_EPERM() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(errno_export("EPERM"))
}

/// `pm_mpy_errno_ENOENT` — return the `ENOENT` export from `errno`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_errno_ENOENT() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(errno_export("ENOENT"))
}

/// `pm_mpy_errno_EIO` — return the `EIO` export from `errno`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_errno_EIO() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(errno_export("EIO"))
}

/// `pm_mpy_errno_EBADF` — return the `EBADF` export from `errno`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_errno_EBADF() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(errno_export("EBADF"))
}

/// `pm_mpy_errno_EAGAIN` — return the `EAGAIN` export from `errno`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_errno_EAGAIN() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(errno_export("EAGAIN"))
}

/// `pm_mpy_errno_ENOMEM` — return the `ENOMEM` export from `errno`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_errno_ENOMEM() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(errno_export("ENOMEM"))
}

/// `pm_mpy_errno_EACCES` — return the `EACCES` export from `errno`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_errno_EACCES() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(errno_export("EACCES"))
}

/// `pm_mpy_errno_EEXIST` — return the `EEXIST` export from `errno`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_errno_EEXIST() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(errno_export("EEXIST"))
}

/// `pm_mpy_errno_ENODEV` — return the `ENODEV` export from `errno`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_errno_ENODEV() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(errno_export("ENODEV"))
}

/// `pm_mpy_errno_EISDIR` — return the `EISDIR` export from `errno`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_errno_EISDIR() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(errno_export("EISDIR"))
}

/// `pm_mpy_errno_EINVAL` — return the `EINVAL` export from `errno`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_errno_EINVAL() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(errno_export("EINVAL"))
}

/// `pm_mpy_errno_EOPNOTSUPP` — return the `EOPNOTSUPP` export from `errno`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_errno_EOPNOTSUPP() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(errno_export("EOPNOTSUPP"))
}

/// `pm_mpy_errno_EADDRINUSE` — return the `EADDRINUSE` export from `errno`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_errno_EADDRINUSE() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(errno_export("EADDRINUSE"))
}

/// `pm_mpy_errno_ECONNABORTED` — return the `ECONNABORTED` export from `errno`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_errno_ECONNABORTED() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(errno_export("ECONNABORTED"))
}

/// `pm_mpy_errno_ECONNRESET` — return the `ECONNRESET` export from `errno`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_errno_ECONNRESET() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(errno_export("ECONNRESET"))
}

/// `pm_mpy_errno_ENOBUFS` — return the `ENOBUFS` export from `errno`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_errno_ENOBUFS() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(errno_export("ENOBUFS"))
}

/// `pm_mpy_errno_ENOTCONN` — return the `ENOTCONN` export from `errno`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_errno_ENOTCONN() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(errno_export("ENOTCONN"))
}

/// `pm_mpy_errno_ETIMEDOUT` — return the `ETIMEDOUT` export from `errno`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_errno_ETIMEDOUT() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(errno_export("ETIMEDOUT"))
}

/// `pm_mpy_errno_ECONNREFUSED` — return the `ECONNREFUSED` export from `errno`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_errno_ECONNREFUSED() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(errno_export("ECONNREFUSED"))
}

/// `pm_mpy_errno_EHOSTUNREACH` — return the `EHOSTUNREACH` export from `errno`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_errno_EHOSTUNREACH() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(errno_export("EHOSTUNREACH"))
}

/// `pm_mpy_errno_EALREADY` — return the `EALREADY` export from `errno`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_errno_EALREADY() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(errno_export("EALREADY"))
}

/// `pm_mpy_errno_EINPROGRESS` — return the `EINPROGRESS` export from `errno`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_errno_EINPROGRESS() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(errno_export("EINPROGRESS"))
}
