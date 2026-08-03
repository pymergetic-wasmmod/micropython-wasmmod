//! rewrite of py/modmath.c
// symmetry: done

use crate::bc::ModuleContext;
use crate::malloc;
use crate::map::{self, LookupKind, Map, MapElem};
use crate::mpconfig;
use crate::obj::{self, Obj, ObjBase, ObjType, TYPE_FLAG_BUILTIN_FUN};
use crate::objdict;
use crate::objfloat::MpFloat;
use crate::objfloat::{self, get_float};
use crate::objmodule;
use crate::objtuple;
use crate::qstr;
use crate::raise::{self, MpRaise};
use crate::runtime;
use crate::runtime0::BinaryOp;

type BuiltinFn1 = fn(Obj) -> Obj;
type BuiltinFn2 = fn(Obj, Obj) -> Obj;
type BuiltinFnVar = fn(usize, &[Obj]) -> Obj;
type BuiltinFnKw = fn(usize, &[Obj], &mut Map) -> Obj;

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
struct ObjFunBuiltinVar {
    base: ObjBase,
    min_args: u8,
    max_args: u8,
    fun: BuiltinFnVar,
}
#[repr(C)]
struct ObjFunBuiltinKw {
    base: ObjBase,
    min_args: u8,
    max_args: u8,
    fun: BuiltinFnKw,
}

static mut F1: [*const (); 1] = [call1 as *const ()];
static mut F2: [*const (); 1] = [call2 as *const ()];
static mut FV: [*const (); 1] = [callv as *const ()];
static mut FKW: [*const (); 1] = [call_kw as *const ()];
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
static TV: ObjType = ObjType {
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
    slots: unsafe { FV.as_ptr() },
};
static TKW: ObjType = ObjType {
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
    slots: unsafe { FKW.as_ptr() },
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
fn callv(s: Obj, n: usize, k: usize, a: &[Obj]) -> Obj {
    let self_ = unsafe { &*(obj::as_ptr(s) as *const ObjFunBuiltinVar) };
    crate::argcheck::check_num(n, k, self_.min_args as usize, self_.max_args as usize, false);
    (self_.fun)(n, a)
}
fn call_kw(s: Obj, n: usize, k: usize, a: &[Obj]) -> Obj {
    let self_ = unsafe { &*(obj::as_ptr(s) as *const ObjFunBuiltinKw) };
    crate::argcheck::check_num(n, k, self_.min_args as usize, self_.max_args as usize, true);
    let mut kw = Map::default();
    map::init(&mut kw, k);
    for i in 0..k {
        if let Some(slot) = map::lookup(&mut kw, a[n + i * 2], LookupKind::AddIfNotFound) {
            slot.value = a[n + i * 2 + 1];
        }
    }
    (self_.fun)(n, &a[..n], &mut kw)
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
fn mkv(min_args: u8, max_args: u8, f: BuiltinFnVar) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltinVar>().expect("math funv");
    unsafe {
        (*o).base.type_ = &TV as *const ObjType;
        (*o).min_args = min_args;
        (*o).max_args = max_args;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltinVar as *const ())
    }
}
fn mk_kw(min_args: u8, max_args: u8, f: BuiltinFnKw) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltinKw>().expect("math fun kw");
    unsafe {
        (*o).base.type_ = &TKW as *const ObjType;
        (*o).min_args = min_args;
        (*o).max_args = max_args;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltinKw as *const ())
    }
}

fn math_error() -> ! {
    raise::raise(MpRaise::ValueError("math domain error"));
}

fn generic1(x: Obj, f: fn(MpFloat) -> MpFloat) -> Obj {
    let v = get_float(x);
    let ans = f(v);
    if (ans.is_nan() && !v.is_nan()) || (ans.is_infinite() && !v.is_infinite()) {
        math_error();
    }
    objfloat::new_float(ans)
}

fn generic2(x: Obj, y: Obj, f: fn(MpFloat, MpFloat) -> MpFloat) -> Obj {
    let v = get_float(x);
    let yv = get_float(y);
    let ans = f(v, yv);
    if (ans.is_nan() && !v.is_nan() && !yv.is_nan())
        || (ans.is_infinite() && !v.is_infinite() && !yv.is_infinite())
    {
        math_error();
    }
    objfloat::new_float(ans)
}

