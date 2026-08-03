//! rewrite of py/modmath.c
// symmetry: done

use crate::bc::ModuleContext;
use crate::map::{self, MapElem};
use crate::malloc;
use crate::objfloat::MpFloat;
use crate::mpconfig;
use crate::obj::{self, Obj, ObjBase, ObjType, TYPE_FLAG_BUILTIN_FUN};
use crate::objdict;
use crate::objfloat;
use crate::objmodule;
use crate::qstr;
use crate::raise::{self, MpRaise};

type BuiltinFn1 = fn(Obj) -> Obj;
type BuiltinFn2 = fn(Obj, Obj) -> Obj;

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

static mut F1: [*const (); 1] = [call1 as *const ()];
static mut F2: [*const (); 1] = [call2 as *const ()];
static T1: ObjType = ObjType {
    base: ObjBase { type_: core::ptr::null() },
    flags: TYPE_FLAG_BUILTIN_FUN,
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
static T2: ObjType = ObjType {
    base: ObjBase { type_: core::ptr::null() },
    flags: TYPE_FLAG_BUILTIN_FUN,
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
    slots: unsafe { F2.as_ptr() },
};

fn call1(s: Obj, n: usize, k: usize, a: &[Obj]) -> Obj {
    crate::argcheck::check_num(n, k, 1, 1, false);
    let self_ = unsafe { &*(obj::as_ptr(s) as *const ObjFunBuiltin1) };
    (self_.fun)(a[0])
}

fn call2(s: Obj, n: usize, k: usize, a: &[Obj]) -> Obj {
    crate::argcheck::check_num(n, k, 2, 2, false);
    let self_ = unsafe { &*(obj::as_ptr(s) as *const ObjFunBuiltin2) };
    (self_.fun)(a[0], a[1])
}

fn mk1(f: BuiltinFn1) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltin1>().expect("math fun1");
    unsafe {
        (*o).base.type_ = &T1 as *const ObjType;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltin1 as *const ())
    }
}

fn mk2(f: BuiltinFn2) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltin2>().expect("math fun2");
    unsafe {
        (*o).base.type_ = &T2 as *const ObjType;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltin2 as *const ())
    }
}

fn math_error() -> ! {
    raise::raise(MpRaise::ValueError("math domain error"));
}

fn generic1(x: Obj, f: fn(MpFloat) -> MpFloat) -> Obj {
    let v = objfloat::float_get(x);
    let ans = f(v);
    if (ans.is_nan() && !v.is_nan()) || (ans.is_infinite() && !v.is_infinite()) {
        math_error();
    }
    objfloat::new_float(ans)
}

fn sqrt(x: Obj) -> Obj {
    generic1(x, |v| v.sqrt())
}
fn sin(x: Obj) -> Obj {
    generic1(x, |v| v.sin())
}
fn cos(x: Obj) -> Obj {
    generic1(x, |v| v.cos())
}
fn fabs(x: Obj) -> Obj {
    generic1(x, |v| v.abs())
}
fn floor(x: Obj) -> Obj {
    obj::new_int(objfloat::float_get(x).floor() as i64 as crate::obj::Int)
}
fn ceil(x: Obj) -> Obj {
    obj::new_int(objfloat::float_get(x).ceil() as i64 as crate::obj::Int)
}
fn pow(x: Obj, y: Obj) -> Obj {
    let v = objfloat::float_get(x);
    let yv = objfloat::float_get(y);
    let ans = v.powf(yv);
    if (ans.is_nan() && !v.is_nan() && !yv.is_nan()) || (ans.is_infinite() && !v.is_infinite() && !yv.is_infinite()) {
        math_error();
    }
    objfloat::new_float(ans)
}

pub fn init_module() -> Obj {
    if !(mpconfig::PY_BUILTINS_FLOAT && mpconfig::PY_MATH) {
        return obj::OBJ_NULL;
    }
    let mut table = vec![
        MapElem { key: obj::new_qstr(qstr::from_str("__name__")), value: obj::new_qstr(qstr::from_str("math")) },
        MapElem { key: obj::new_qstr(qstr::from_str("sqrt")), value: mk1(sqrt) },
        MapElem { key: obj::new_qstr(qstr::from_str("sin")), value: mk1(sin) },
        MapElem { key: obj::new_qstr(qstr::from_str("cos")), value: mk1(cos) },
        MapElem { key: obj::new_qstr(qstr::from_str("fabs")), value: mk1(fabs) },
        MapElem { key: obj::new_qstr(qstr::from_str("floor")), value: mk1(floor) },
        MapElem { key: obj::new_qstr(qstr::from_str("ceil")), value: mk1(ceil) },
        MapElem { key: obj::new_qstr(qstr::from_str("pow")), value: mk2(pow) },
    ];
    if mpconfig::PY_MATH_CONSTANTS {
        table.push(MapElem { key: obj::new_qstr(qstr::from_str("pi")), value: objfloat::new_float(core::f64::consts::PI) });
        table.push(MapElem { key: obj::new_qstr(qstr::from_str("e")), value: objfloat::new_float(core::f64::consts::E) });
    }
    let ctx = malloc::new_obj::<ModuleContext>().expect("math module");
    let dict = objdict::new_dict(table.len());
    unsafe {
        map::init_fixed_table(&mut (*objdict::dict_ptr(dict)).map, table);
        (*ctx).module.base.type_ = objmodule::type_module();
        (*ctx).module.globals = objdict::dict_ptr(dict);
        (*ctx).constants = Default::default();
    }
    let module = obj::from_ptr(ctx as *const ModuleContext as *const ());
    objmodule::register_builtin_module(qstr::from_str("math"), module);
    module
}
