//! rewrite of extmod/vfs_lfsx_file.c (LFS2 file objects)
// symmetry: done

use py_rs::gc::{self, ALLOC_FLAG_HAS_FINALISER};
use py_rs::malloc;
use py_rs::map::{self, MapElem};
use py_rs::mpprint::{self, Print, PrintKind, VaArg};
use py_rs::obj::{
    self, Obj, ObjBase, ObjType, TYPE_FLAG_BINDS_SELF, TYPE_FLAG_BUILTIN_FUN,
    TYPE_FLAG_ITER_IS_STREAM,
};
use py_rs::objdict::{self, ObjDict};
use py_rs::objstr;
use py_rs::qstr;
use py_rs::raise::{self, MpRaise};
use py_rs::stream::{
    self, StreamP, StreamSeek, SEEK_CUR, SEEK_END, SEEK_SET, STREAM_CLOSE, STREAM_ERROR,
    STREAM_FLUSH, STREAM_SEEK,
};

use shared_rs::timeutils::timeutils;

use crate::vfs_lfs::ObjVfsLfs2;
use crate::vfs_lfs_diskio::{self, LfsMount};

#[repr(C)]
pub struct ObjVfsLfs2File {
    pub base: py_rs::obj::ObjBase,
    pub mount: *mut LfsMount,
    path: *mut String,
    offset: u64,
    writable: bool,
    append: bool,
    closed: bool,
    mtime: [u8; 8],
    buffer_len: usize,
}

fn file_ptr(o: Obj) -> *mut ObjVfsLfs2File {
    obj::as_ptr(o) as *mut ObjVfsLfs2File
}

fn path_str(f: &ObjVfsLfs2File) -> &str {
    unsafe { &*f.path }
}

fn file_buffer_ptr(f: *mut ObjVfsLfs2File) -> *mut u8 {
    unsafe { (f as *mut u8).add(core::mem::size_of::<ObjVfsLfs2File>()) }
}

fn alloc_file_obj(
    type_out: &'static py_rs::obj::ObjType,
    cache_size: usize,
) -> *mut ObjVfsLfs2File {
    let size = core::mem::size_of::<ObjVfsLfs2File>() + cache_size;
    let ptr = gc::gc_alloc(size, ALLOC_FLAG_HAS_FINALISER).expect("VfsLfs2 file");
    unsafe {
        (*(ptr as *mut py_rs::obj::ObjBase)).type_ = type_out;
        let f = ptr as *mut ObjVfsLfs2File;
        (*f).buffer_len = cache_size;
        f
    }
}

fn free_file_path(f: &mut ObjVfsLfs2File) {
    if !f.path.is_null() {
        unsafe {
            drop(Box::from_raw(f.path));
            f.path = core::ptr::null_mut();
        }
    }
}

fn file_del(self_in: Obj) -> Obj {
    let self_ = unsafe { &mut *file_ptr(self_in) };
    if !self_.closed {
        self_.closed = true;
    }
    free_file_path(self_);
    obj::CONST_NONE
}

fn check_open(f: &ObjVfsLfs2File) {
    if f.closed {
        raise::raise(MpRaise::ValueError("I/O operation on closed file"));
    }
}

fn with_open_file<R>(
    mount: &mut LfsMount,
    path: &str,
    mode: &str,
    offset: u64,
    f: impl FnOnce(
        &mut littlefs_rust::FileHandle<'_, crate::vfs_lfs_diskio::LfsBlockDevice>,
    ) -> Result<R, i32>,
) -> Result<R, i32> {
    let options = LfsMount::file_options_from_mode(mode)?;
    let lfs_path = LfsMount::lfs_path(path);
    let fs = mount.fs_mut()?;
    let mut file = fs
        .open_file(&lfs_path, options)
        .map_err(vfs_lfs_diskio::map_lfs_err)?;
    if offset > 0 && !mode.contains('a') {
        file.seek(offset as usize)
            .map_err(vfs_lfs_diskio::map_lfs_err)?;
    }
    let result = f(&mut file);
    let _ = file.sync();
    let _ = file.close();
    result
}