fn sqrt(x: Obj) -> Obj {
    generic1(x, |v| v.sqrt())
}
fn exp(x: Obj) -> Obj {
    generic1(x, |v| v.exp())
}
fn expm1(x: Obj) -> Obj {
    generic1(x, |v| v.exp_m1())
}
fn log2(x: Obj) -> Obj {
    generic1(x, |v| {
        if v <= 0.0 {
            return f64::NAN;
        }
        v.log2()
    })
}
fn log10(x: Obj) -> Obj {
    generic1(x, |v| {
        if v <= 0.0 {
            return f64::NAN;
        }
        v.log10()
    })
}
fn cosh(x: Obj) -> Obj {
    generic1(x, |v| v.cosh())
}
fn sinh(x: Obj) -> Obj {
    generic1(x, |v| v.sinh())
}
fn tanh(x: Obj) -> Obj {
    generic1(x, |v| v.tanh())
}
fn acosh(x: Obj) -> Obj {
    generic1(x, |v| v.acosh())
}
fn asinh(x: Obj) -> Obj {
    generic1(x, |v| v.asinh())
}
fn atanh(x: Obj) -> Obj {
    generic1(x, |v| v.atanh())
}
fn cos(x: Obj) -> Obj {
    generic1(x, |v| v.cos())
}
fn sin(x: Obj) -> Obj {
    generic1(x, |v| v.sin())
}
fn tan(x: Obj) -> Obj {
    generic1(x, |v| v.tan())
}
fn acos(x: Obj) -> Obj {
    generic1(x, |v| v.acos())
}
fn asin(x: Obj) -> Obj {
    generic1(x, |v| v.asin())
}
fn atan(x: Obj) -> Obj {
    generic1(x, |v| v.atan())
}
fn atan2(y: Obj, x: Obj) -> Obj {
    generic2(y, x, |yv, xv| yv.atan2(xv))
}
fn fabs(x: Obj) -> Obj {
    generic1(x, |v| v.abs())
}
fn floor(x: Obj) -> Obj {
    obj::new_int(get_float(x).floor() as i64 as crate::obj::Int)
}
fn ceil(x: Obj) -> Obj {
    obj::new_int(get_float(x).ceil() as i64 as crate::obj::Int)
}
fn trunc(x: Obj) -> Obj {
    obj::new_int(get_float(x).trunc() as i64 as crate::obj::Int)
}
fn pow(x: Obj, y: Obj) -> Obj {
    generic2(x, y, |v, yv| v.powf(yv))
}
fn fmod(x: Obj, y: Obj) -> Obj {
    generic2(x, y, |v, yv| v % yv)
}
fn copysign(x: Obj, y: Obj) -> Obj {
    objfloat::new_float(get_float(x).copysign(get_float(y)))
}
fn ldexp(x: Obj, exp: Obj) -> Obj {
    let e = obj::get_int(exp) as i32;
    objfloat::new_float(get_float(x) * 2f64.powi(e))
}

fn isfinite(x: Obj) -> Obj {
    obj::new_bool(get_float(x).is_finite())
}
fn isinf(x: Obj) -> Obj {
    obj::new_bool(get_float(x).is_infinite())
}
fn isnan(x: Obj) -> Obj {
    obj::new_bool(get_float(x).is_nan())
}

fn log(n_args: usize, args: &[Obj]) -> Obj {
    let x = get_float(args[0]);
    if x <= 0.0 {
        math_error();
    }
    let l = x.ln();
    if n_args == 1 {
        return objfloat::new_float(l);
    }
    let base = get_float(args[1]);
    if base <= 0.0 {
        math_error();
    } else if base == 1.0 {
        raise::raise(MpRaise::ZeroDivisionError);
    }
    objfloat::new_float(l / base.ln())
}

