//! rewrite of extmod/vfs_rom_file.c
// symmetry: done

use py_rs::malloc;
use py_rs::map::{self, MapElem};
use py_rs::mpconfig;
use py_rs::mperrno;
use py_rs::obj::{self, BufferFn, BufferInfo, Obj, ObjBase, ObjType, TYPE_FLAG_ITER_IS_STREAM};
use py_rs::objdict::{self, ObjDict};
use py_rs::objstr;
use py_rs::qstr;
use py_rs::raise::{self, MpRaise};
use py_rs::stream::{
    self, StreamP, StreamSeek, SEEK_CUR, SEEK_END, SEEK_SET, STREAM_CLOSE, STREAM_ERROR,
    STREAM_SEEK,
};

use crate::vfs_rom::{self, ObjVfsRom};

#[repr(C)]
pub struct ObjVfsRomFile {
    pub base: ObjBase,
    pub file_size: usize,
    pub file_offset: usize,
    pub file_data: *const u8,
}

/// `mp_vfs_rom_file_open`
pub fn open(self_in: Obj, path_in: Obj, mode_in: Obj) -> Obj {
    let self_ = unsafe { &*(obj::as_ptr(self_in) as *const ObjVfsRom) };

    let mode_s = objstr::str_get_str(mode_in);
    let mut type_out = type_textio();
    for b in mode_s.bytes() {
        match b {
            b'r' => {}
            b'w' | b'a' | b'+' => raise::raise(MpRaise::OSError(mperrno::EROFS)),
            b'b' => type_out = type_fileio(),
            b't' => type_out = type_textio(),
            _ => {}
        }
    }

    let o = malloc::new_obj::<ObjVfsRomFile>().expect("VfsRom file");
    unsafe {
        (*o).base.type_ = type_out;
        (*o).file_offset = 0;
    }

    let path = objstr::str_get_str(path_in);
    let mut file_size = 0usize;
    let mut file_data = core::ptr::null();
    let stat = vfs_rom::search_filesystem(self_, &path, Some(&mut file_size), Some(&mut file_data));
    match stat {
        py_rs::builtinimport::ImportStat::NoExist => {
            raise::raise(MpRaise::OSError(mperrno::ENOENT));
        }
        py_rs::builtinimport::ImportStat::Dir => {
            raise::raise(MpRaise::OSError(mperrno::EISDIR));
        }
        py_rs::builtinimport::ImportStat::File => {}
    }

    unsafe {
        (*o).file_size = file_size;
        (*o).file_data = file_data;
        obj::from_ptr(o as *const ObjVfsRomFile as *const ())
    }
}

fn file_get_buffer(self_in: Obj, bufinfo: &mut BufferInfo, flags: u32) -> obj::Int {
    let self_ = unsafe { &*(obj::as_ptr(self_in) as *const ObjVfsRomFile) };
    if flags == obj::BUFFER_READ {
        bufinfo.buf = self_.file_data as *mut u8;
        bufinfo.len = self_.file_size;
        bufinfo.typecode = b'B' as i32;
        0
    } else {
        1
    }
}

fn file_read(self_in: Obj, buf: *mut u8, size: usize, errcode: *mut i32) -> usize {
    let self_ = unsafe { &*(obj::as_ptr(self_in) as *const ObjVfsRomFile) };
    unsafe {
        *errcode = 0;
    }
    let remain = self_.file_size.saturating_sub(self_.file_offset);
    let size = size.min(remain);
    if size > 0 {
        unsafe {
            std::ptr::copy_nonoverlapping(self_.file_data.add(self_.file_offset), buf, size);
        }
    }
    unsafe {
        (*(obj::as_ptr(self_in) as *mut ObjVfsRomFile)).file_offset += size;
    }
    size
}