fn file_print(print: &Print, self_in: Obj, _kind: PrintKind) {
    let self_ = unsafe { &*file_ptr(self_in) };
    let tag = if core::ptr::eq(self_.base.type_, type_fileio()) {
        "FileIO"
    } else {
        "TextIOWrapper"
    };
    mpprint::printf(
        print,
        "<io.{} {:x}>",
        [VaArg::Str(tag), VaArg::USize(self_in.0 as usize)],
    );
}

/// `mp_vfs_lfs2_file_open`
pub fn open(vfs_in: Obj, path_in: Obj, mode_in: Obj) -> Obj {
    let mount = unsafe { &mut *(*(obj::as_ptr(vfs_in) as *const ObjVfsLfs2)).mount };
    let path = mount.resolve_path(&objstr::str_get_str(path_in));
    let mode = objstr::str_get_str(mode_in);

    let mut writable = false;
    let mut append = false;
    let mut type_out = type_textio();
    for b in mode.bytes() {
        match b {
            b'r' => {}
            b'w' | b'x' => writable = true,
            b'a' => {
                writable = true;
                append = true;
            }
            b'+' => writable = true,
            b'b' => type_out = type_fileio(),
            b't' => type_out = type_textio(),
            _ => {}
        }
    }

    if writable && mount.blockdev.writeblocks[0] == obj::OBJ_NULL {
        raise::raise(MpRaise::OSError(py_rs::mperrno::EROFS));
    }

    with_open_file(mount, &path, &mode, 0, |_| Ok(()))
        .unwrap_or_else(|e| raise::raise(MpRaise::OSError(e)));

    if mount.enable_mtime {
        let mtime = timeutils::lfs_mtime_bytes_from_now();
        mount
            .write_mtime(&path, &mtime)
            .unwrap_or_else(|e| raise::raise(MpRaise::OSError(e)));
    }

    let cache_size = mount.cache_size();
    let o = alloc_file_obj(type_out, cache_size);
    unsafe {
        (*o).mount = mount as *mut LfsMount;
        (*o).path = Box::into_raw(Box::new(path));
        (*o).offset = 0;
        (*o).writable = writable;
        (*o).append = append;
        (*o).closed = false;
        (*o).mtime = [0; 8];
        core::ptr::write_bytes(file_buffer_ptr(o), 0, cache_size);
        obj::from_ptr(o as *const ObjVfsLfs2File as *const ())
    }
}

fn file_read(self_in: Obj, buf: *mut u8, size: usize, errcode: *mut i32) -> usize {
    let self_ = unsafe { &mut *file_ptr(self_in) };
    check_open(self_);
    unsafe {
        *errcode = 0;
    }
    let slice = unsafe { std::slice::from_raw_parts_mut(buf, size) };
    let path = path_str(self_).to_string();
    let mode = "rb";
    let mount = unsafe { &mut *self_.mount };
    match with_open_file(mount, &path, mode, self_.offset, |file| {
        let n = file.read(slice).map_err(vfs_lfs_diskio::map_lfs_err)?;
        Ok(n)
    }) {
        Ok(n) => {
            self_.offset += n as u64;
            n
        }
        Err(e) => {
            unsafe {
                *errcode = e;
            }
            STREAM_ERROR
        }
    }
}

