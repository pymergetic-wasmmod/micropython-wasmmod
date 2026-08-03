//! rewrite of py/objcomplex.c
// symmetry: done

use crate::argcheck;
use crate::malloc;
use crate::mpconfig;
use crate::mpprint::{self, Print, PrintKind};
use crate::obj::{self, Obj, ObjBase, ObjType, TYPE_FLAG_EQ_CHECKS_OTHER_TYPE, TYPE_FLAG_EQ_NOT_REFLEXIVE};
use crate::objfloat::{self, MpFloat};
use crate::parsenum;
use crate::qstr;
use crate::raise::{self, MpRaise};
use crate::runtime0::{BinaryOp, UnaryOp};

#[repr(C)]
pub struct ObjComplex {
    pub base: ObjBase,
    pub real: MpFloat,
    pub imag: MpFloat,
}

static mut COMPLEX_SLOTS: [*const (); 5] = [
    complex_make_new as *const (),
    complex_print as *const (),
    complex_unary_op as *const (),
    complex_binary_op_slot as *const (),
    complex_attr as *const (),
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
    slot_index_attr: 5,
    slot_index_subscr: 0,
    slot_index_iter: 0,
    slot_index_buffer: 0,
    slot_index_protocol: 0,
    slot_index_parent: 0,
    slot_index_locals_dict: 0,
    slots: unsafe { COMPLEX_SLOTS.as_ptr() },
};

pub fn type_complex() -> &'static ObjType {
    &TYPE
}

pub fn is_complex(o: Obj) -> bool {
    obj::is_exact_type(o, type_complex())
}

/// `mp_obj_new_complex`
pub fn new_complex(real: MpFloat, imag: MpFloat) -> Obj {
    let o = malloc::new_obj::<ObjComplex>().expect("objcomplex alloc");
    unsafe {
        (*o).base.type_ = type_complex() as *const ObjType;
        (*o).real = real;
        (*o).imag = imag;
        obj::from_ptr(o as *const ObjComplex as *const ())
    }
}

/// `mp_obj_complex_get`
pub fn complex_get(o: Obj, real: &mut MpFloat, imag: &mut MpFloat) {
    assert!(is_complex(o));
    unsafe {
        let self_ = &*(obj::as_ptr(o) as *const ObjComplex);
        *real = self_.real;
        *imag = self_.imag;
    }
}

/// `mp_obj_get_complex_maybe`
pub fn get_complex_maybe(o: Obj, real: &mut MpFloat, imag: &mut MpFloat) -> bool {
    if objfloat::get_float_maybe(o, real) {
        *imag = 0.0;
        return true;
    }
    if is_complex(o) {
        complex_get(o, real, imag);
        return true;
    }
    let converted = crate::runtime::unary_op_obj(UnaryOp::ComplexMaybe, o);
    if converted != obj::OBJ_NULL && is_complex(converted) {
        complex_get(converted, real, imag);
        return true;
    }
    false
}

/// `mp_obj_get_complex`
pub fn get_complex(o: Obj, real: &mut MpFloat, imag: &mut MpFloat) {
    if !get_complex_maybe(o, real, imag) {
        raise::raise(MpRaise::TypeError("can't convert to complex"));
    }
}

