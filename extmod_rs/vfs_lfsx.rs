//! rewrite of extmod/vfs_lfsx.c (LFS2 shared VFS ops)
// symmetry: done

use littlefs_rust::DirHandle;
use py_rs::malloc;
use py_rs::obj::{self, Obj, ObjBase};
use py_rs::objpolyiter;
use py_rs::objstr;
use py_rs::objtuple;
use py_rs::raise::{self, MpRaise};

use shared_rs::timeutils::timeutils;

use crate::vfs_lfs::ObjVfsLfs2;
use crate::vfs_lfs_diskio::{self, LfsMount};

pub const MP_S_IFDIR: i32 = 0x4000;
pub const MP_S_IFREG: i32 = 0x8000;

fn vfs_ptr(o: Obj) -> *mut ObjVfsLfs2 {
    obj::as_ptr(o) as *mut ObjVfsLfs2
}

fn mount_mut(o: Obj) -> &'static mut LfsMount {
    unsafe { &mut *(*vfs_ptr(o)).mount }
}

fn require_writable(o: Obj) {
    let mount = unsafe { &*(*vfs_ptr(o)).mount };
    if mount.blockdev.writeblocks[0] == obj::OBJ_NULL {
        raise::raise(MpRaise::OSError(py_rs::mperrno::EROFS));
    }
}

#[repr(C)]
struct IlistdirIter {
    base: ObjBase,
    iternext: py_rs::obj::IterNextFn,
    finaliser: py_rs::obj::IterNextFn,
    is_str: bool,
    dir: Option<DirHandle<'static, 'static>>,
}

fn ilistdir_iternext(self_in: Obj) -> Obj {
    let self_ = unsafe { &mut *(obj::as_ptr(self_in) as *mut IlistdirIter) };
    let Some(dir) = self_.dir.as_mut() else {
        return obj::OBJ_STOP_ITERATION;
    };
    loop {
        let entry = match dir.read() {
            Ok(Some(e)) => e,
            Ok(None) => {
                self_.dir = None;
                return obj::OBJ_STOP_ITERATION;
            }
            Err(e) => raise::raise(MpRaise::OSError(vfs_lfs_diskio::map_lfs_err(e))),
        };
        if entry.name == "." || entry.name == ".." {
            continue;
        }
        let name_obj = if self_.is_str {
            objstr::new_str(entry.name.as_bytes())
        } else {
            objstr::new_bytes(entry.name.as_bytes())
        };
        let mode = if entry.ty == littlefs_rust::FileType::Dir {
            MP_S_IFDIR
        } else {
            MP_S_IFREG
        };
        return objtuple::new_tuple(
            4,
            Some(&[
                name_obj,
                obj::new_small_int(mode as isize),
                obj::new_small_int(0),
                obj::new_small_int(entry.size as isize),
            ]),
        );
    }
}

fn ilistdir_it_del(self_in: Obj) -> Obj {
    let self_ = unsafe { &mut *(obj::as_ptr(self_in) as *mut IlistdirIter) };
    self_.dir = None;
    obj::CONST_NONE
}

pub fn ilistdir(self_in: Obj, n: usize, args: &[Obj]) -> Obj {
    let path_in = if n >= 1 { args[0] } else { objstr::new_str(b"") };
    let mount = mount_mut(self_in);
    let path = objstr::str_get_str(path_in);
    let lfs_path = LfsMount::lfs_path(&mount.resolve_path(&path));
    let fs = mount
        .fs_mut()
        .unwrap_or_else(|e| raise::raise(MpRaise::OSError(e)));
    let dir = fs
        .open_dir(&lfs_path)
        .unwrap_or_else(|e| raise::raise(MpRaise::OSError(vfs_lfs_diskio::map_lfs_err(e))));
    // SAFETY: iterator outlives only while the owning VfsLfs2 (and its mount) remain
    // reachable, matching upstream's `ilistdir_it_t` holding `MP_OBJ_VFS_LFSx *vfs`.
    let dir = unsafe { core::mem::transmute::<DirHandle<'_, 'static>, DirHandle<'static, 'static>>(dir) };
    let o = malloc::new_obj::<IlistdirIter>().expect("VfsLfs2 ilistdir");
    unsafe {
        (*o).base.type_ = objpolyiter::type_polymorph_iter_with_finaliser();
        (*o).iternext = ilistdir_iternext;
        (*o).finaliser = ilistdir_it_del;
        (*o).is_str = obj::is_str(path_in);
        (*o).dir = Some(dir);
        obj::from_ptr(o as *const IlistdirIter as *const ())
    }
}