fn frexp(x: Obj) -> Obj {
    let (frac, exp) = frexp_f64(get_float(x));
    objtuple::new_tuple(
        2,
        Some(&[objfloat::new_float(frac), obj::new_int(exp as i64 as crate::obj::Int)]),
    )
}

fn frexp_f64(x: MpFloat) -> (MpFloat, i32) {
    if x == 0.0 || !x.is_finite() {
        return (x, 0);
    }
    let bits = x.to_bits();
    let mut e = ((bits >> 52) & 0x7ff) as i32;
    let mut mbits = bits & ((1u64 << 52) - 1);
    if e == 0 {
        // subnormal: normalize
        e = 1;
        while mbits & (1u64 << 52) == 0 {
            mbits <<= 1;
            e -= 1;
        }
        mbits &= (1u64 << 52) - 1;
    }
    let exp = e - 1022;
    let frac_bits = (bits & (1u64 << 63)) | (0x3feu64 << 52) | mbits;
    (f64::from_bits(frac_bits), exp)
}

fn modf(x_obj: Obj) -> Obj {
    let x = get_float(x_obj);
    let int_part = x.trunc();
    let mut frac = x - int_part;
    if mpconfig::PY_MATH_MODF_FIX_NEGZERO && frac == 0.0 {
        frac = frac.copysign(x);
    }
    objtuple::new_tuple(
        2,
        Some(&[objfloat::new_float(frac), objfloat::new_float(int_part)]),
    )
}

fn radians(x: Obj) -> Obj {
    objfloat::new_float(get_float(x) * (core::f64::consts::PI / 180.0))
}
fn degrees(x: Obj) -> Obj {
    objfloat::new_float(get_float(x) * (180.0 / core::f64::consts::PI))
}

fn kw_float(kw: &mut Map, name: &str) -> Option<MpFloat> {
    let key = obj::new_qstr(qstr::from_str(name));
    map::lookup(kw, key, LookupKind::Lookup).map(|e| get_float(e.value))
}

fn isclose(n_args: usize, args: &[Obj], kw: &mut Map) -> Obj {
    let _ = n_args;
    let a = get_float(args[0]);
    let b = get_float(args[1]);
    let rel_tol = kw_float(kw, "rel_tol").unwrap_or(1e-9);
    let abs_tol = kw_float(kw, "abs_tol").unwrap_or(0.0);
    if rel_tol < 0.0 || abs_tol < 0.0 {
        math_error();
    }
    if a == b {
        return obj::CONST_TRUE;
    }
    let difference = (a - b).abs();
    if difference.is_infinite() {
        return obj::CONST_FALSE;
    }
    if difference <= abs_tol
        || difference <= (rel_tol * a).abs()
        || difference <= (rel_tol * b).abs()
    {
        obj::CONST_TRUE
    } else {
        obj::CONST_FALSE
    }
}

fn erf(x: Obj) -> Obj {
    generic1(x, libm_erf)
}
fn erfc(x: Obj) -> Obj {
    generic1(x, |v| 1.0 - libm_erf(v))
}
fn gamma(x: Obj) -> Obj {
    generic1(x, libm_tgamma)
}
fn lgamma(x: Obj) -> Obj {
    generic1(x, libm_lgamma)
}

fn libm_erf(x: MpFloat) -> MpFloat {
    let sign = if x < 0.0 { -1.0 } else { 1.0 };
    let x = x.abs();
    let t = 1.0 / (1.0 + 0.3275911 * x);
    let y = 1.0
        - (((((1.061405429 * t - 1.453152027) * t) + 1.421413741) * t - 0.284496736) * t
            + 0.254829592)
            * t
            * (-x * x).exp();
    sign * y
}

fn libm_tgamma(x: MpFloat) -> MpFloat {
    if x < 0.5 {
        return core::f64::consts::PI / ((core::f64::consts::PI * x).sin() * libm_tgamma(1.0 - x));
    }
    const G: f64 = 5.0;
    const C: [f64; 7] = [
        1.000000000190015,
        76.18009172947146,
        -86.50532032941677,
        24.01409824083091,
        -1.231739572450155,
        0.1208650973866179e-2,
        -0.539841384818556e-5,
    ];
    let mut y = x;
    let mut tmp = x + G + 0.5;
    tmp = (tmp.ln() * (x + 0.5)) - tmp;
    let mut ser = C[0];
    for c in C.iter().skip(1) {
        y += 1.0;
        ser += c / y;
    }
    (tmp + (2.5066282746310005 * ser / x).ln()).exp()
}

