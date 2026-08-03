//! rewrite of py/objstringio.c + py/objstringio.h
// symmetry: done

use crate::argcheck;
use crate::malloc;
use crate::map::{self, MapElem};
use crate::mpconfig;
use crate::mpprint::{self, Print, PrintKind, VaArg};
use crate::obj::{
    self, Obj, ObjBase, ObjType, TYPE_FLAG_BINDS_SELF, TYPE_FLAG_BUILTIN_FUN,
    TYPE_FLAG_ITER_IS_STREAM,
};
use crate::objdict::{self, ObjDict};
use crate::objstr;
use crate::qstr;
use crate::raise::{self, MpRaise};
use crate::stream::{self, StreamP, StreamSeek, SEEK_CUR, SEEK_END, SEEK_SET, STREAM_ERROR};
use crate::vstr::{self, Vstr};

#[repr(C)]
pub struct ObjStringio {
    pub base: ObjBase,
    pub vstr: *mut Vstr,
    pub pos: usize,
    pub ref_obj: Obj,
}

fn check_open(o: &ObjStringio) {
    if mpconfig::CPYTHON_COMPAT && o.vstr.is_null() {
        raise::raise(MpRaise::ValueError("I/O operation on closed file"));
    }
}

fn stringio_print(print: &Print, self_in: Obj, _kind: PrintKind) {
    let self_ = unsafe { &*(obj::as_ptr(self_in) as *const ObjStringio) };
    let tag = if core::ptr::eq(self_.base.type_, type_stringio()) {
        "StringIO"
    } else {
        "BytesIO"
    };
    mpprint::printf(
        print,
        "<io.{} {:p}>",
        [VaArg::Str(tag), VaArg::USize(self_in.0)],
    );
}

fn stringio_read(self_in: Obj, buf: *mut u8, size: usize, errcode: *mut i32) -> usize {
    let o = unsafe { &mut *(obj::as_ptr(self_in) as *mut ObjStringio) };
    check_open(o);
    unsafe {
        *errcode = 0;
    }
    let v = unsafe { &*o.vstr };
    if v.len <= o.pos {
        return 0;
    }
    let remaining = v.len - o.pos;
    let size = size.min(remaining);
    unsafe {
        std::ptr::copy_nonoverlapping(v.buf.add(o.pos), buf, size);
    }
    o.pos += size;
    size
}

fn stringio_copy_on_write(o: &mut ObjStringio) {
    let v = unsafe { &*o.vstr };
    let new_buf = malloc::new::<u8>(v.len).expect("stringio cow");
    unsafe {
        std::ptr::copy_nonoverlapping(v.buf, new_buf, v.len);
        let nv = Vstr {
            alloc: v.len,
            len: v.len,
            buf: new_buf,
            fixed_buf: false,
        };
        vstr::free(o.vstr);
        o.vstr = malloc::new_obj::<Vstr>().expect("vstr");
        *o.vstr = nv;
    }
}

fn stringio_write(self_in: Obj, buf: *const u8, size: usize, errcode: *mut i32) -> usize {
    let o = unsafe { &mut *(obj::as_ptr(self_in) as *mut ObjStringio) };
    check_open(o);
    unsafe {
        *errcode = 0;
    }
    let v = unsafe { &mut *o.vstr };
    if v.fixed_buf {
        stringio_copy_on_write(o);
    }
    let v = unsafe { &mut *o.vstr };
    let new_pos = o.pos + size;
    if new_pos < o.pos {
        unsafe {
            *errcode = 27;
        }
        return STREAM_ERROR;
    }
    let org_len = v.len;
    if new_pos > v.alloc {
        v.len = v.alloc;
        vstr::add_len(v, new_pos - v.alloc);
    }
    if o.pos > org_len {
        unsafe {
            std::ptr::write_bytes(v.buf.add(org_len), 0, o.pos - org_len);
        }
    }
    unsafe {
        std::ptr::copy_nonoverlapping(buf, v.buf.add(o.pos), size);
    }
    o.pos = new_pos;
    if new_pos > v.len {
        v.len = new_pos;
    }
    size
}

