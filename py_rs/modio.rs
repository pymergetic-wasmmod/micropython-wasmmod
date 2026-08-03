//! rewrite of py/modio.c
// symmetry: done

use crate::bc::ModuleContext;
use crate::builtin;
use crate::malloc;
use crate::map::{self, MapElem};
use crate::mpconfig;
use crate::obj::{self, Obj, ObjBase, ObjType};
use crate::objdict;
use crate::objmodule;
use crate::objstringio;
use crate::qstr;
use crate::runtime;
use crate::stream::{StreamP, STREAM_ERROR};

static mut IOBASE_SLOTS: [*const (); 2] = [iobase_make_new as *const (), core::ptr::null()];
static mut TYPE_IOBASE: ObjType = ObjType {
    base: ObjBase {
        type_: core::ptr::null(),
    },
    flags: obj::TYPE_FLAG_NONE,
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
    slot_index_locals_dict: 0,
    slots: unsafe { IOBASE_SLOTS.as_ptr() },
};

static IOBASE_STREAM: StreamP = StreamP {
    read: Some(iobase_read),
    write: Some(iobase_write),
    ioctl: Some(iobase_ioctl),
    is_text: false,
};

static mut IOBASE_SINGLETON: ObjBase = ObjBase {
    type_: core::ptr::null(),
};

static IOBASE_INIT: std::sync::OnceLock<()> = std::sync::OnceLock::new();

fn init_iobase() {
    IOBASE_INIT.get_or_init(|| unsafe {
        TYPE_IOBASE.name = qstr::from_str("IOBase");
        IOBASE_SLOTS[1] = &IOBASE_STREAM as *const StreamP as *const ();
        IOBASE_SINGLETON.type_ = &raw const TYPE_IOBASE as *const ObjType;
    });
}

fn iobase_make_new(_type_in: &ObjType, _n_args: usize, _n_kw: usize, _args: &[Obj]) -> Obj {
    init_iobase();
    unsafe { obj::from_ptr(&raw const IOBASE_SINGLETON as *const ()) }
}

/// Delegate to instance `readinto` / `write` (C `iobase_read_write`).
fn iobase_rw(obj: Obj, buf: *mut u8, size: usize, errcode: *mut i32, method: &str) -> usize {
    let mut dest = [obj::OBJ_NULL, obj::OBJ_NULL];
    runtime::load_method(obj, qstr::from_str(method), &mut dest);
    let view = crate::objarray::new_bytearray_by_ref(size, buf);
    let call_args = [dest[0], dest[1], view];
    let ret_obj = runtime::call_method_n_kw(1, 0, &call_args);
    if ret_obj == obj::CONST_NONE {
        unsafe {
            *errcode = 11;
        }
        return STREAM_ERROR;
    }
    let ret = obj::get_int(ret_obj);
    if ret >= 0 {
        if (ret as usize) > size {
            unsafe {
                *errcode = 5;
            }
            return STREAM_ERROR;
        }
        ret as usize
    } else {
        unsafe {
            *errcode = (-ret) as i32;
        }
        STREAM_ERROR
    }
}

fn iobase_read(obj: Obj, buf: *mut u8, size: usize, errcode: *mut i32) -> usize {
    iobase_rw(obj, buf, size, errcode, "readinto")
}

fn iobase_write(obj: Obj, buf: *const u8, size: usize, errcode: *mut i32) -> usize {
    iobase_rw(obj, buf as *mut u8, size, errcode, "write")
}

fn iobase_ioctl(obj: Obj, request: u32, arg: usize, errcode: *mut i32) -> usize {
    let mut dest = [obj::OBJ_NULL, obj::OBJ_NULL];
    runtime::load_method(obj, qstr::from_str("ioctl"), &mut dest);
    let call_args = [
        dest[0],
        dest[1],
        obj::new_int(request as i64 as crate::obj::Int),
        obj::new_int(arg as i64 as crate::obj::Int),
    ];
    let ret = obj::get_int(runtime::call_method_n_kw(2, 0, &call_args));
    if ret >= 0 {
        ret as usize
    } else {
        unsafe {
            *errcode = (-ret) as i32;
        }
        STREAM_ERROR
    }
}

pub fn type_iobase() -> &'static ObjType {
    init_iobase();
    unsafe { &TYPE_IOBASE }
}

pub fn init_module() -> Obj {
    if !mpconfig::PY_IO {
        return obj::OBJ_NULL;
    }
    let mut table = vec![
        MapElem {
            key: obj::new_qstr(qstr::from_str("__name__")),
            value: obj::new_qstr(qstr::from_str("io")),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("open")),
            value: obj::from_ptr(builtin::builtin_open as *const ()),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("StringIO")),
            value: obj::from_ptr(objstringio::type_stringio() as *const obj::ObjType as *const ()),
        },
    ];
    if mpconfig::PY_IO_BYTESIO {
        table.push(MapElem {
            key: obj::new_qstr(qstr::from_str("BytesIO")),
            value: obj::from_ptr(objstringio::type_bytesio() as *const obj::ObjType as *const ()),
        });
    }
    if mpconfig::PY_IO_IOBASE {
        table.push(MapElem {
            key: obj::new_qstr(qstr::from_str("IOBase")),
            value: obj::from_ptr(type_iobase() as *const ObjType as *const ()),
        });
    }
    let ctx = malloc::new_obj::<ModuleContext>().expect("io module");
    let dict = objdict::new_dict(table.len());
    unsafe {
        map::init_fixed_table(&mut (*objdict::dict_ptr(dict)).map, table);
        (*ctx).module.base.type_ = objmodule::type_module();
        (*ctx).module.globals = objdict::dict_ptr(dict);
        (*ctx).constants = Default::default();
    }
    let module = obj::from_ptr(ctx as *const ModuleContext as *const ());
    objmodule::register_builtin_module(qstr::from_str("io"), module);
    module
}
