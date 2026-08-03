//! rewrite of py/objobject.c
// symmetry: done

use crate::argcheck;
use crate::map::{self, MapElem};
use crate::mpconfig;
use crate::obj::{self, Obj, ObjBase, ObjType, TYPE_FLAG_BINDS_SELF, TYPE_FLAG_BUILTIN_FUN};
use crate::objdict::{self, ObjDict};
use crate::qstr;
use crate::raise::{self, MpRaise};

#[repr(C)]
pub struct ObjObject {
    pub base: ObjBase,
}

type BuiltinFn0 = fn() -> Obj;
type BuiltinFn1 = fn(Obj) -> Obj;
type BuiltinFn2 = fn(Obj, Obj) -> Obj;
type BuiltinFn3 = fn(Obj, Obj, Obj) -> Obj;

#[repr(C)]
struct ObjFunBuiltin0 {
    base: ObjBase,
    fun: BuiltinFn0,
}
#[repr(C)]
struct ObjFunBuiltin1 {
    base: ObjBase,
    fun: BuiltinFn1,
}
#[repr(C)]
struct ObjFunBuiltin2 {
    base: ObjBase,
    fun: BuiltinFn2,
}
#[repr(C)]
struct ObjFunBuiltin3 {
    base: ObjBase,
    fun: BuiltinFn3,
}

static mut F0S: [*const (); 1] = [f0 as *const ()];
static mut F1S: [*const (); 1] = [f1 as *const ()];
static mut F2S: [*const (); 1] = [f2 as *const ()];
static mut F3S: [*const (); 1] = [f3 as *const ()];

static TF0: ObjType = ObjType {
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
    slots: unsafe { F0S.as_ptr() },
};
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
static TF2: ObjType = ObjType {
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
    slots: unsafe { F2S.as_ptr() },
};
static TF3: ObjType = ObjType {
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
    slots: unsafe { F3S.as_ptr() },
};

fn f0(s: Obj, n: usize, k: usize, _a: &[Obj]) -> Obj {
    argcheck::check_num(n, k, 0, 0, false);
    let self_ = unsafe { &*(obj::as_ptr(s) as *const ObjFunBuiltin0) };
    (self_.fun)()
}
fn f1(s: Obj, n: usize, k: usize, a: &[Obj]) -> Obj {
    argcheck::check_num(n, k, 1, 1, false);
    let self_ = unsafe { &*(obj::as_ptr(s) as *const ObjFunBuiltin1) };
    (self_.fun)(a[0])
}
fn f2(s: Obj, n: usize, k: usize, a: &[Obj]) -> Obj {
    argcheck::check_num(n, k, 2, 2, false);
    let self_ = unsafe { &*(obj::as_ptr(s) as *const ObjFunBuiltin2) };
    (self_.fun)(a[0], a[1])
}
fn f3(s: Obj, n: usize, k: usize, a: &[Obj]) -> Obj {
    argcheck::check_num(n, k, 3, 3, false);
    let self_ = unsafe { &*(obj::as_ptr(s) as *const ObjFunBuiltin3) };
    (self_.fun)(a[0], a[1], a[2])
}

fn mk0(f: BuiltinFn0) -> Obj {
    let o = crate::malloc::new_obj::<ObjFunBuiltin0>().expect("fun0");
    unsafe {
        (*o).base.type_ = &TF0 as *const ObjType;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltin0 as *const ())
    }
}
fn mk1(f: BuiltinFn1) -> Obj {
    let o = crate::malloc::new_obj::<ObjFunBuiltin1>().expect("fun1");
    unsafe {
        (*o).base.type_ = &TF1 as *const ObjType;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltin1 as *const ())
    }
}
fn mk2(f: BuiltinFn2) -> Obj {
    let o = crate::malloc::new_obj::<ObjFunBuiltin2>().expect("fun2");
    unsafe {
        (*o).base.type_ = &TF2 as *const ObjType;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltin2 as *const ())
    }
}
fn mk3(f: BuiltinFn3) -> Obj {
    let o = crate::malloc::new_obj::<ObjFunBuiltin3>().expect("fun3");
    unsafe {
        (*o).base.type_ = &TF3 as *const ObjType;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltin3 as *const ())
    }
}

fn object_make_new(type_in: &ObjType, n_args: usize, n_kw: usize, _args: &[Obj]) -> Obj {
    argcheck::check_num(n_args, n_kw, 0, 0, false);
    let type_static: &'static ObjType = unsafe { &*(type_in as *const ObjType) };
    let o = obj::malloc_helper(core::mem::size_of::<ObjObject>(), type_static) as *mut ObjObject;
    obj::from_ptr(o as *const ObjObject as *const ())
}

fn object___init__(_self: Obj) -> Obj {
    obj::CONST_NONE
}

fn object___new__(_cls: Obj) -> Obj {
    raise::raise(MpRaise::RuntimeError("object.__new__"));
}

fn object___setattr__(_self_in: Obj, _attr: Obj, _value: Obj) -> Obj {
    raise::raise(MpRaise::RuntimeError("object.__setattr__"));
}

fn object___delattr__(_self_in: Obj, _attr: Obj) -> Obj {
    raise::raise(MpRaise::RuntimeError("object.__delattr__"));
}

static mut OBJECT_SLOTS: [*const (); 2] = [object_make_new as *const (), core::ptr::null()];
static mut TYPE_OBJECT: ObjType = ObjType {
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
    slot_index_protocol: 0,
    slot_index_parent: 0,
    slot_index_locals_dict: 2,
    slots: unsafe { OBJECT_SLOTS.as_ptr() },
};

static INIT: std::sync::OnceLock<()> = std::sync::OnceLock::new();

fn init_type() {
    INIT.get_or_init(|| {
        let mut table = Vec::new();
        if mpconfig::CPYTHON_COMPAT {
            table.push(MapElem {
                key: obj::new_qstr(qstr::from_str("__init__")),
                value: mk1(object___init__),
            });
            table.push(MapElem {
                key: obj::new_qstr(qstr::from_str("__new__")),
                value: mk1(object___new__),
            });
        }
        if mpconfig::PY_DELATTR_SETATTR {
            table.push(MapElem {
                key: obj::new_qstr(qstr::from_str("__setattr__")),
                value: mk3(object___setattr__),
            });
            table.push(MapElem {
                key: obj::new_qstr(qstr::from_str("__delattr__")),
                value: mk2(object___delattr__),
            });
        }
        if !table.is_empty() {
            let ptr = obj::malloc_helper(core::mem::size_of::<ObjDict>(), objdict::type_dict()) as *mut ObjDict;
            unsafe {
                map::init_fixed_table(&mut (*ptr).map, table);
                OBJECT_SLOTS[1] = obj::from_ptr(ptr as *const ObjDict as *const ()).0 as *const ();
            }
        }
        unsafe {
            TYPE_OBJECT.name = qstr::from_str("object");
        }
    });
}

pub fn type_object() -> &'static ObjType {
    init_type();
    unsafe { &TYPE_OBJECT }
}