fn format_float_simple(val: MpFloat, show_sign: bool) -> String {
    let mut s = if show_sign && val >= 0.0 {
        format!("+{:.12}", val)
    } else {
        format!("{:.12}", val)
    };
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

pub fn complex_print(print: &Print, o_in: Obj, _kind: PrintKind) {
    let o = unsafe { &*(obj::as_ptr(o_in) as *const ObjComplex) };
    if o.real != 0.0 {
        mpprint::print_str(print, "(");
        mpprint::print_str(print, &format_float_simple(o.real, false));
        mpprint::print_str(print, &format_float_simple(o.imag, true));
        mpprint::print_str(print, "j)");
    } else {
        mpprint::print_str(print, &format_float_simple(o.imag, false));
        mpprint::print_str(print, "j");
    }
}

pub fn complex_make_new(_type_in: &ObjType, n_args: usize, n_kw: usize, args: &[Obj]) -> Obj {
    debug_assert!(mpconfig::PY_BUILTINS_COMPLEX);
    argcheck::check_num(n_args, n_kw, 0, 2, false);
    match n_args {
        0 => new_complex(0.0, 0.0),
        1 => {
            if obj::is_str(args[0]) {
                let q = crate::objstr::str_get_qstr(args[0]);
                let s = qstr::str_from_qstr(q).unwrap_or_default();
                return parsenum::parse_num_complex(s.as_bytes(), None);
            } else if is_complex(args[0]) {
                args[0]
            } else {
                let mut real = 0.0;
                let mut imag = 0.0;
                get_complex(args[0], &mut real, &mut imag);
                new_complex(real, imag)
            }
        }
        _ => {
            let (mut real, mut imag) = if is_complex(args[0]) {
                let mut r = 0.0;
                let mut i = 0.0;
                complex_get(args[0], &mut r, &mut i);
                (r, i)
            } else {
                (objfloat::get_float(args[0]), 0.0)
            };
            if is_complex(args[1]) {
                let mut real2 = 0.0;
                let mut imag2 = 0.0;
                complex_get(args[1], &mut real2, &mut imag2);
                real -= imag2;
                imag += real2;
            } else {
                imag += objfloat::get_float(args[1]);
            }
            new_complex(real, imag)
        }
    }
}

pub fn complex_unary_op(op: UnaryOp, o_in: Obj) -> Obj {
    let o = unsafe { &*(obj::as_ptr(o_in) as *const ObjComplex) };
    match op {
        UnaryOp::Bool => obj::new_bool(o.real != 0.0 || o.imag != 0.0),
        UnaryOp::Hash => {
            obj::new_small_int(objfloat::float_hash(o.real) ^ objfloat::float_hash(o.imag))
        }
        UnaryOp::Positive => o_in,
        UnaryOp::Negative => new_complex(-o.real, -o.imag),
        UnaryOp::Abs => objfloat::new_float((o.real * o.real + o.imag * o.imag).sqrt()),
        _ => obj::OBJ_NULL,
    }
}

pub fn complex_binary_op_slot(op: BinaryOp, lhs_in: Obj, rhs_in: Obj) -> Obj {
    let lhs = unsafe { &*(obj::as_ptr(lhs_in) as *const ObjComplex) };
    complex_binary_op(op, lhs.real, lhs.imag, rhs_in)
}

pub fn complex_attr(self_in: Obj, attr: qstr::Qstr, dest: &mut [Obj; 2]) {
    if dest[0] != obj::OBJ_NULL {
        return;
    }
    let self_ = unsafe { &*(obj::as_ptr(self_in) as *const ObjComplex) };
    if attr == qstr::from_str("real") {
        dest[0] = objfloat::new_float(self_.real);
    } else if attr == qstr::from_str("imag") {
        dest[0] = objfloat::new_float(self_.imag);
    }
}

/// `mp_obj_complex_binary_op`
pub fn complex_binary_op(op: BinaryOp, mut lhs_real: MpFloat, mut lhs_imag: MpFloat, rhs_in: Obj) -> Obj {
    let mut rhs_real = 0.0;
    let mut rhs_imag = 0.0;
    if !get_complex_maybe(rhs_in, &mut rhs_real, &mut rhs_imag) {
        return obj::OBJ_NULL;
    }

    match op {
        BinaryOp::Add | BinaryOp::InplaceAdd => {
            lhs_real += rhs_real;
            lhs_imag += rhs_imag;
        }
        BinaryOp::Subtract | BinaryOp::InplaceSubtract => {
            lhs_real -= rhs_real;
            lhs_imag -= rhs_imag;
        }
        BinaryOp::Multiply | BinaryOp::InplaceMultiply => {
            let real = lhs_real * rhs_real - lhs_imag * rhs_imag;
            lhs_imag = lhs_real * rhs_imag + lhs_imag * rhs_real;
            lhs_real = real;
        }
        BinaryOp::FloorDivide | BinaryOp::InplaceFloorDivide => {
            raise::raise(MpRaise::TypeError("can't truncate-divide a complex number"));
        }
        BinaryOp::TrueDivide | BinaryOp::InplaceTrueDivide => {
            if rhs_imag == 0.0 {
                if rhs_real == 0.0 {
                    raise::raise(MpRaise::ZeroDivisionError);
                }
                lhs_real /= rhs_real;
                lhs_imag /= rhs_real;
            } else if rhs_real == 0.0 {
                let real = lhs_imag / rhs_imag;
                lhs_imag = -lhs_real / rhs_imag;
                lhs_real = real;
            } else {
                let rhs_len_sq = rhs_real * rhs_real + rhs_imag * rhs_imag;
                rhs_real /= rhs_len_sq;
                rhs_imag /= -rhs_len_sq;
                let real = lhs_real * rhs_real - lhs_imag * rhs_imag;
                lhs_imag = lhs_real * rhs_imag + lhs_imag * rhs_real;
                lhs_real = real;
            }
        }
        BinaryOp::Power | BinaryOp::InplacePower => {
            let abs1 = (lhs_real * lhs_real + lhs_imag * lhs_imag).sqrt();
            if abs1 == 0.0 {
                if rhs_imag == 0.0 && rhs_real >= 0.0 {
                    lhs_real = if rhs_real == 0.0 { 1.0 } else { 0.0 };
                    lhs_imag = 0.0;
                } else {
                    raise::raise(MpRaise::ZeroDivisionError);
                }
            } else {
                let ln1 = abs1.ln();
                let arg1 = lhs_imag.atan2(lhs_real);
                let x3 = rhs_real * ln1 - rhs_imag * arg1;
                let y3 = rhs_imag * ln1 + rhs_real * arg1;
                let exp_x3 = x3.exp();
                lhs_real = exp_x3 * y3.cos();
                lhs_imag = exp_x3 * y3.sin();
            }
        }
        BinaryOp::Equal => return obj::new_bool(lhs_real == rhs_real && lhs_imag == rhs_imag),
        _ => return obj::OBJ_NULL,
    }
    new_complex(lhs_real, lhs_imag)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gc;

    #[test]
    fn complex_multiply() {
        let _ = gc::init();
        let a = new_complex(1.0, 2.0);
        let b = new_complex(3.0, 4.0);
        let p = complex_binary_op(BinaryOp::Multiply, 1.0, 2.0, b);
        let mut r = 0.0;
        let mut i = 0.0;
        complex_get(p, &mut r, &mut i);
        assert!((r - (-5.0)).abs() < 1e-10);
        assert!((i - 10.0).abs() < 1e-10);
    }
}