fn libm_lgamma(x: MpFloat) -> MpFloat {
    libm_tgamma(x).abs().ln()
}

fn factorial(x: Obj) -> Obj {
    let max = obj::get_int(x);
    if max < 0 {
        raise::raise(MpRaise::ValueError("negative factorial"));
    }
    if max == 0 {
        return obj::new_small_int(1);
    }
    let mut acc = obj::new_small_int(1);
    for i in 1..=max {
        acc = runtime::binary_op_obj(BinaryOp::Multiply, acc, obj::new_small_int(i));
    }
    acc
}

fn me(name: &str, val: Obj) -> MapElem {
    MapElem {
        key: obj::new_qstr(qstr::from_str(name)),
        value: val,
    }
}

pub fn init_module() -> Obj {
    if !(mpconfig::PY_BUILTINS_FLOAT && mpconfig::PY_MATH) {
        return obj::OBJ_NULL;
    }
    let mut table = vec![
        me("__name__", obj::new_qstr(qstr::from_str("math"))),
        me("sqrt", mk1(sqrt)),
        me("pow", mk2(pow)),
        me("exp", mk1(exp)),
        me("log", mkv(1, 2, log)),
        me("cos", mk1(cos)),
        me("sin", mk1(sin)),
        me("tan", mk1(tan)),
        me("acos", mk1(acos)),
        me("asin", mk1(asin)),
        me("atan", mk1(atan)),
        me("atan2", mk2(atan2)),
        me("ceil", mk1(ceil)),
        me("copysign", mk2(copysign)),
        me("fabs", mk1(fabs)),
        me("floor", mk1(floor)),
        me("fmod", mk2(fmod)),
        me("frexp", mk1(frexp)),
        me("ldexp", mk2(ldexp)),
        me("modf", mk1(modf)),
        me("isfinite", mk1(isfinite)),
        me("isinf", mk1(isinf)),
        me("isnan", mk1(isnan)),
        me("trunc", mk1(trunc)),
        me("radians", mk1(radians)),
        me("degrees", mk1(degrees)),
    ];
    if mpconfig::PY_MATH_ISCLOSE {
        table.push(me("isclose", mk_kw(2, 2, isclose)));
    }
    if mpconfig::PY_MATH_SPECIAL_FUNCTIONS {
        table.push(me("expm1", mk1(expm1)));
        table.push(me("log2", mk1(log2)));
        table.push(me("log10", mk1(log10)));
        table.push(me("cosh", mk1(cosh)));
        table.push(me("sinh", mk1(sinh)));
        table.push(me("tanh", mk1(tanh)));
        table.push(me("acosh", mk1(acosh)));
        table.push(me("asinh", mk1(asinh)));
        table.push(me("atanh", mk1(atanh)));
        table.push(me("erf", mk1(erf)));
        table.push(me("erfc", mk1(erfc)));
        table.push(me("gamma", mk1(gamma)));
        table.push(me("lgamma", mk1(lgamma)));
    }
    if mpconfig::PY_MATH_FACTORIAL {
        table.push(me("factorial", mk1(factorial)));
    }
    if mpconfig::PY_MATH_CONSTANTS {
        table.push(me("e", objfloat::new_float(core::f64::consts::E)));
        table.push(me("pi", objfloat::new_float(core::f64::consts::PI)));
        table.push(me("tau", objfloat::new_float(core::f64::consts::TAU)));
        table.push(me("inf", objfloat::new_float(f64::INFINITY)));
        table.push(me("nan", objfloat::new_float(f64::NAN)));
    } else {
        // C always exports e/pi even when CONSTANTS is off for tau/inf/nan.
        table.push(me("e", objfloat::new_float(core::f64::consts::E)));
        table.push(me("pi", objfloat::new_float(core::f64::consts::PI)));
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
