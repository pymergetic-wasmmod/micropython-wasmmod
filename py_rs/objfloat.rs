//! rewrite of py/objfloat.c + py/objfloat.h
// symmetry: done

use core::mem::size_of;

use crate::argcheck;
use crate::malloc;
use crate::misc;
use crate::mpconfig;
use crate::mpprint::{self, Print, PrintKind, PF_FLAG_ALWAYS_DECIMAL};
use crate::obj::{self, BufferInfo, Obj, ObjBase, ObjType, TYPE_FLAG_EQ_CHECKS_OTHER_TYPE, TYPE_FLAG_EQ_NOT_REFLEXIVE};
use crate::objcomplex;
use crate::objint_mpz;
use crate::parsenum;
use crate::raise::{self, MpRaise};
use crate::runtime;
use crate::runtime0::{BinaryOp, UnaryOp};

/// Native float type (`mp_float_t`) — follows `MICROPY_FLOAT_IMPL`.
pub type MpFloat = f64;

const FLOAT_ZERO: MpFloat = 0.0;

#[repr(C)]
pub struct ObjFloat {
    pub base: ObjBase,
    pub value: MpFloat,
}

static mut FLOAT_SLOTS: [*const (); 4] = [
    float_make_new as *const (),
    float_print as *const (),
    float_unary_op as *const (),
    float_binary_op as *const (),
];

static TYPE: ObjType = ObjType {
    base: ObjBase { type_: core::ptr::null() },
    flags: TYPE_FLAG_EQ_NOT_REFLEXIVE | TYPE_FLAG_EQ_CHECKS_OTHER_TYPE,
    name: 0,
    slot_index_make_new: 1,
    slot_index_print: 2,
    slot_index_unary_op: 3,
    slot_index_binary_op: 4,
    slot_index_call: 0,
    slot_index_attr: 0,
    slot_index_subscr: 0,
    slot_index_iter: 0,
    slot_index_buffer: 0,
    slot_index_protocol: 0,
    slot_index_parent: 0,
    slot_index_locals_dict: 0,
    slots: unsafe { FLOAT_SLOTS.as_ptr() },
};

pub fn type_float() -> &'static ObjType {
    &TYPE
}

pub fn is_float(o: Obj) -> bool {
    obj::is_exact_type(o, type_float())
}

fn new_boxed(value: MpFloat) -> Obj {
    let o = malloc::new_obj::<ObjFloat>().expect("objfloat alloc");
    unsafe {
        (*o).base.type_ = type_float() as *const ObjType;
        (*o).value = value;
        obj::from_ptr(o as *const ObjFloat as *const ())
    }
}

/// `mp_obj_new_float`
pub fn new_float(value: MpFloat) -> Obj {
    new_boxed(value)
}

pub fn new_float_from_f(value: f32) -> Obj {
    new_boxed(f64::from(value))
}

pub fn new_float_from_d(value: f64) -> Obj {
    new_boxed(value)
}

/// `mp_obj_float_get`
pub fn float_get(o: Obj) -> MpFloat {
    if !is_float(o) {
        raise::raise(MpRaise::TypeError("can't convert to float"));
    }
    unsafe { (*(obj::as_ptr(o) as *const ObjFloat)).value }
}

pub fn get_float_to_f(o: Obj) -> f32 {
    float_get(o) as f32
}

pub fn get_float_to_d(o: Obj) -> f64 {
    float_get(o)
}

/// `mp_obj_get_float_maybe`
pub fn get_float_maybe(o: Obj, out: &mut MpFloat) -> bool {
    if o == obj::CONST_FALSE {
        *out = FLOAT_ZERO;
    } else if o == obj::CONST_TRUE {
        *out = 1.0;
    } else if obj::is_small_int(o) {
        *out = obj::small_int_value(o) as MpFloat;
    } else if obj::is_exact_type(o, crate::objint::type_int()) {
        unsafe {
            *out = crate::mpz::as_float(&(*(obj::as_ptr(o) as *const objint_mpz::ObjInt)).mpz);
        }
    } else if is_float(o) {
        *out = float_get(o);
    } else {
        let converted = runtime::unary_op_obj(UnaryOp::FloatMaybe, o);
        if converted == obj::OBJ_NULL || !is_float(converted) {
            return false;
        }
        *out = float_get(converted);
    }
    true
}

/// `mp_obj_get_float`
pub fn get_float(o: Obj) -> MpFloat {
    let mut val = FLOAT_ZERO;
    if !get_float_maybe(o, &mut val) {
        raise::raise(MpRaise::TypeError("can't convert to float"));
    }
    val
}