fn stringio_ioctl(self_in: Obj, request: u32, arg: usize, errcode: *mut i32) -> usize {
    let o = unsafe { &mut *(obj::as_ptr(self_in) as *mut ObjStringio) };
    check_open(o);
    unsafe {
        *errcode = 0;
    }
    match request {
        stream::STREAM_SEEK => {
            let s = unsafe { &mut *(arg as *mut StreamSeek) };
            let v = unsafe { &*o.vstr };
            let ref_ = match s.whence {
                SEEK_CUR => o.pos,
                SEEK_END => v.len,
                _ => 0,
            };
            let new_pos = if s.whence != SEEK_SET && s.offset < 0 {
                ref_.saturating_sub((-s.offset) as usize)
            } else {
                ref_ + s.offset as usize
            };
            s.offset = new_pos as i64;
            o.pos = new_pos;
            0
        }
        stream::STREAM_FLUSH => 0,
        stream::STREAM_CLOSE => {
            if mpconfig::CPYTHON_COMPAT {
                vstr::free(o.vstr);
                o.vstr = core::ptr::null_mut();
            } else {
                let v = unsafe { &mut *o.vstr };
                vstr::clear(v);
                v.alloc = 0;
                v.len = 0;
                o.pos = 0;
            }
            0
        }
        _ => {
            unsafe {
                *errcode = 22;
            }
            STREAM_ERROR
        }
    }
}

fn content_type(o: &ObjStringio) -> &'static ObjType {
    if core::ptr::eq(o.base.type_, type_stringio()) {
        objstr::type_str()
    } else {
        objstr::type_bytes()
    }
}

fn stringio_getvalue(self_in: Obj) -> Obj {
    let self_ = unsafe { &*(obj::as_ptr(self_in) as *const ObjStringio) };
    check_open(self_);
    let v = unsafe { &*self_.vstr };
    let slice = unsafe { std::slice::from_raw_parts(v.buf, v.len) };
    objstr::new_str_of_type(content_type(self_), slice)
}

fn stringio_new(type_in: &ObjType) -> *mut ObjStringio {
    let type_static: &'static ObjType = unsafe { &*(type_in as *const ObjType) };
    obj::malloc_helper(core::mem::size_of::<ObjStringio>(), type_static) as *mut ObjStringio
}

fn stringio_make_new(type_in: &ObjType, n_args: usize, n_kw: usize, args: &[Obj]) -> Obj {
    argcheck::check_num(n_args, n_kw, 0, 1, false);
    let type_static: &'static ObjType = unsafe { &*(type_in as *const ObjType) };
    let o =
        obj::malloc_helper(core::mem::size_of::<ObjStringio>(), type_static) as *mut ObjStringio;
    unsafe {
        (*o).pos = 0;
        (*o).ref_obj = obj::OBJ_NULL;
        if n_args == 0 {
            (*o).vstr = vstr::new(16);
            return obj::from_ptr(o as *const ObjStringio as *const ());
        }
        if obj::is_int(args[0]) {
            (*o).vstr = vstr::new(obj::get_int(args[0]) as usize);
            return obj::from_ptr(o as *const ObjStringio as *const ());
        }
        let mut bufinfo = obj::BufferInfo::default();
        obj::get_buffer_raise(args[0], &mut bufinfo, obj::BUFFER_READ);
        if obj::is_str_or_bytes(args[0]) {
            (*o).vstr = malloc::new_obj::<Vstr>().expect("vstr");
            vstr::init_fixed_buf(&mut *(*o).vstr, bufinfo.len, bufinfo.buf);
            (*(*o).vstr).len = bufinfo.len;
            (*o).ref_obj = args[0];
            return obj::from_ptr(o as *const ObjStringio as *const ());
        }
        (*o).vstr = vstr::new(bufinfo.len);
        let self_obj = obj::from_ptr(o as *const ObjStringio as *const ());
        let mut err = 0;
        stringio_write(self_obj, bufinfo.buf, bufinfo.len, &mut err);
        (*o).pos = 0;
        self_obj
    }
}

static STRINGIO_STREAM: StreamP = StreamP {
    read: Some(stringio_read),
    write: Some(stringio_write),
    ioctl: Some(stringio_ioctl),
    is_text: true,
};

