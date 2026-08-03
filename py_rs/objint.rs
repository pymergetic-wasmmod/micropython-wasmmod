//! rewrite of py/objint.c + py/objint.h
// symmetry: done

use crate::argcheck;
use crate::mpconfig;
use crate::mpprint::{self, Print, PrintKind};
use crate::mpz;
use crate::obj::{self, Int, Obj, ObjType};
use crate::objint_impl;
use crate::objint_mpz;
use crate::raise::{self, MpRaise};
use crate::runtime;
use crate::runtime0::{BinaryOp, UnaryOp};
use crate::smallint;

pub use objint_mpz::{int_get_checked, int_get_truncated, int_get_uint_checked, new_int, new_int_from_ll, new_int_from_str, new_int_from_uint, new_int_from_ull, type_int};

pub fn int_make_new(_type_in: &ObjType, n_args: usize, n_kw: usize, args: &[Obj]) -> Obj {
    argcheck::check_num(n_args, n_kw, 0, 2, false);
    match n_args {
        0 => obj::new_small_int(0),
        1 => {
            let o = runtime::unary_op_obj(UnaryOp::IntMaybe, args[0]);
            if o != obj::OBJ_NULL { return o; }
            raise::raise(MpRaise::TypeError("can't convert to int"));
        }
        _ => raise::raise(MpRaise::TypeError("int() arg 2 not supported yet")),
    }
}

pub fn int_print(print: &Print, self_in: Obj, _kind: PrintKind) {
    let s = int_formatted(self_in, 10, None, b'\0', b'\0');
    mpprint::print_str(print, &s);
}

pub fn int_format_size(num_bits: usize, base: i32, prefix: Option<&str>, comma: u8) -> usize {
    assert!((2..=16).contains(&base));
    let log_base2_floor = [0, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 3, 3, 3, 3, 4];
    let num_digits = num_bits / log_base2_floor[(base - 1) as usize] + 1;
    let num_commas = if comma != 0 { if base == 10 { num_digits / 3 } else { num_digits / 4 } } else { 0 };
    let prefix_len = prefix.map(|p| p.len()).unwrap_or(0);
    num_digits + num_commas + prefix_len + 2
}

pub fn int_formatted(self_in: Obj, base: i32, prefix: Option<&str>, base_char: u8, comma: u8) -> String {
    let mut buf = vec![0u8; 64];
    let len = if obj::is_small_int(self_in) {
        format_small(obj::small_int_value(self_in), base as u32, prefix, base_char, &mut buf)
    } else {
        unsafe {
            mpz::as_str_inpl(
                unsafe { &(*((obj::as_ptr(self_in) as *const objint_mpz::ObjInt))).mpz },
                base as u32,
                prefix,
                base_char,
                comma,
                &mut buf,
            )
        }
    };
    String::from_utf8_lossy(&buf[..len]).into_owned()
}

fn format_small(mut num: Int, base: u32, prefix: Option<&str>, base_char: u8, buf: &mut [u8]) -> usize {
    let mut sign = b'\0';
    if num < 0 { num = -num; sign = b'-'; }
    let mut digits = Vec::new();
    if num == 0 { digits.push(b'0'); } else {
        let mut n = num as u64;
        while n > 0 {
            let mut c = (n % base as u64) as u8;
            n /= base as u64;
            if c >= 10 { c += base_char - 10; } else { c += b'0'; }
            digits.push(c);
        }
    }
    let mut pos = 0usize;
    if let Some(p) = prefix { buf[pos..pos+p.len()].copy_from_slice(p.as_bytes()); pos += p.len(); }
    if sign != 0 { buf[pos] = sign; pos += 1; }
    for d in digits.iter().rev() { buf[pos] = *d; pos += 1; }
    pos
}

pub fn int_sign(self_in: Obj) -> i32 {
    if obj::is_small_int(self_in) {
        let v = obj::small_int_value(self_in);
        return if v < 0 { -1 } else if v > 0 { 1 } else { 0 };
    }
    unsafe {
        let z = unsafe { &(*(obj::as_ptr(self_in) as *const objint_mpz::ObjInt)).mpz };
        if mpz::is_zero(z) { 0 } else if z.neg { -1 } else { 1 }
    }
}

pub fn int_unary_op(op: UnaryOp, o_in: Obj) -> Obj {
    if obj::is_small_int(o_in) { return obj::OBJ_NULL; }
    objint_mpz::int_unary_op(op, o_in)
}

pub fn int_binary_op(op: BinaryOp, lhs_in: Obj, rhs_in: Obj) -> Obj {
    if obj::is_small_int(lhs_in) && obj::is_small_int(rhs_in) { return obj::OBJ_NULL; }
    if obj::is_small_int(lhs_in) || obj::is_exact_type(lhs_in, type_int()) {
        return objint_mpz::binary_op_mpz(op, lhs_in, rhs_in);
    }
    obj::OBJ_NULL
}

pub fn binary_op_extra_cases(op: BinaryOp, lhs_in: Obj, rhs_in: Obj) -> Obj {
    if rhs_in == obj::CONST_FALSE {
        return runtime::binary_op(op, lhs_in, obj::new_small_int(0)).unwrap_or(obj::OBJ_NULL);
    }
    if rhs_in == obj::CONST_TRUE {
        return runtime::binary_op(op, lhs_in, obj::new_small_int(1)).unwrap_or(obj::OBJ_NULL);
    }
    obj::OBJ_NULL
}

pub use objint_mpz::binary_op_mpz;

pub fn int_to_bytes(self_in: Obj, buf_len: usize, buf: &mut [u8], big_endian: bool, is_signed: bool, overflow_check: bool) {
    if obj::is_exact_type(self_in, type_int()) {
        unsafe {
            let z = unsafe { &(*(obj::as_ptr(self_in) as *const objint_mpz::ObjInt)).mpz };
            if overflow_check && !is_signed && z.neg { objint_impl::raise_unsigned_negative_overflow(); }
            if !mpz::as_bytes(z, big_endian, is_signed, buf_len, buf) && overflow_check {
                objint_impl::raise_to_bytes_overflow(buf_len);
            }
        }
    } else {
        objint_impl::small_int_to_bytes(obj::get_int(self_in), buf_len, buf, big_endian, is_signed, overflow_check);
    }
}