pub fn mp_float_t() -> MpFloat {
    debug_assert!(mpconfig::PY_BUILTINS_FLOAT);
    FLOAT_ZERO
}

/// `mp_float_hash`
pub fn float_hash(src: MpFloat) -> obj::Int {
    if mpconfig::FLOAT_HIGH_QUALITY_HASH {
        float_hash_hq(src)
    } else {
        src as obj::Int
    }
}

fn float_hash_hq(src: MpFloat) -> obj::Int {
    let bits = src.to_bits();
    let exp = ((bits >> misc::FLOAT_FRAC_BITS as u64) & ((1u64 << misc::FLOAT_EXP_BITS as u64) - 1)) as i32;
    let adj_exp = exp - misc::FLOAT_EXP_BIAS as i32;
    let frac = bits & ((1u64 << misc::FLOAT_FRAC_BITS as u64) - 1);
    let mut val: obj::Int;
    if adj_exp < 0 {
        val = bits as obj::Int;
    } else {
        let frc = frac | (1u64 << misc::FLOAT_FRAC_BITS as u64);
        if adj_exp <= misc::FLOAT_FRAC_BITS as i32 {
            let shift = misc::FLOAT_FRAC_BITS as i32 - adj_exp;
            val = ((frc >> shift) ^ (frc & ((1u64 << shift) - 1))) as obj::Int;
        } else if (adj_exp as u32) < mpconfig::BITS_PER_BYTE as u32 * size_of::<obj::Int>() as u32 - 1 {
            val = (frc << (adj_exp - misc::FLOAT_FRAC_BITS as i32)) as obj::Int;
        } else {
            val = frc as obj::Int;
        }
    }
    if bits >> 63 != 0 {
        val = -(val as obj::Uint as obj::Int);
    }
    val
}

fn format_float(val: MpFloat) -> String {
    if val.is_nan() {
        return "nan".into();
    }
    if val.is_infinite() {
        return if val > 0.0 { "inf".into() } else { "-inf".into() };
    }
    let mut s = format!("{:.12}", val);
    if PF_FLAG_ALWAYS_DECIMAL != 0 && !s.contains('.') && !s.contains('e') && !s.contains('E') {
        s.push_str(".0");
    }
    if s.contains('.') {
        while s.ends_with('0') {
            s.pop();
        }
        if s.ends_with('.') {
            s.push('0');
        }
    }
    s
}

pub fn float_print(print: &Print, o_in: Obj, _kind: PrintKind) {
    mpprint::print_str(print, &format_float(float_get(o_in)));
}

pub fn float_make_new(_type_in: &ObjType, n_args: usize, n_kw: usize, args: &[Obj]) -> Obj {
    debug_assert!(mpconfig::PY_BUILTINS_FLOAT);
    argcheck::check_num(n_args, n_kw, 0, 1, false);
    match n_args {
        0 => new_float(FLOAT_ZERO),
        _ => {
            let mut bufinfo = BufferInfo::default();
            if obj::get_buffer(args[0], &mut bufinfo, obj::BUFFER_READ) {
                let slice = unsafe { std::slice::from_raw_parts(bufinfo.buf, bufinfo.len) };
                return parsenum::parse_num_float(slice, false, None);
            } else if is_float(args[0]) {
                args[0]
            } else {
                new_float(get_float(args[0]))
            }
        }
    }
}

pub fn float_unary_op(op: UnaryOp, o_in: Obj) -> Obj {
    let val = float_get(o_in);
    match op {
        UnaryOp::Bool => obj::new_bool(val != FLOAT_ZERO),
        UnaryOp::Hash => obj::new_small_int(float_hash(val)),
        UnaryOp::Positive => o_in,
        UnaryOp::Negative => new_float(-val),
        UnaryOp::Abs => {
            if val.is_sign_negative() {
                new_float(-val)
            } else {
                o_in
            }
        }
        _ => obj::OBJ_NULL,
    }
}

pub fn float_binary_op(op: BinaryOp, lhs_in: Obj, rhs_in: Obj) -> Obj {
    let lhs_val = float_get(lhs_in);
    if mpconfig::PY_BUILTINS_COMPLEX && obj::is_exact_type(rhs_in, objcomplex::type_complex()) {
        return objcomplex::complex_binary_op(op, lhs_val, 0.0, rhs_in);
    }
    float_binary_op_val(op, lhs_val, rhs_in)
}