static BYTESIO_STREAM: StreamP = StreamP {
    read: Some(stringio_read),
    write: Some(stringio_write),
    ioctl: Some(stringio_ioctl),
    is_text: false,
};

type BuiltinFn1 = fn(Obj) -> Obj;

#[repr(C)]
struct ObjFunBuiltin1 {
    base: ObjBase,
    fun: BuiltinFn1,
}

static mut F1S: [*const (); 1] = [f1 as *const ()];
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
    slots: unsafe { F1S.as_ptr() },
};

fn f1(s: Obj, n: usize, k: usize, a: &[Obj]) -> Obj {
    argcheck::check_num(n, k, 1, 1, false);
    let self_ = unsafe { &*(obj::as_ptr(s) as *const ObjFunBuiltin1) };
    (self_.fun)(a[0])
}

fn mk1(f: BuiltinFn1) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltin1>().expect("stringio fun");
    unsafe {
        (*o).base.type_ = &TF1 as *const ObjType;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltin1 as *const ())
    }
}

fn locals_dict() -> *const () {
    static mut DICT: Option<*const ()> = None;
    unsafe {
        if DICT.is_none() {
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
                    key: obj::new_qstr(qstr::from_str("getvalue")),
                    value: mk1(stringio_getvalue),
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
            map::init_fixed_table(&mut (*ptr).map, table);
            DICT = Some(obj::from_ptr(ptr as *const ObjDict as *const ()).0 as *const ());
        }
        DICT.unwrap()
    }
}

static mut STRINGIO_SLOTS: [*const (); 4] = [
    stringio_make_new as *const (),
    stringio_print as *const (),
    &STRINGIO_STREAM as *const StreamP as *const (),
    core::ptr::null(),
];

static mut BYTESIO_SLOTS: [*const (); 4] = [
    stringio_make_new as *const (),
    stringio_print as *const (),
    &BYTESIO_STREAM as *const StreamP as *const (),
    core::ptr::null(),
];

static mut TYPE_STRINGIO: ObjType = ObjType {
    base: ObjBase {
        type_: core::ptr::null(),
    },
    flags: TYPE_FLAG_ITER_IS_STREAM,
    name: 0,
    slot_index_make_new: 1,
    slot_index_print: 2,
    slot_index_call: 0,
    slot_index_unary_op: 0,
    slot_index_binary_op: 0,
    slot_index_attr: 0,
    slot_index_subscr: 0,
    slot_index_iter: 0,
    slot_index_buffer: 0,
    slot_index_protocol: 3,
    slot_index_parent: 0,
    slot_index_locals_dict: 4,
    slots: unsafe { STRINGIO_SLOTS.as_ptr() },
};

static mut TYPE_BYTESIO: ObjType = ObjType {
    base: ObjBase {
        type_: core::ptr::null(),
    },
    flags: TYPE_FLAG_ITER_IS_STREAM,
    name: 0,
    slot_index_make_new: 1,
    slot_index_print: 2,
    slot_index_call: 0,
    slot_index_unary_op: 0,
    slot_index_binary_op: 0,
    slot_index_attr: 0,
    slot_index_subscr: 0,
    slot_index_iter: 0,
    slot_index_buffer: 0,
    slot_index_protocol: 3,
    slot_index_parent: 0,
    slot_index_locals_dict: 4,
    slots: unsafe { BYTESIO_SLOTS.as_ptr() },
};

static INIT: std::sync::OnceLock<()> = std::sync::OnceLock::new();

fn init_types() {
    INIT.get_or_init(|| unsafe {
        TYPE_STRINGIO.name = qstr::from_str("StringIO");
        TYPE_BYTESIO.name = qstr::from_str("BytesIO");
        STRINGIO_SLOTS[3] = locals_dict();
        BYTESIO_SLOTS[3] = locals_dict();
    });
}

pub fn type_stringio() -> &'static ObjType {
    if !mpconfig::PY_IO {
        panic!("io disabled");
    }
    init_types();
    unsafe { &TYPE_STRINGIO }
}

pub fn type_bytesio() -> &'static ObjType {
    if !(mpconfig::PY_IO && mpconfig::PY_IO_BYTESIO) {
        panic!("BytesIO disabled");
    }
    init_types();
    unsafe { &TYPE_BYTESIO }
}