fn file_write(self_in: Obj, buf: *const u8, size: usize, errcode: *mut i32) -> usize {
    let self_ = unsafe { &mut *file_ptr(self_in) };
    check_open(self_);
    if !self_.writable {
        unsafe {
            *errcode = py_rs::mperrno::EBADF;
        }
        return STREAM_ERROR;
    }
    unsafe {
        *errcode = 0;
    }
    let slice = unsafe { std::slice::from_raw_parts(buf, size) };
    let path = path_str(self_).to_string();
    let mode = if self_.append { "ab" } else { "r+b" };
    let offset = if self_.append { 0 } else { self_.offset };
    let mount = unsafe { &mut *self_.mount };
    if mount.enable_mtime {
        self_.mtime = timeutils::lfs_mtime_bytes_from_now();
        mount
            .write_mtime(&path, &self_.mtime)
            .unwrap_or_else(|e| raise::raise(MpRaise::OSError(e)));
    }
    match with_open_file(mount, &path, mode, offset, |file| {
        file.write_all(slice).map_err(vfs_lfs_diskio::map_lfs_err)?;
        Ok(slice.len())
    }) {
        Ok(n) => {
            self_.offset += n as u64;
            self_.append = false;
            n
        }
        Err(e) => {
            unsafe {
                *errcode = e;
            }
            STREAM_ERROR
        }
    }
}

fn file_ioctl(self_in: Obj, request: u32, arg: usize, errcode: *mut i32) -> usize {
    let self_ = unsafe { &mut *file_ptr(self_in) };
    match request {
        STREAM_SEEK => {
            check_open(self_);
            let s = unsafe { &mut *(arg as *mut StreamSeek) };
            self_.offset = match s.whence {
                SEEK_SET => s.offset as u64,
                SEEK_CUR => (self_.offset as i64 + s.offset) as u64,
                SEEK_END => {
                    let path = path_str(self_).to_string();
                    let mount = unsafe { &*self_.mount };
                    match mount.stat_path(&path) {
                        Ok(st) => (st.size as i64 + s.offset) as u64,
                        Err(e) => {
                            unsafe {
                                *errcode = e;
                            }
                            return STREAM_ERROR;
                        }
                    }
                }
                _ => {
                    unsafe {
                        *errcode = py_rs::mperrno::EINVAL;
                    }
                    return STREAM_ERROR;
                }
            };
            s.offset = self_.offset as i64;
            0
        }
        STREAM_FLUSH => {
            check_open(self_);
            if !self_.writable {
                return 0;
            }
            let path = path_str(self_).to_string();
            let mode = "r+b";
            let mount = unsafe { &mut *self_.mount };
            match with_open_file(mount, &path, mode, self_.offset, |file| {
                file.sync().map_err(vfs_lfs_diskio::map_lfs_err)
            }) {
                Ok(()) => 0,
                Err(e) => {
                    unsafe {
                        *errcode = e;
                    }
                    STREAM_ERROR
                }
            }
        }
        STREAM_CLOSE => {
            self_.closed = true;
            free_file_path(self_);
            0
        }
        _ => {
            unsafe {
                *errcode = py_rs::mperrno::EINVAL;
            }
            STREAM_ERROR
        }
    }
}

type BuiltinFn1 = fn(Obj) -> Obj;

#[repr(C)]
struct ObjFunBuiltin1 {
    base: ObjBase,
    fun: BuiltinFn1,
}

static mut F1: [*const (); 1] = [call1 as *const ()];
static TF1: ObjType = ObjType {
    base: ObjBase {
        type_: core::ptr::null(),
    },
    flags: TYPE_FLAG_BINDS_SELF | TYPE_FLAG_BUILTIN_FUN,
    name: 0,
    slot_index_make_new: 0,
    slot_index_print: 0,
    slot_index_call: 1,
    slot_index_unary_op: 0,
    slot_index_binary_op: 0,
    slot_index_attr: 0,
    slot_index_subscr: 0,
    slot_index_iter: 0,
    slot_index_buffer: 0,
    slot_index_protocol: 0,
    slot_index_parent: 0,
    slot_index_locals_dict: 0,
    slots: unsafe { F1.as_ptr() },
};

fn call1(s: Obj, n: usize, k: usize, a: &[Obj]) -> Obj {
    py_rs::argcheck::check_num(n, k, 1, 1, false);
    unsafe { ((*(obj::as_ptr(s) as *const ObjFunBuiltin1)).fun)(a[0]) }
}