fn float_divmod(x: &mut MpFloat, y: &mut MpFloat) {
    let mut modulo = *x % *y;
    let mut div = (*x - modulo) / *y;
    if modulo == 0.0 {
        modulo = 0.0_f64.copysign(*y);
    } else if (modulo < 0.0) != (*y < 0.0) {
        modulo += *y;
        div -= 1.0;
    }
    let floordiv = if div == 0.0 {
        0.0_f64.copysign(*x / *y)
    } else {
        let mut floordiv = div.floor();
        if div - floordiv > 0.5 {
            floordiv += 1.0;
        }
        floordiv
    };
    *x = floordiv;
    *y = modulo;
}

/// `mp_obj_float_binary_op`
pub fn float_binary_op_val(op: BinaryOp, mut lhs_val: MpFloat, rhs_in: Obj) -> Obj {
    let mut rhs_val = FLOAT_ZERO;
    if !get_float_maybe(rhs_in, &mut rhs_val) {
        return obj::OBJ_NULL;
    }

    match op {
        BinaryOp::Add | BinaryOp::InplaceAdd => lhs_val += rhs_val,
        BinaryOp::Subtract | BinaryOp::InplaceSubtract => lhs_val -= rhs_val,
        BinaryOp::Multiply | BinaryOp::InplaceMultiply => lhs_val *= rhs_val,
        BinaryOp::FloorDivide | BinaryOp::InplaceFloorDivide => {
            if rhs_val == 0.0 {
                raise::raise(MpRaise::ZeroDivisionError);
            }
            float_divmod(&mut lhs_val, &mut rhs_val);
        }
        BinaryOp::TrueDivide | BinaryOp::InplaceTrueDivide => {
            if rhs_val == 0.0 {
                raise::raise(MpRaise::ZeroDivisionError);
            }
            lhs_val /= rhs_val;
        }
        BinaryOp::Modulo | BinaryOp::InplaceModulo => {
            if rhs_val == 0.0 {
                raise::raise(MpRaise::ZeroDivisionError);
            }
            lhs_val %= rhs_val;
            if lhs_val == 0.0 {
                lhs_val = 0.0_f64.copysign(rhs_val);
            } else if (lhs_val < 0.0) != (rhs_val < 0.0) {
                lhs_val += rhs_val;
            }
        }
        BinaryOp::Power | BinaryOp::InplacePower => {
            if lhs_val == 0.0 && rhs_val < 0.0 && !rhs_val.is_infinite() {
                raise::raise(MpRaise::ZeroDivisionError);
            }
            if lhs_val < 0.0 && rhs_val != rhs_val.floor() && !rhs_val.is_nan() {
                if mpconfig::PY_BUILTINS_COMPLEX {
                    return objcomplex::complex_binary_op(BinaryOp::Power, lhs_val, 0.0, rhs_in);
                }
                raise::raise(MpRaise::ValueError("complex values not supported"));
            }
            if mpconfig::PY_MATH_POW_FIX_NAN {
                if lhs_val == 1.0 || rhs_val == 0.0 {
                    lhs_val = 1.0;
                } else if rhs_val.is_nan() {
                    lhs_val = rhs_val;
                } else {
                    lhs_val = lhs_val.powf(rhs_val);
                }
            } else {
                lhs_val = lhs_val.powf(rhs_val);
            }
        }
        BinaryOp::Divmod => {
            if rhs_val == 0.0 {
                raise::raise(MpRaise::ZeroDivisionError);
            }
            float_divmod(&mut lhs_val, &mut rhs_val);
            return crate::objtuple::new_tuple(2, Some(&[new_float(lhs_val), new_float(rhs_val)]));
        }
        BinaryOp::Less => return obj::new_bool(lhs_val < rhs_val),
        BinaryOp::More => return obj::new_bool(lhs_val > rhs_val),
        BinaryOp::Equal => return obj::new_bool(lhs_val == rhs_val),
        BinaryOp::LessEqual => return obj::new_bool(lhs_val <= rhs_val),
        BinaryOp::MoreEqual => return obj::new_bool(lhs_val >= rhs_val),
        _ => return obj::OBJ_NULL,
    }
    new_float(lhs_val)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gc;

    #[test]
    fn float_arithmetic() {
        let _ = gc::init();
        let a = new_float(3.5);
        let b = new_float(2.0);
        let sum = float_binary_op_val(BinaryOp::Add, float_get(a), b);
        assert!((float_get(sum) - 5.5).abs() < 1e-10);
    }

    #[test]
    fn get_float_from_int() {
        let mut out = 0.0;
        assert!(get_float_maybe(obj::new_small_int(42), &mut out));
        assert!((out - 42.0).abs() < 1e-10);
    }
}