fn file_ioctl(self_in: Obj, request: u32, arg: usize, errcode: *mut i32) -> usize {
    let self_ = unsafe { &mut *(obj::as_ptr(self_in) as *mut ObjVfsRomFile) };
    match request {
        STREAM_SEEK => {
            let s = unsafe { &mut *(arg as *mut StreamSeek) };
            self_.file_offset = match s.whence {
                SEEK_SET => s.offset as usize,
                SEEK_CUR => (self_.file_offset as i64 + s.offset) as usize,
                SEEK_END => (self_.file_size as i64 + s.offset) as usize,
                _ => {
                    unsafe {
                        *errcode = mperrno::EINVAL;
                    }
                    return STREAM_ERROR;
                }
            };
            if self_.file_offset > self_.file_size {
                if s.offset < 0 {
                    unsafe {
                        *errcode = mperrno::EINVAL;
                    }
                    return STREAM_ERROR;
                }
                self_.file_offset = self_.file_size;
            }
            s.offset = self_.file_offset as i64;
            0
        }
        STREAM_CLOSE => 0,
        _ => {
            unsafe {
                *errcode = mperrno::EINVAL;
            }
            STREAM_ERROR
        }
    }
}

static FILEIO_STREAM: StreamP = StreamP {
    read: Some(file_read),
    write: None,
    ioctl: Some(file_ioctl),
    is_text: false,
};

static TEXTIO_STREAM: StreamP = StreamP {
    read: Some(file_read),
    write: None,
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
    flags: py_rs::obj::TYPE_FLAG_BINDS_SELF | py_rs::obj::TYPE_FLAG_BUILTIN_FUN,
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
    let o = malloc::new_obj::<ObjFunBuiltin1>().expect("vfs_rom file fn1");
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
                key: obj::new_qstr(qstr::from_str("seek")),
                value: stream::stream_seek_obj(),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("tell")),
                value: stream::stream_tell_obj(),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("close")),
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

static mut FILEIO_SLOTS: [*const (); 3] = [core::ptr::null(); 3];
static mut TYPE_FILEIO: ObjType = ObjType {
    base: ObjBase {
        type_: core::ptr::null(),
    },
    flags: TYPE_FLAG_ITER_IS_STREAM,
    name: 0,
    slot_index_make_new: 0,
    slot_index_print: 0,
    slot_index_call: 0,
    slot_index_unary_op: 0,
    slot_index_binary_op: 0,
    slot_index_attr: 0,
    slot_index_subscr: 0,
    slot_index_iter: 0,
    slot_index_buffer: 1,
    slot_index_protocol: 2,
    slot_index_parent: 0,
    slot_index_locals_dict: 3,
    slots: unsafe { FILEIO_SLOTS.as_ptr() },
};

static mut TEXTIO_SLOTS: [*const (); 2] = [core::ptr::null(); 2];
static mut TYPE_TEXTIO: ObjType = ObjType {
    base: ObjBase {
        type_: core::ptr::null(),
    },
    flags: TYPE_FLAG_ITER_IS_STREAM,
    name: 0,
    slot_index_make_new: 0,
    slot_index_print: 0,
    slot_index_call: 0,
    slot_index_unary_op: 0,
    slot_index_binary_op: 0,
    slot_index_attr: 0,
    slot_index_subscr: 0,
    slot_index_iter: 0,
    slot_index_buffer: 0,
    slot_index_protocol: 1,
    slot_index_parent: 0,
    slot_index_locals_dict: 2,
    slots: unsafe { TEXTIO_SLOTS.as_ptr() },
};

static TYPE_INIT: std::sync::OnceLock<()> = std::sync::OnceLock::new();

fn init_types() {
    TYPE_INIT.get_or_init(|| {
        let dict = locals_dict();
        unsafe {
            FILEIO_SLOTS[0] = file_get_buffer as BufferFn as *const ();
            FILEIO_SLOTS[1] = &FILEIO_STREAM as *const StreamP as *const ();
            FILEIO_SLOTS[2] = dict;
            TYPE_FILEIO.name = qstr::from_str("FileIO");

            TEXTIO_SLOTS[0] = &TEXTIO_STREAM as *const StreamP as *const ();
            TEXTIO_SLOTS[1] = dict;
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
    mpconfig::VFS_ROM && mpconfig::PY_VFS
}
