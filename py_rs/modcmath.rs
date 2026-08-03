//! rewrite of py/modcmath.c
// symmetry: done

use crate::bc::ModuleContext;
use crate::malloc;
use crate::map::{self, MapElem};
use crate::mpconfig;
use crate::obj::{self, Obj, ObjBase, ObjType, TYPE_FLAG_BUILTIN_FUN};
use crate::objcomplex;
use crate::objdict;
use crate::objfloat::{self, MpFloat};
use crate::objmodule;
use crate::objtuple;
use crate::qstr;

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
    base: ObjBase {
        type_: core::ptr::null(),
    },
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
    base: ObjBase {
        type_: core::ptr::null(),
    },
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
    let o = malloc::new_obj::<ObjFunBuiltin1>().expect("cmath fun1");
    unsafe {
        (*o).base.type_ = &T1 as *const ObjType;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltin1 as *const ())
    }
}

fn mk2(f: BuiltinFn2) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltin2>().expect("cmath fun2");
    unsafe {
        (*o).base.type_ = &T2 as *const ObjType;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltin2 as *const ())
    }
}

fn me(name: &str, value: Obj) -> MapElem {
    MapElem {
        key: obj::new_qstr(qstr::from_str(name)),
        value,
    }
}

fn cmath_phase(z: Obj) -> Obj {
    let mut re = 0.0;
    let mut im = 0.0;
    objcomplex::get_complex(z, &mut re, &mut im);
    objfloat::new_float(im.atan2(re))
}

fn cmath_polar(z: Obj) -> Obj {
    let mut re = 0.0;
    let mut im = 0.0;
    objcomplex::get_complex(z, &mut re, &mut im);
    objtuple::new_tuple(
        2,
        Some(&[
            objfloat::new_float((re * re + im * im).sqrt()),
            objfloat::new_float(im.atan2(re)),
        ]),
    )
}

fn cmath_rect(r_obj: Obj, phi_obj: Obj) -> Obj {
    let r = objfloat::get_float(r_obj);
    let phi = objfloat::get_float(phi_obj);
    objcomplex::new_complex(r * phi.cos(), r * phi.sin())
}

fn cmath_exp(z: Obj) -> Obj {
    let mut re = 0.0;
    let mut im = 0.0;
    objcomplex::get_complex(z, &mut re, &mut im);
    let exp_re = re.exp();
    objcomplex::new_complex(exp_re * im.cos(), exp_re * im.sin())
}

fn cmath_log(z: Obj) -> Obj {
    let mut re = 0.0;
    let mut im = 0.0;
    objcomplex::get_complex(z, &mut re, &mut im);
    objcomplex::new_complex(0.5 * (re * re + im * im).ln(), im.atan2(re))
}

fn cmath_log10(z: Obj) -> Obj {
    let mut re = 0.0;
    let mut im = 0.0;
    objcomplex::get_complex(z, &mut re, &mut im);
    // 0.4342944819032518 == log10(e)
    objcomplex::new_complex(
        0.5 * (re * re + im * im).log10(),
        0.4342944819032518 * im.atan2(re),
    )
}

fn cmath_sqrt(z: Obj) -> Obj {
    let mut re = 0.0;
    let mut im = 0.0;
    objcomplex::get_complex(z, &mut re, &mut im);
    let sqrt_abs = (re * re + im * im).powf(0.25);
    let theta = 0.5 * im.atan2(re);
    objcomplex::new_complex(sqrt_abs * theta.cos(), sqrt_abs * theta.sin())
}

fn cmath_cos(z: Obj) -> Obj {
    let mut re = 0.0;
    let mut im = 0.0;
    objcomplex::get_complex(z, &mut re, &mut im);
    objcomplex::new_complex(re.cos() * im.cosh(), -re.sin() * im.sinh())
}

fn cmath_sin(z: Obj) -> Obj {
    let mut re = 0.0;
    let mut im = 0.0;
    objcomplex::get_complex(z, &mut re, &mut im);
    objcomplex::new_complex(re.sin() * im.cosh(), re.cos() * im.sinh())
}

pub fn init_module() -> Obj {
    if !(mpconfig::PY_BUILTINS_FLOAT && mpconfig::PY_BUILTINS_COMPLEX && mpconfig::PY_CMATH) {
        return obj::OBJ_NULL;
    }
    let mut table = vec![
        me("__name__", obj::new_qstr(qstr::from_str("cmath"))),
        me("e", objfloat::new_float(core::f64::consts::E as MpFloat)),
        me("pi", objfloat::new_float(core::f64::consts::PI as MpFloat)),
        me("phase", mk1(cmath_phase)),
        me("polar", mk1(cmath_polar)),
        me("rect", mk2(cmath_rect)),
        me("exp", mk1(cmath_exp)),
        me("log", mk1(cmath_log)),
        me("sqrt", mk1(cmath_sqrt)),
        me("cos", mk1(cmath_cos)),
        me("sin", mk1(cmath_sin)),
    ];
    if mpconfig::PY_MATH_SPECIAL_FUNCTIONS {
        table.push(me("log10", mk1(cmath_log10)));
    }
    let ctx = malloc::new_obj::<ModuleContext>().expect("cmath module");
    let dict = objdict::new_dict(table.len());
    unsafe {
        map::init_fixed_table(&mut (*objdict::dict_ptr(dict)).map, table);
        (*ctx).module.base.type_ = objmodule::type_module();
        (*ctx).module.globals = objdict::dict_ptr(dict);
        (*ctx).constants = Default::default();
    }
    let module = obj::from_ptr(ctx as *const ModuleContext as *const ());
    objmodule::register_builtin_module(qstr::from_str("cmath"), module);
    module
}
