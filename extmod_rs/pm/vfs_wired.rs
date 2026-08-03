//! Wired `pm_mpy_vfs_*` accessors.
// symmetry: done

use super::vfs::vfs_export;
use py_rs::pm::mpy::pm_mpy_obj_t;

/// `pm_mpy_vfs_mount` — return the `mount` export from `vfs`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_vfs_mount() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(vfs_export("mount"))
}

/// `pm_mpy_vfs_umount` — return the `umount` export from `vfs`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_vfs_umount() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(vfs_export("umount"))
}

/// `pm_mpy_vfs_rom_ioctl` — return the `rom_ioctl` export from `vfs`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_vfs_rom_ioctl() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(vfs_export("rom_ioctl"))
}

/// `pm_mpy_vfs_VfsFat` — return the `VfsFat` export from `vfs`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_vfs_VfsFat() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(vfs_export("VfsFat"))
}

/// `pm_mpy_vfs_VfsLfs1` — return the `VfsLfs1` export from `vfs`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_vfs_VfsLfs1() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(vfs_export("VfsLfs1"))
}

/// `pm_mpy_vfs_VfsLfs2` — return the `VfsLfs2` export from `vfs`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_vfs_VfsLfs2() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(vfs_export("VfsLfs2"))
}

/// `pm_mpy_vfs_VfsRom` — return the `VfsRom` export from `vfs`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_vfs_VfsRom() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(vfs_export("VfsRom"))
}

/// `pm_mpy_vfs_VfsPosix` — return the `VfsPosix` export from `vfs`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_vfs_VfsPosix() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(vfs_export("VfsPosix"))
}
