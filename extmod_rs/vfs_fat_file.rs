//! rewrite of extmod/vfs_fat_file.c
// symmetry: done

use std::io::{Read, Seek, SeekFrom, Write};

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

use crate::vfs_fat::ObjVfsFat;
use crate::vfs_fat_diskio::{self, FatBlockStream, FatMount};

#[repr(C)]
pub struct ObjVfsFatFile {
    pub base: ObjBase,
    pub mount: *mut FatMount,
    path: *mut String,
    offset: u64,
    writable: bool,
    append: bool,
    closed: bool,
}

fn file_ptr(o: Obj) -> *mut ObjVfsFatFile {
    obj::as_ptr(o) as *mut ObjVfsFatFile
}

fn path_str(f: &ObjVfsFatFile) -> &str {
    unsafe { &*f.path }
}

fn fatfs_err(err: std::io::Error) -> i32 {
    vfs_fat_diskio::map_io_err(err)
}

fn with_open_file<R>(
    mount: &mut FatMount,
    path: &str,
    write: bool,
    append: bool,
    offset: u64,
    truncate: bool,
    f: impl FnOnce(&mut fatfs::File<'_, FatBlockStream>) -> Result<R, i32>,
) -> Result<R, i32> {
    let fs = mount.fs_mut()?;
    let root = fs.root_dir();
    let mut file = if write {
        if truncate {
            root.create_file(path).map_err(fatfs_err)?
        } else if append {
            root.open_file(path)
                .or_else(|_| root.create_file(path))
                .map_err(fatfs_err)?
        } else {
            root.open_file(path)
                .or_else(|_| root.create_file(path))
                .map_err(fatfs_err)?
        }
    } else {
        root.open_file(path).map_err(fatfs_err)?
    };
    if append {
        file.seek(SeekFrom::End(0)).map_err(fatfs_err)?;
    } else if offset > 0 || write {
        file.seek(SeekFrom::Start(offset)).map_err(fatfs_err)?;
    }
    f(&mut file)
}

fn check_open(f: &ObjVfsFatFile) {
    if f.closed {
        raise::raise(MpRaise::ValueError("I/O operation on closed file"));
    }
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

/// `fat_vfs_open`
pub fn open(vfs_in: Obj, path_in: Obj, mode_in: Obj) -> Obj {
    let mount = unsafe { &mut *(*(obj::as_ptr(vfs_in) as *const ObjVfsFat)).mount };
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

    let truncate = mode.contains('w') || mode.contains('x');
    with_open_file(mount, &path, writable, append, 0, truncate, |file| {
        file.flush().map_err(fatfs_err)?;
        Ok(())
    })
    .unwrap_or_else(|e| raise::raise(MpRaise::OSError(e)));

    let o = malloc::new_obj::<ObjVfsFatFile>().expect("VfsFat file");
    unsafe {
        (*o).base.type_ = type_out;
        (*o).mount = mount as *mut FatMount;
        (*o).path = Box::into_raw(Box::new(path));
        (*o).offset = 0;
        (*o).writable = writable;
        (*o).append = append;
        (*o).closed = false;
        obj::from_ptr(o as *const ObjVfsFatFile as *const ())
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
    let mount = unsafe { &mut *self_.mount };
    match with_open_file(mount, &path, false, false, self_.offset, false, |file| {
        let n = file.read(slice).map_err(fatfs_err)?;
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
    let mount = unsafe { &mut *self_.mount };
    match with_open_file(
        mount,
        &path,
        true,
        self_.append,
        if self_.append { 0 } else { self_.offset },
        false,
        |file| {
            file.write_all(slice).map_err(fatfs_err)?;
            file.flush().map_err(fatfs_err)?;
            Ok(slice.len())
        },
    ) {
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
                    let mount = unsafe { &mut *self_.mount };
                    match with_open_file(mount, &path, false, false, 0, false, |file| {
                        file.seek(SeekFrom::End(0)).map_err(fatfs_err)
                    }) {
                        Ok(end) => (end as i64 + s.offset) as u64,
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
            let mount = unsafe { &mut *self_.mount };
            match with_open_file(mount, &path, true, false, self_.offset, false, |file| {
                file.flush().map_err(fatfs_err)
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
    let o = malloc::new_obj::<ObjFunBuiltin1>().expect("vfs_fat file fn1");
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
                value: stream::stream_close_obj(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    use py_rs::argcheck;
    use py_rs::gc;
    use py_rs::mpstate;
    use py_rs::obj::BufferInfo;
    use py_rs::objfun::{self, BuiltinFnVar};
    use py_rs::runtime;

    use crate::vfs_blockdev::{
        BLOCKDEV_IOCTL_BLOCK_COUNT, BLOCKDEV_IOCTL_BLOCK_SIZE, BLOCKDEV_IOCTL_INIT,
    };

    static TEST_RAM: Mutex<Vec<u8>> = Mutex::new(Vec::new());
    const BLOCK_SIZE: usize = 512;

    fn setup() {
        let _ = gc::init();
        qstr::init();
        mpstate::init();
        init_types();
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
        let _arg = if n_args >= 2 {
            args[1]
        } else {
            obj::CONST_NONE
        };
        match op {
            BLOCKDEV_IOCTL_BLOCK_COUNT => {
                let ram = TEST_RAM.lock().expect("ram lock");
                obj::new_small_int((ram.len() / BLOCK_SIZE) as isize)
            }
            BLOCKDEV_IOCTL_BLOCK_SIZE => obj::new_small_int(BLOCK_SIZE as isize),
            BLOCKDEV_IOCTL_INIT => obj::CONST_NONE,
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

    fn test_mount(blocks: usize) -> Box<FatMount> {
        use fatfs::{FileSystem, FormatVolumeOptions, FsOptions};

        use crate::vfs_blockdev::{VfsBlockdev, BLOCKDEV_FLAG_HAVE_IOCTL};

        *TEST_RAM.lock().expect("ram lock") = vec![0u8; blocks * BLOCK_SIZE];
        let mut mount = Box::new(FatMount {
            blockdev: VfsBlockdev::default(),
            fs: None,
            no_filesystem: false,
            cwd: String::new(),
        });
        mount.blockdev.block_size = BLOCK_SIZE;
        mount.blockdev.flags |= BLOCKDEV_FLAG_HAVE_IOCTL;
        mount.blockdev.readblocks[0] = mk_var(2, 3, ram_readblocks);
        mount.blockdev.writeblocks[0] = mk_var(2, 3, ram_writeblocks);
        mount.blockdev.ioctl[0] = mk_var(1, 2, ram_ioctl);

        let block_count = blocks;
        let bdev_ptr = &mut mount.blockdev as *mut VfsBlockdev;
        let mut stream = FatBlockStream::new(bdev_ptr, block_count);
        fatfs::format_volume(&mut stream, FormatVolumeOptions::new()).expect("format");
        stream.seek(std::io::SeekFrom::Start(0)).expect("rewind");
        mount.fs = Some(FileSystem::new(stream, FsOptions::new()).expect("open fs"));
        mount
    }

    fn make_file(
        mount: *mut FatMount,
        path: &str,
        writable: bool,
        type_out: &'static ObjType,
    ) -> Obj {
        let o = malloc::new_obj::<ObjVfsFatFile>().expect("test file");
        unsafe {
            (*o).base.type_ = type_out;
            (*o).mount = mount;
            (*o).path = Box::into_raw(Box::new(path.to_string()));
            (*o).offset = 0;
            (*o).writable = writable;
            (*o).append = false;
            (*o).closed = false;
            obj::from_ptr(o as *const ObjVfsFatFile as *const ())
        }
    }

    fn call_readline(file: Obj) -> String {
        let fun = stream::stream_unbuffered_readline_obj();
        let line = runtime::call_function_n_kw(fun, 1, 0, &[file]);
        objstr::str_get_str(line).to_string()
    }

    #[test]
    fn ram_mount_mkfs() {
        let _guard = py_rs::vm_test::lock();
        setup();
        let mount = test_mount(50);
        assert!(mount.fs.is_some());
    }

    #[test]
    fn file_stream_write_seek_tell_readline_flush() {
        let _guard = py_rs::vm_test::lock();
        setup();
        let mut mount = test_mount(50);
        let mount_ptr = mount.as_mut() as *mut FatMount;
        let file = make_file(mount_ptr, "lines.txt", true, type_textio());

        let payload = b"line1\nline2\n";
        let mut err = 0;
        assert_eq!(
            file_write(file, payload.as_ptr(), payload.len(), &mut err),
            payload.len()
        );
        assert_eq!(err, 0);

        assert_eq!(stream::stream_seek(file, 0, SEEK_SET, &mut err), 0);
        assert_eq!(err, 0);
        assert_eq!(stream::stream_seek(file, 0, SEEK_CUR, &mut err), 0);
        assert_eq!(err, 0);

        let mut line_buf = [0u8; 16];
        assert_eq!(file_read(file, line_buf.as_mut_ptr(), 6, &mut err), 6);
        assert_eq!(&line_buf[..6], b"line1\n");
        assert_eq!(err, 0);
        assert_eq!(file_read(file, line_buf.as_mut_ptr(), 6, &mut err), 6);
        assert_eq!(&line_buf[..6], b"line2\n");

        assert_eq!(
            stream::stream_seek(file, 0, SEEK_END, &mut err),
            payload.len() as i64
        );
        assert_eq!(
            stream::stream_seek(file, 0, SEEK_CUR, &mut err),
            payload.len() as i64
        );

        let mut nlr_buf = py_rs::nlr::NlrBuf::default();
        py_rs::nlr::protect(&mut nlr_buf, || {
            runtime::call_function_n_kw(stream::stream_flush_obj(), 1, 0, &[file]);
        })
        .expect("flush raised");

        let mut tail = [0u8; 8];
        assert_eq!(file_read(file, tail.as_mut_ptr(), tail.len(), &mut err), 0);
        assert_eq!(err, 0);

        assert_eq!(stream::stream_seek(file, 0, SEEK_SET, &mut err), 0);
        assert_eq!(call_readline(file), "line1\n");
        assert_eq!(call_readline(file), "line2\n");
    }
}
