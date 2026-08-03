//! rewrite of py/objringio.c
// symmetry: done

use crate::argcheck;
use crate::map::{self, MapElem};
use crate::malloc;
use crate::mpconfig;
use crate::obj::{self, Obj, ObjBase, ObjType, TYPE_FLAG_BINDS_SELF, TYPE_FLAG_BUILTIN_FUN};
use crate::objdict::{self, ObjDict};
use crate::qstr;
use crate::raise::{self, MpRaise};
use crate::ringbuf::Ringbuf;
use crate::stream::{self, StreamP, STREAM_ERROR, STREAM_POLL_RD, STREAM_POLL_WR};

#[repr(C)]
pub struct ObjRingio {
    pub base: ObjBase,
    pub ringbuffer: Ringbuf,
}

type BuiltinFn1 = fn(Obj) -> Obj;

#[repr(C)]
struct ObjFunBuiltin1 {
    base: ObjBase,
    fun: BuiltinFn1,
}

static mut F1S: [*const (); 1] = [f1 as *const ()];
static TF1: ObjType = ObjType {
    base: ObjBase { type_: core::ptr::null() },
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
    let o = malloc::new_obj::<ObjFunBuiltin1>().expect("ringio fun");
    unsafe {
        (*o).base.type_ = &TF1 as *const ObjType;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltin1 as *const ())
    }
}

fn ringio_read(self_in: Obj, buf: *mut u8, size: usize, errcode: *mut i32) -> usize {
    let self_ = unsafe { &mut *(obj::as_ptr(self_in) as *mut ObjRingio) };
    let n = size.min(self_.ringbuffer.avail());
    if n > 0 {
        let slice = unsafe { std::slice::from_raw_parts_mut(buf, n) };
        let _ = self_.ringbuffer.get_bytes(slice);
    }
    unsafe {
        *errcode = 0;
    }
    n
}

fn ringio_write(self_in: Obj, buf: *const u8, size: usize, errcode: *mut i32) -> usize {
    let self_ = unsafe { &mut *(obj::as_ptr(self_in) as *mut ObjRingio) };
    let n = size.min(self_.ringbuffer.free());
    if n > 0 {
        let slice = unsafe { std::slice::from_raw_parts(buf, n) };
        let _ = self_.ringbuffer.put_bytes(slice);
    }
    unsafe {
        *errcode = 0;
    }
    n
}

fn ringio_ioctl(self_in: Obj, request: u32, arg: usize, errcode: *mut i32) -> usize {
    let self_ = unsafe { &*(obj::as_ptr(self_in) as *const ObjRingio) };
    match request {
        stream::STREAM_POLL => {
            let mut ret = 0u32;
            if (arg as u32 & STREAM_POLL_RD) != 0 && self_.ringbuffer.avail() > 0 {
                ret |= STREAM_POLL_RD;
            }
            if (arg as u32 & STREAM_POLL_WR) != 0 && self_.ringbuffer.free() > 0 {
                ret |= STREAM_POLL_WR;
            }
            ret as usize
        }
        stream::STREAM_CLOSE => 0,
        _ => {
            unsafe {
                *errcode = 22;
            }
            STREAM_ERROR
        }
    }
}

fn ringio_any(self_in: Obj) -> Obj {
    let self_ = unsafe { &*(obj::as_ptr(self_in) as *const ObjRingio) };
    obj::new_small_int(self_.ringbuffer.avail() as isize)
}

fn ringio_make_new(type_in: &ObjType, n_args: usize, n_kw: usize, args: &[Obj]) -> Obj {
    argcheck::check_num(n_args, n_kw, 1, 1, false);
    let mut bufinfo = obj::BufferInfo::default();
    let (owned, size) = if obj::get_buffer(args[0], &mut bufinfo, obj::BUFFER_RW) {
        (false, bufinfo.len)
    } else {
        let sz = obj::get_int(args[0]) as usize + 1;
        (true, sz)
    };
    if size < 2 || size > u16::MAX as usize {
        raise::raise(MpRaise::ValueError(""));
    }
    let type_static: &'static ObjType = unsafe { &*(type_in as *const ObjType) };
    let o = obj::malloc_helper(core::mem::size_of::<ObjRingio>(), type_static) as *mut ObjRingio;
    unsafe {
        if owned {
            (*o).ringbuffer = Ringbuf::new(size);
        } else {
            let mut rb = Ringbuf::new(size);
            rb.buf = std::slice::from_raw_parts(bufinfo.buf, bufinfo.len).to_vec();
            (*o).ringbuffer = rb;
        }
        obj::from_ptr(o as *const ObjRingio as *const ())
    }
}

static RINGIO_STREAM: StreamP = StreamP {
    read: Some(ringio_read),
    write: Some(ringio_write),
    ioctl: Some(ringio_ioctl),
    is_text: false,
};

static mut RINGIO_SLOTS: [*const (); 3] = [
    ringio_make_new as *const (),
    &RINGIO_STREAM as *const StreamP as *const (),
    core::ptr::null(),
];

static mut TYPE_RINGIO: ObjType = ObjType {
    base: ObjBase { type_: core::ptr::null() },
    flags: 0,
    name: 0,
    slot_index_make_new: 1,
    slot_index_print: 0,
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
    slots: unsafe { RINGIO_SLOTS.as_ptr() },
};

static INIT: std::sync::OnceLock<()> = std::sync::OnceLock::new();

fn init_type() {
    INIT.get_or_init(|| {
        let table = vec![
            MapElem { key: obj::new_qstr(qstr::from_str("any")), value: mk1(ringio_any) },
            MapElem { key: obj::new_qstr(qstr::from_str("read")), value: stream::stream_read_obj() },
            MapElem { key: obj::new_qstr(qstr::from_str("readline")), value: stream::stream_unbuffered_readline_obj() },
            MapElem { key: obj::new_qstr(qstr::from_str("readinto")), value: stream::stream_readinto_obj() },
            MapElem { key: obj::new_qstr(qstr::from_str("write")), value: stream::stream_write_obj() },
            MapElem { key: obj::new_qstr(qstr::from_str("close")), value: stream::stream_close_obj() },
        ];
        let ptr = obj::malloc_helper(core::mem::size_of::<ObjDict>(), objdict::type_dict()) as *mut ObjDict;
        unsafe {
            map::init_fixed_table(&mut (*ptr).map, table);
            RINGIO_SLOTS[2] = obj::from_ptr(ptr as *const ObjDict as *const ()).0 as *const ();
            TYPE_RINGIO.name = qstr::from_str("RingIO");
        }
    });
}

pub fn type_ringio() -> &'static ObjType {
    if !mpconfig::PY_MICROPYTHON_RINGIO {
        panic!("RingIO disabled");
    }
    init_type();
    unsafe { &TYPE_RINGIO }
}