pub fn mkdir(self_in: Obj, path_in: Obj) -> Obj {
    require_writable(self_in);
    let mount = mount_mut(self_in);
    let path = objstr::str_get_str(path_in);
    mount
        .mkdir(&path)
        .unwrap_or_else(|e| raise::raise(MpRaise::OSError(e)));
    obj::CONST_NONE
}

pub fn remove(self_in: Obj, path_in: Obj) -> Obj {
    require_writable(self_in);
    let mount = mount_mut(self_in);
    let path = objstr::str_get_str(path_in);
    mount
        .remove_path(&path)
        .unwrap_or_else(|e| raise::raise(MpRaise::OSError(e)));
    obj::CONST_NONE
}

pub fn rmdir(self_in: Obj, path_in: Obj) -> Obj {
    remove(self_in, path_in)
}

pub fn rename(self_in: Obj, old_path_in: Obj, new_path_in: Obj) -> Obj {
    require_writable(self_in);
    let mount = mount_mut(self_in);
    let old_path = objstr::str_get_str(old_path_in);
    let new_path = objstr::str_get_str(new_path_in);
    mount
        .rename_path(&old_path, &new_path)
        .unwrap_or_else(|e| raise::raise(MpRaise::OSError(e)));
    obj::CONST_NONE
}

pub fn chdir(self_in: Obj, path_in: Obj) -> Obj {
    let mount = mount_mut(self_in);
    let path = objstr::str_get_str(path_in);
    mount
        .chdir(&path)
        .unwrap_or_else(|e| raise::raise(MpRaise::OSError(e)));
    obj::CONST_NONE
}

pub fn getcwd(self_in: Obj) -> Obj {
    let mount = unsafe { &*(*vfs_ptr(self_in)).mount };
    objstr::new_str(mount.getcwd().as_bytes())
}

pub fn stat(self_in: Obj, path_in: Obj) -> Obj {
    let mount = unsafe { &*(*vfs_ptr(self_in)).mount };
    let path = objstr::str_get_str(path_in);
    let st = mount
        .stat_path(&path)
        .unwrap_or_else(|e| raise::raise(MpRaise::OSError(e)));
    let mode = if st.is_dir { MP_S_IFDIR } else { MP_S_IFREG };
    let ts_obj = mount
        .read_mtime(&path)
        .map(timeutils::obj_from_timestamp)
        .unwrap_or_else(|| obj::new_small_int(0));
    objtuple::new_tuple(
        10,
        Some(&[
            obj::new_small_int(mode as isize),
            obj::new_small_int(0),
            obj::new_small_int(0),
            obj::new_small_int(0),
            obj::new_small_int(0),
            obj::new_small_int(0),
            obj::new_small_int(st.size as isize),
            ts_obj,
            ts_obj,
            ts_obj,
        ]),
    )
}

pub fn statvfs(self_in: Obj, _path_in: Obj) -> Obj {
    let mount = unsafe { &*(*vfs_ptr(self_in)).mount };
    let vals = mount
        .statvfs()
        .unwrap_or_else(|e| raise::raise(MpRaise::OSError(e)));
    let items: Vec<Obj> = vals.iter().map(|v| obj::new_small_int(*v)).collect();
    objtuple::new_tuple(10, Some(&items))
}

