//! rewrite of extmod/vfs_posix_file.c
// symmetry: done

use std::ffi::CString;

use py_rs::malloc;
use py_rs::map::{self, MapElem};
use py_rs::mpconfig;
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
    self, StreamP, StreamSeek, STREAM_CLOSE, STREAM_ERROR, STREAM_FLUSH, STREAM_GET_FILENO,
    STREAM_SEEK,
};

#[repr(C)]
pub struct ObjVfsPosixFile {
    pub base: ObjBase,
    pub fd: i32,
}

fn check_fd_is_open(o: &ObjVfsPosixFile) {
    if mpconfig::CPYTHON_COMPAT && o.fd < 0 {
        raise::raise(MpRaise::ValueError("I/O operation on closed file"));
    }
}

fn file_print(print: &Print, self_in: Obj, _kind: PrintKind) {
    let self_ = unsafe { &*(obj::as_ptr(self_in) as *const ObjVfsPosixFile) };
    let tag = if core::ptr::eq(self_.base.type_, type_fileio()) {
        "FileIO"
    } else {
        "TextIOWrapper"
    };
    mpprint::printf(print, "<io.{} {}>", [VaArg::Str(tag), VaArg::Int(self_.fd)]);
}

/// `mp_vfs_posix_file_open`
pub fn open(type_in: &'static ObjType, file_in: Obj, mode_in: Obj) -> Obj {
    let mode_s = objstr::str_get_str(mode_in);
    let mut mode_rw = 0i32;
    let mut mode_x = 0i32;
    let mut type_out = type_in;
    for b in mode_s.bytes() {
        match b {
            b'r' => mode_rw = libc::O_RDONLY,
            b'w' => {
                mode_rw = libc::O_WRONLY;
                mode_x = libc::O_CREAT | libc::O_TRUNC;
            }
            b'a' => {
                mode_rw = libc::O_WRONLY;
                mode_x = libc::O_CREAT | libc::O_APPEND;
            }
            b'+' => mode_rw = libc::O_RDWR,
            b'b' => type_out = type_fileio(),
            b't' => type_out = type_textio(),
            _ => {}
        }
    }

    let o = malloc::new_obj::<ObjVfsPosixFile>().expect("posix file");
    unsafe {
        (*o).base.type_ = type_out;
        (*o).fd = -1;
    }

    if obj::is_small_int(file_in) {
        unsafe {
            (*o).fd = obj::small_int_value(file_in) as i32;
        }
        return obj::from_ptr(o as *const ObjVfsPosixFile as *const ());
    }

    let fname = objstr::str_get_str(file_in);
    let cpath = CString::new(fname).unwrap_or_default();
    let fd = unsafe { libc::open(cpath.as_ptr(), mode_x | mode_rw, 0o644) };
    if fd < 0 {
        raise::raise(MpRaise::OSError(errno()));
    }
    unsafe {
        (*o).fd = fd;
    }
    obj::from_ptr(o as *const ObjVfsPosixFile as *const ())
}

fn errno() -> i32 {
    unsafe { *libc::__errno_location() }
}

fn file_fileno(self_in: Obj) -> Obj {
    let self_ = unsafe { &*(obj::as_ptr(self_in) as *const ObjVfsPosixFile) };
    check_fd_is_open(self_);
    obj::new_small_int(self_.fd as isize)
}

fn file_read(self_in: Obj, buf: *mut u8, size: usize, errcode: *mut i32) -> usize {
    let o = unsafe { &*(obj::as_ptr(self_in) as *const ObjVfsPosixFile) };
    check_fd_is_open(o);
    unsafe {
        *errcode = 0;
        let r = libc::read(o.fd, buf as *mut _, size);
        if r < 0 {
            *errcode = errno();
            return STREAM_ERROR;
        }
        r as usize
    }
}

fn file_write(self_in: Obj, buf: *const u8, size: usize, errcode: *mut i32) -> usize {
    let o = unsafe { &*(obj::as_ptr(self_in) as *const ObjVfsPosixFile) };
    check_fd_is_open(o);
    unsafe {
        *errcode = 0;
        let r = libc::write(o.fd, buf as *const _, size);
        if r < 0 {
            *errcode = errno();
            return STREAM_ERROR;
        }
        r as usize
    }
}

