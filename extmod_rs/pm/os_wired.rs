//! Wired `pm_mpy_os_*` accessors.
// symmetry: done

use super::os::os_export;
use py_rs::pm::mpy::pm_mpy_obj_t;

/// `pm_mpy_os_getenv` — return the `getenv` export from `os`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_os_getenv() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(os_export("getenv"))
}

/// `pm_mpy_os_putenv` — return the `putenv` export from `os`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_os_putenv() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(os_export("putenv"))
}

/// `pm_mpy_os_unsetenv` — return the `unsetenv` export from `os`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_os_unsetenv() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(os_export("unsetenv"))
}

/// `pm_mpy_os_sync` — return the `sync` export from `os`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_os_sync() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(os_export("sync"))
}

/// `pm_mpy_os_system` — return the `system` export from `os`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_os_system() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(os_export("system"))
}

/// `pm_mpy_os_uname` — return the `uname` export from `os`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_os_uname() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(os_export("uname"))
}

/// `pm_mpy_os_urandom` — return the `urandom` export from `os`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_os_urandom() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(os_export("urandom"))
}

/// `pm_mpy_os_sep` — return the `sep` export from `os`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_os_sep() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(os_export("sep"))
}

/// `pm_mpy_os_chdir` — return the `chdir` export from `os`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_os_chdir() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(os_export("chdir"))
}

/// `pm_mpy_os_getcwd` — return the `getcwd` export from `os`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_os_getcwd() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(os_export("getcwd"))
}

/// `pm_mpy_os_listdir` — return the `listdir` export from `os`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_os_listdir() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(os_export("listdir"))
}

/// `pm_mpy_os_mkdir` — return the `mkdir` export from `os`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_os_mkdir() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(os_export("mkdir"))
}

/// `pm_mpy_os_remove` — return the `remove` export from `os`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_os_remove() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(os_export("remove"))
}

/// `pm_mpy_os_rename` — return the `rename` export from `os`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_os_rename() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(os_export("rename"))
}

/// `pm_mpy_os_rmdir` — return the `rmdir` export from `os`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_os_rmdir() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(os_export("rmdir"))
}

/// `pm_mpy_os_unlink` — return the `unlink` export from `os`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_os_unlink() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(os_export("unlink"))
}

/// `pm_mpy_os_stat` — return the `stat` export from `os`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_os_stat() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(os_export("stat"))
}

/// `pm_mpy_os_statvfs` — return the `statvfs` export from `os`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_os_statvfs() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(os_export("statvfs"))
}

/// `pm_mpy_os_dupterm` — return the `dupterm` export from `os`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_os_dupterm() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(os_export("dupterm"))
}

/// `pm_mpy_os_dupterm_notify` — return the `dupterm_notify` export from `os`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_os_dupterm_notify() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(os_export("dupterm_notify"))
}

/// `pm_mpy_os_errno` — return the `errno` export from `os`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_os_errno() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(os_export("errno"))
}

/// `pm_mpy_os_ilistdir` — return the `ilistdir` export from `os`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_os_ilistdir() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(os_export("ilistdir"))
}

/// `pm_mpy_os_mount` — return the `mount` export from `os`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_os_mount() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(os_export("mount"))
}

/// `pm_mpy_os_umount` — return the `umount` export from `os`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_os_umount() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(os_export("umount"))
}

/// `pm_mpy_os_VfsFat` — return the `VfsFat` export from `os`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_os_VfsFat() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(os_export("VfsFat"))
}

/// `pm_mpy_os_VfsLfs1` — return the `VfsLfs1` export from `os`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_os_VfsLfs1() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(os_export("VfsLfs1"))
}

/// `pm_mpy_os_VfsLfs2` — return the `VfsLfs2` export from `os`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_os_VfsLfs2() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(os_export("VfsLfs2"))
}

/// `pm_mpy_os_VfsPosix` — return the `VfsPosix` export from `os`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_os_VfsPosix() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(os_export("VfsPosix"))
}