pub fn enabled() -> bool {
    vfs_lfs_diskio::enabled()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    use py_rs::argcheck;
    use py_rs::gc;
    use py_rs::mpstate;
    use py_rs::obj::BufferInfo;
    use py_rs::objfun::{self, BuiltinFnVar};
    use py_rs::objpolyiter;
    use py_rs::objtuple;
    use py_rs::qstr;

    use crate::vfs_blockdev::{
        BLOCKDEV_IOCTL_BLOCK_COUNT, BLOCKDEV_IOCTL_BLOCK_ERASE, BLOCKDEV_IOCTL_BLOCK_SIZE,
        BLOCKDEV_IOCTL_INIT, BLOCKDEV_FLAG_HAVE_IOCTL, VfsBlockdev,
    };
    use crate::vfs_lfs::{type_vfs_lfs2, ObjVfsLfs2};
    use crate::vfs_lfs_diskio::{LfsBlockDevice, LfsMount};

    static TEST_RAM: Mutex<Vec<u8>> = Mutex::new(Vec::new());
    const BLOCK_SIZE: usize = 512;
    const BLOCK_COUNT: usize = 32;

    fn setup() {
        let _ = gc::init();
        qstr::init();
        mpstate::init();
        type_vfs_lfs2();
    }

    fn ram_readblocks(n_args: usize, args: &[Obj]) -> Obj {
        let block = obj::get_int_truncated(args[0]) as usize;
        let mut bufinfo = BufferInfo {
            buf: core::ptr::null_mut(),
            len: 0,
            typecode: 0,
        };
        obj::get_buffer_raise(args[1], &mut bufinfo, obj::BUFFER_WRITE);
        let off = if n_args >= 3 {
            obj::get_int_truncated(args[2]) as usize
        } else {
            0
        };
        let base = block * BLOCK_SIZE + off;
        let ram = TEST_RAM.lock().expect("ram lock");
        let dst = unsafe { std::slice::from_raw_parts_mut(bufinfo.buf as *mut u8, bufinfo.len) };
        dst.copy_from_slice(&ram[base..base + dst.len()]);
        obj::CONST_NONE
    }

    fn ram_writeblocks(n_args: usize, args: &[Obj]) -> Obj {
        let block = obj::get_int_truncated(args[0]) as usize;
        let mut bufinfo = BufferInfo {
            buf: core::ptr::null_mut(),
            len: 0,
            typecode: 0,
        };
        obj::get_buffer_raise(args[1], &mut bufinfo, obj::BUFFER_READ);
        let off = if n_args >= 3 {
            obj::get_int_truncated(args[2]) as usize
        } else {
            0
        };
        let base = block * BLOCK_SIZE + off;
        let mut ram = TEST_RAM.lock().expect("ram lock");
        let src = unsafe { std::slice::from_raw_parts(bufinfo.buf as *const u8, bufinfo.len) };
        ram[base..base + src.len()].copy_from_slice(src);
        obj::CONST_NONE
    }

    fn ram_ioctl(n_args: usize, args: &[Obj]) -> Obj {
        let op = obj::get_int_truncated(args[0]) as usize;
        let arg = if n_args >= 2 {
            obj::get_int_truncated(args[1]) as usize
        } else {
            0
        };
        match op {
            BLOCKDEV_IOCTL_BLOCK_COUNT => obj::new_small_int(BLOCK_COUNT as isize),
            BLOCKDEV_IOCTL_BLOCK_SIZE => obj::new_small_int(BLOCK_SIZE as isize),
            BLOCKDEV_IOCTL_INIT => obj::CONST_NONE,
            BLOCKDEV_IOCTL_BLOCK_ERASE => {
                let mut ram = TEST_RAM.lock().expect("ram lock");
                let start = arg * BLOCK_SIZE;
                ram[start..start + BLOCK_SIZE].fill(0xff);
                obj::CONST_NONE
            }
            _ => obj::CONST_NONE,
        }
    }

    fn mk_var(min: usize, max: usize, fun: BuiltinFnVar) -> Obj {
        let o = obj::malloc_helper(
            core::mem::size_of::<objfun::ObjFunBuiltinVar>(),
            objfun::type_fun_builtin_var(),
        ) as *mut objfun::ObjFunBuiltinVar;
        unsafe {
            (*o).base.type_ = objfun::type_fun_builtin_var();
            (*o).sig = argcheck::make_sig(min, max, false);
            (*o).fun.var = fun;
            obj::from_ptr(o as *const objfun::ObjFunBuiltinVar as *const ())
        }
    }

    fn test_mount() -> Box<LfsMount> {
        use littlefs_rust::Filesystem;

        *TEST_RAM.lock().expect("ram lock") = vec![0xff; BLOCK_COUNT * BLOCK_SIZE];
        let mut mount = Box::new(LfsMount {
            blockdev: VfsBlockdev::default(),
            no_filesystem: false,
            cwd: "/".to_string(),
            read_size: 32,
            prog_size: 32,
            lookahead: 32,
            enable_mtime: true,
            fs: None,
        });
        mount.blockdev.block_size = BLOCK_SIZE;
        mount.blockdev.flags |= BLOCKDEV_FLAG_HAVE_IOCTL;
        mount.blockdev.readblocks[0] = mk_var(2, 3, ram_readblocks);
        mount.blockdev.writeblocks[0] = mk_var(2, 3, ram_writeblocks);
        mount.blockdev.ioctl[0] = mk_var(1, 2, ram_ioctl);

        let bdev_ptr = &mut mount.blockdev as *mut VfsBlockdev;
        let mut device = LfsBlockDevice::new(bdev_ptr, BLOCK_COUNT, BLOCK_SIZE);
        let opts = mount.fs_options();
        Filesystem::format_device_with_options(&mut device, opts).expect("format");
        mount.fs = Some(
            Filesystem::mount_device_mut_with_options(device, opts).expect("mount"),
        );
        mount
    }

    fn vfs_for_mount(mount: Box<LfsMount>) -> (Obj, *mut LfsMount) {
        let mount_ptr = Box::into_raw(mount);
        let vfs_obj = unsafe {
            let o = malloc::new_obj::<ObjVfsLfs2>().expect("vfs");
            (*o).base.type_ = type_vfs_lfs2();
            (*o).mount = mount_ptr;
            obj::from_ptr(o as *const ObjVfsLfs2 as *const ())
        };
        (vfs_obj, mount_ptr)
    }

    #[test]
    fn streaming_ilistdir_skips_dot_entries() {
        let _guard = py_rs::vm_test::lock();
        setup();
        let mut mount = test_mount();
        mount.mkdir("sub").expect("mkdir");
        mount
            .fs_mut()
            .expect("fs")
            .create_file("/sub/a.txt", b"a")
            .expect("create");
        let (vfs_obj, mount_ptr) = vfs_for_mount(mount);

        let iter = ilistdir(vfs_obj, 1, &[objstr::new_str(b"sub")]);
        let mut names = Vec::new();
        loop {
            let item = objpolyiter::polymorph_it_iternext(iter);
            if item == obj::OBJ_STOP_ITERATION {
                break;
            }
            let (_, items) = objtuple::tuple_get(item);
            let name = objstr::str_get_str(items[0]);
            names.push(name.to_string());
        }
        assert_eq!(names, vec!["a.txt".to_string()]);

        unsafe {
            drop(Box::from_raw(mount_ptr));
        }
    }

    #[test]
    fn stat_reports_mtime_when_enabled() {
        let _guard = py_rs::vm_test::lock();
        setup();
        let mut mount = test_mount();
        mount
            .fs_mut()
            .expect("fs")
            .create_file("/mtime.txt", b"x")
            .expect("create");
        mount.touch_mtime("mtime.txt").expect("touch");
        let (vfs_obj, mount_ptr) = vfs_for_mount(mount);

        let st = stat(vfs_obj, objstr::new_str(b"mtime.txt"));
        let (_, items) = objtuple::tuple_get(st);
        let mtime = obj::get_int_truncated(items[8]);
        assert!(mtime > 0);

        unsafe {
            drop(Box::from_raw(mount_ptr));
        }
    }
}