fn file_ioctl(self_in: Obj, request: u32, arg: usize, errcode: *mut i32) -> usize {
    let o = unsafe { &mut *(obj::as_ptr(self_in) as *mut ObjVfsPosixFile) };
    match request {
        STREAM_FLUSH => {
            check_fd_is_open(o);
            unsafe {
                let _ = libc::fsync(o.fd);
            }
            0
        }
        STREAM_SEEK => {
            check_fd_is_open(o);
            let s = unsafe { &mut *(arg as *mut StreamSeek) };
            let off = unsafe { libc::lseek(o.fd, s.offset, s.whence) };
            if off == -1 {
                unsafe {
                    *errcode = errno();
                }
                return STREAM_ERROR;
            }
            s.offset = off;
            0
        }
        STREAM_CLOSE => {
            if o.fd >= 0 {
                unsafe {
                    libc::close(o.fd);
                }
            }
            o.fd = -1;
            0
        }
        STREAM_GET_FILENO => {
            check_fd_is_open(o);
            o.fd as usize
        }
        _ => {
            unsafe {
                *errcode = 22;
            }
            STREAM_ERROR
        }
    }
}

static FILEIO_STREAM: StreamP = StreamP {
    read: Some(file_read),
    write: Some(file_write),
    ioctl: Some(file_ioctl),
    is_text: false,
};

static TEXTIO_STREAM: StreamP = StreamP {
    read: Some(file_read),
    write: Some(file_write),
    ioctl: Some(file_ioctl),
    is_text: true,
};

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
    let o = malloc::new_obj::<ObjFunBuiltin1>().expect("posix file fn1");
    unsafe {
        (*o).base.type_ = &TF1;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltin1 as *const ())
    }
}

fn locals_dict() -> *const () {
    static INIT: std::sync::OnceLock<()> = std::sync::OnceLock::new();
    static mut DICT: *const () = core::ptr::null();
    INIT.get_or_init(|| {
        let table = vec![
            MapElem {
                key: obj::new_qstr(qstr::from_str("fileno")),
                value: mk1(file_fileno),
            },
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
            DICT = obj::from_ptr(ptr as *const ObjDict as *const ()).0 as *const ();
        }
    });
    unsafe { DICT }
}

static mut FILEIO_SLOTS: [*const (); 4] = [core::ptr::null(); 4];
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

static mut TEXTIO_SLOTS: [*const (); 4] = [core::ptr::null(); 4];
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

static TYPE_INIT: std::sync::OnceLock<()> = std::sync::OnceLock::new();

fn init_types() {
    TYPE_INIT.get_or_init(|| {
        let dict = locals_dict();
        unsafe {
            FILEIO_SLOTS[0] = file_print as *const ();
            FILEIO_SLOTS[1] = &FILEIO_STREAM as *const StreamP as *const ();
            FILEIO_SLOTS[2] = dict;
            TYPE_FILEIO.name = qstr::from_str("FileIO");

            TEXTIO_SLOTS[0] = file_print as *const ();
            TEXTIO_SLOTS[1] = &TEXTIO_STREAM as *const StreamP as *const ();
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
    mpconfig::VFS_POSIX && mpconfig::PY_VFS
}

/// Install TextIOWrapper objects for fds 0/1/2 into `sys` (C `mp_sys_std*_obj`).
pub fn install_sys_stdfiles() {
    if !(mpconfig::PY_SYS_STDFILES && enabled()) {
        return;
    }
    let r = objstr::new_str(b"r");
    let w = objstr::new_str(b"w");
    let stdin = open(type_textio(), obj::new_small_int(0), r);
    let stdout = open(type_textio(), obj::new_small_int(1), w);
    let stderr = open(type_textio(), obj::new_small_int(2), w);
    py_rs::modsys::set_sys_stdio(stdin, stdout, stderr);
}