fn mk1(f: BuiltinFn1) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltin1>().expect("vfs_lfs file fn1");
    unsafe {
        (*o).base.type_ = &TF1;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltin1 as *const ())
    }
}

static FILEIO_STREAM_P: StreamP = StreamP {
    read: Some(file_read),
    write: Some(file_write),
    ioctl: Some(file_ioctl),
    is_text: false,
};

static TEXTIO_STREAM_P: StreamP = StreamP {
    read: Some(file_read),
    write: Some(file_write),
    ioctl: Some(file_ioctl),
    is_text: true,
};

static mut FILEIO_SLOTS: [*const (); 3] = [core::ptr::null(); 3];
static mut TEXTIO_SLOTS: [*const (); 3] = [core::ptr::null(); 3];
static mut TYPE_FILEIO: ObjType = ObjType {
    base: ObjBase {
        type_: core::ptr::null(),
    },
    flags: TYPE_FLAG_ITER_IS_STREAM,
    name: 0,
    slot_index_make_new: 0,
    slot_index_print: 1,
    slot_index_call: 0,
    slot_index_unary_op: 0,
    slot_index_binary_op: 0,
    slot_index_attr: 0,
    slot_index_subscr: 0,
    slot_index_iter: 0,
    slot_index_buffer: 0,
    slot_index_protocol: 2,
    slot_index_parent: 0,
    slot_index_locals_dict: 3,
    slots: unsafe { FILEIO_SLOTS.as_ptr() },
};
static mut TYPE_TEXTIO: ObjType = ObjType {
    base: ObjBase {
        type_: core::ptr::null(),
    },
    flags: TYPE_FLAG_ITER_IS_STREAM,
    name: 0,
    slot_index_make_new: 0,
    slot_index_print: 1,
    slot_index_call: 0,
    slot_index_unary_op: 0,
    slot_index_binary_op: 0,
    slot_index_attr: 0,
    slot_index_subscr: 0,
    slot_index_iter: 0,
    slot_index_buffer: 0,
    slot_index_protocol: 2,
    slot_index_parent: 0,
    slot_index_locals_dict: 3,
    slots: unsafe { TEXTIO_SLOTS.as_ptr() },
};

fn locals_dict() -> *const () {
    static INIT: std::sync::OnceLock<()> = std::sync::OnceLock::new();
    static mut DICT: *const () = core::ptr::null();
    INIT.get_or_init(|| {
        let table = vec![
            MapElem {
                key: obj::new_qstr(qstr::from_str("read")),
                value: stream::stream_read_obj(),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("readinto")),
                value: stream::stream_readinto_obj(),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("readline")),
                value: stream::stream_unbuffered_readline_obj(),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("readlines")),
                value: stream::stream_unbuffered_readlines_obj(),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("write")),
                value: stream::stream_write_obj(),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("seek")),
                value: stream::stream_seek_obj(),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("tell")),
                value: stream::stream_tell_obj(),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("flush")),
                value: stream::stream_flush_obj(),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("close")),
                value: stream::stream_close_obj(),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("__del__")),
                value: mk1(file_del),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("__enter__")),
                value: mk1(|o| o),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("__exit__")),
                value: stream::stream___exit___obj(),
            },
        ];
        let ptr = obj::malloc_helper(core::mem::size_of::<ObjDict>(), objdict::type_dict())
            as *mut ObjDict;
        unsafe {
            map::init_fixed_table(&mut (*ptr).map, table);
            DICT = ptr as *const ();
        }
    });
    unsafe { DICT }
}

