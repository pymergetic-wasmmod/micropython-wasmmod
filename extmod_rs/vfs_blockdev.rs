//! rewrite of extmod/vfs_blockdev.c
// symmetry: done

use py_rs::mpconfig;
use py_rs::mperrno::{EINVAL, EROFS};
use py_rs::obj::{self, Obj};
use py_rs::objarray::{self, OBJ_ARRAY_TYPECODE_FLAG_RW};
use py_rs::qstr;
use py_rs::runtime;

pub const BLOCKDEV_FLAG_HAVE_IOCTL: u16 = 0x0004;
pub const BLOCKDEV_FLAG_FREE_OBJ: u16 = 0x0002;
pub const BLOCKDEV_FLAG_NO_FILESYSTEM: u16 = 0x0008;

pub const BLOCKDEV_IOCTL_INIT: usize = 1;
pub const BLOCKDEV_IOCTL_DEINIT: usize = 2;
pub const BLOCKDEV_IOCTL_SYNC: usize = 3;
pub const BLOCKDEV_IOCTL_BLOCK_COUNT: usize = 4;
pub const BLOCKDEV_IOCTL_BLOCK_SIZE: usize = 5;
pub const BLOCKDEV_IOCTL_BLOCK_ERASE: usize = 6;

/// VFS block-device protocol state (`mp_vfs_blockdev_t`).
#[repr(C)]
pub struct VfsBlockdev {
    pub flags: u16,
    pub block_size: usize,
    pub readblocks: [Obj; 5],
    pub writeblocks: [Obj; 5],
    pub ioctl: [Obj; 4],
    pub old_sync: [Obj; 2],
    pub old_count: [Obj; 2],
}

impl Default for VfsBlockdev {
    fn default() -> Self {
        Self {
            flags: 0,
            block_size: 0,
            readblocks: [obj::OBJ_NULL; 5],
            writeblocks: [obj::OBJ_NULL; 5],
            ioctl: [obj::OBJ_NULL; 4],
            old_sync: [obj::OBJ_NULL; 2],
            old_count: [obj::OBJ_NULL; 2],
        }
    }
}

pub fn enabled() -> bool {
    mpconfig::PY_VFS
}

fn load_pair(obj_in: Obj, attr: &str, dest: &mut [Obj; 2]) {
    runtime::load_method(obj_in, qstr::from_str(attr), dest);
}

fn load_pair_maybe(obj_in: Obj, attr: &str, dest: &mut [Obj; 2]) {
    runtime::load_method_maybe(obj_in, qstr::from_str(attr), dest);
}

/// `mp_vfs_blockdev_init`
pub fn blockdev_init(self_: &mut VfsBlockdev, bdev: Obj) {
    load_pair(bdev, "readblocks", &mut self_.readblocks[..2].try_into().unwrap());
    load_pair_maybe(bdev, "writeblocks", &mut self_.writeblocks[..2].try_into().unwrap());
    load_pair_maybe(bdev, "ioctl", &mut self_.ioctl[..2].try_into().unwrap());
    if self_.ioctl[0] != obj::OBJ_NULL {
        self_.flags |= BLOCKDEV_FLAG_HAVE_IOCTL;
    } else {
        load_pair_maybe(bdev, "sync", &mut self_.old_sync);
        load_pair(bdev, "count", &mut self_.old_count);
    }
}

fn blockdev_call_rw(
    args: &mut [Obj; 5],
    block_num: usize,
    block_off: usize,
    len: usize,
    buf: *mut u8,
    n_args: usize,
) -> i32 {
    let mv = objarray::new_memoryview(
        b'B' | OBJ_ARRAY_TYPECODE_FLAG_RW,
        len,
        buf,
    );
    args[2] = obj::new_small_int(block_num as isize);
    args[3] = mv;
    args[4] = obj::new_small_int(block_off as isize);
    let ret = runtime::call_method_n_kw(n_args, 0, &args[..2 + n_args]);
    if ret == obj::CONST_NONE {
        0
    } else {
        let i = obj::get_int_truncated(ret) as i32;
        if i > 0 {
            -EINVAL
        } else {
            i
        }
    }
}

/// `mp_vfs_blockdev_read`
pub fn blockdev_read(self_: &mut VfsBlockdev, block_num: usize, num_blocks: usize, buf: *mut u8) -> i32 {
    blockdev_call_rw(
        &mut self_.readblocks,
        block_num,
        0,
        num_blocks * self_.block_size,
        buf,
        2,
    )
}

/// `mp_vfs_blockdev_read_ext`
pub fn blockdev_read_ext(
    self_: &mut VfsBlockdev,
    block_num: usize,
    block_off: usize,
    len: usize,
    buf: *mut u8,
) -> i32 {
    blockdev_call_rw(&mut self_.readblocks, block_num, block_off, len, buf, 3)
}

/// `mp_vfs_blockdev_write`
pub fn blockdev_write(
    self_: &mut VfsBlockdev,
    block_num: usize,
    num_blocks: usize,
    buf: *const u8,
) -> i32 {
    if self_.writeblocks[0] == obj::OBJ_NULL {
        return -EROFS;
    }
    blockdev_call_rw(
        &mut self_.writeblocks,
        block_num,
        0,
        num_blocks * self_.block_size,
        buf as *mut u8,
        2,
    )
}

/// `mp_vfs_blockdev_write_ext`
pub fn blockdev_write_ext(
    self_: &mut VfsBlockdev,
    block_num: usize,
    block_off: usize,
    len: usize,
    buf: *const u8,
) -> i32 {
    if self_.writeblocks[0] == obj::OBJ_NULL {
        return -EROFS;
    }
    blockdev_call_rw(
        &mut self_.writeblocks,
        block_num,
        block_off,
        len,
        buf as *mut u8,
        3,
    )
}

/// `mp_vfs_blockdev_ioctl`
pub fn blockdev_ioctl(self_: &mut VfsBlockdev, cmd: usize, arg: usize) -> Obj {
    if self_.flags & BLOCKDEV_FLAG_HAVE_IOCTL != 0 {
        self_.ioctl[2] = obj::new_small_int(cmd as isize);
        self_.ioctl[3] = obj::new_small_int(arg as isize);
        runtime::call_method_n_kw(2, 0, &self_.ioctl)
    } else {
        match cmd {
            BLOCKDEV_IOCTL_SYNC => {
                if self_.old_sync[0] != obj::OBJ_NULL {
                    runtime::call_method_n_kw(0, 0, &self_.old_sync);
                }
            }
            BLOCKDEV_IOCTL_BLOCK_COUNT => {
                return runtime::call_method_n_kw(0, 0, &self_.old_count);
            }
            BLOCKDEV_IOCTL_BLOCK_SIZE | BLOCKDEV_IOCTL_INIT => {}
            _ => {}
        }
        obj::CONST_NONE
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn readonly_write_returns_erofs() {
        let mut bdev = VfsBlockdev::default();
        let buf = [0u8; 512];
        assert_eq!(blockdev_write(&mut bdev, 0, 1, buf.as_ptr()), -EROFS);
    }
}
