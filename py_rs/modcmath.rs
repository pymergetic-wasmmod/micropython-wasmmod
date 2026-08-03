//! rewrite of py/modcmath.c
// symmetry: done

use crate::bc::ModuleContext;
use crate::map::{self, MapElem};
use crate::malloc;
use crate::mpconfig;
use crate::obj::{self, Obj, ObjBase, ObjType, TYPE_FLAG_BUILTIN_FUN};
use crate::objcomplex;
use crate::objdict;
use crate::objfloat;
use crate::objmodule;
use crate::objtuple;
use crate::qstr;

type BuiltinFn1 = fn(Obj) -> Obj;

#[repr(C)]
struct ObjFunBuiltin1 {
    base: ObjBase,
    fun: BuiltinFn1,
}

static mut F1: [*const (); 1] = [call1 as *const ()];
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

fn call1(s: Obj, n: usize, k: usize, a: &[Obj]) -> Obj {
    crate::argcheck::check_num(n, k, 1, 1, false);
    let self_ = unsafe { &*(obj::as_ptr(s) as *const ObjFunBuiltin1) };
    (self_.fun)(a[0])
}

fn mk1(f: BuiltinFn1) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltin1>().expect("cmath fun1");
    unsafe {
        (*o).base.type_ = &T1 as *const ObjType;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltin1 as *const ())
    }
}

fn cmath_phase(z: Obj) -> Obj {
    let mut re = 0.0;
    let mut im = 0.0;
    objcomplex::complex_get(z, &mut re, &mut im);
    objfloat::new_float(im.atan2(re))
}

fn cmath_polar(z: Obj) -> Obj {
    let mut re = 0.0;
    let mut im = 0.0;
    objcomplex::complex_get(z, &mut re, &mut im);
    objtuple::new_tuple(
        2,
        Some(&[
            objfloat::new_float((re * re + im * im).sqrt()),
            objfloat::new_float(im.atan2(re)),
        ]),
    )
}

pub fn init_module() -> Obj {
    if !(mpconfig::PY_BUILTINS_FLOAT && mpconfig::PY_BUILTINS_COMPLEX && mpconfig::PY_CMATH) {
        return obj::OBJ_NULL;
    }
    let table = vec![
        MapElem { key: obj::new_qstr(qstr::from_str("__name__")), value: obj::new_qstr(qstr::from_str("cmath")) },
        MapElem { key: obj::new_qstr(qstr::from_str("phase")), value: mk1(cmath_phase) },
        MapElem { key: obj::new_qstr(qstr::from_str("polar")), value: mk1(cmath_polar) },
    ];
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