fn init_types() {
    static INIT: std::sync::OnceLock<()> = std::sync::OnceLock::new();
    INIT.get_or_init(|| {
        let dict = locals_dict();
        unsafe {
            FILEIO_SLOTS[0] = file_print as *const ();
            FILEIO_SLOTS[1] = &FILEIO_STREAM_P as *const StreamP as *const ();
            FILEIO_SLOTS[2] = dict;
            TYPE_FILEIO.name = qstr::from_str("FileIO");

            TEXTIO_SLOTS[0] = file_print as *const ();
            TEXTIO_SLOTS[1] = &TEXTIO_STREAM_P as *const StreamP as *const ();
            TEXTIO_SLOTS[2] = dict;
            TYPE_TEXTIO.name = qstr::from_str("TextIOWrapper");
        }
    });
}

pub fn type_fileio() -> &'static ObjType {
    init_types();
    unsafe { &TYPE_FILEIO }
}

pub fn type_textio() -> &'static ObjType {
    init_types();
    unsafe { &TYPE_TEXTIO }
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
    use py_rs::stream::{self, SEEK_SET};

    use crate::vfs_blockdev::{
        VfsBlockdev, BLOCKDEV_FLAG_HAVE_IOCTL, BLOCKDEV_IOCTL_BLOCK_COUNT,
        BLOCKDEV_IOCTL_BLOCK_ERASE, BLOCKDEV_IOCTL_BLOCK_SIZE, BLOCKDEV_IOCTL_INIT,
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
        init_types();
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
        let dst = bufinfo.as_bytes_mut();
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
        let src = bufinfo.as_bytes();
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
        mount.fs = Some(Filesystem::mount_device_mut_with_options(device, opts).expect("mount"));
        mount
    }

    #[test]
    fn statvfs_reports_geometry_and_free_blocks() {
        let _guard = py_rs::vm_test::lock();
        setup();
        let mount = test_mount();
        let sv = mount.statvfs().expect("statvfs");
        assert_eq!(sv[0], BLOCK_SIZE as isize);
        assert_eq!(sv[1], BLOCK_SIZE as isize);
        assert_eq!(sv[2], BLOCK_COUNT as isize);
        assert!(sv[3] > 0);
        assert_eq!(sv[3], sv[4]);
        assert!(sv[9] > 0);
    }

    #[test]
    fn ram_mount_mkfs_open_read_write() {
        let _guard = py_rs::vm_test::lock();
        setup();
        let mount = test_mount();
        assert!(mount.is_mounted());
        assert_eq!(mount.cache_size(), 128);

        let mount_ptr = Box::into_raw(mount);
        let vfs_obj = unsafe {
            let o = py_rs::malloc::new_obj::<ObjVfsLfs2>().expect("vfs");
            (*o).base.type_ = type_vfs_lfs2();
            (*o).mount = mount_ptr;
            obj::from_ptr(o as *const ObjVfsLfs2 as *const ())
        };

        let path = objstr::new_str(b"hello.txt");
        let mode = objstr::new_str(b"wb");
        let file = open(vfs_obj, path, mode);
        let file_ptr = obj::as_ptr(file) as *const ObjVfsLfs2File;
        unsafe {
            assert_eq!((*file_ptr).buffer_len, 128);
        }

        let payload = b"hello!";
        let mut err = 0;
        assert_eq!(
            file_write(file, payload.as_ptr(), payload.len(), &mut err),
            payload.len()
        );
        assert_eq!(err, 0);

        assert_eq!(stream::stream_seek(file, 0, SEEK_SET, &mut err), 0);
        let mut buf = [0u8; 6];
        assert_eq!(file_read(file, buf.as_mut_ptr(), buf.len(), &mut err), 6);
        assert_eq!(&buf, b"hello!");

        let mount = unsafe { &*(*vfs_ptr(vfs_obj)).mount };
        let mtime = mount.read_mtime("hello.txt");
        assert!(mtime.is_some());
    }

    fn vfs_ptr(o: Obj) -> *mut ObjVfsLfs2 {
        obj::as_ptr(o) as *mut ObjVfsLfs2
    }
}
