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

pub use objint_mpz::{
    int_get_checked, int_get_truncated, int_get_uint_checked, new_int, new_int_from_ll,
    new_int_from_str, new_int_from_uint, new_int_from_ull, type_int,
};

use crate::malloc;
use crate::map::{self, MapElem};
use crate::obj::{
    self as obj_mod, BufferInfo, ObjBase, TYPE_FLAG_BINDS_SELF, TYPE_FLAG_BUILTIN_FUN,
};
use crate::objdict::{self, ObjDict};
use crate::objstr;
use crate::objtype;
use crate::qstr;
use core::mem::size_of;

type BuiltinFnVar = fn(usize, &[Obj]) -> Obj;

#[repr(C)]
struct ObjFunBuiltinVar {
    base: ObjBase,
    min_args: u8,
    max_args: u8,
    fun: BuiltinFnVar,
}

static mut FV_SLOTS: [*const (); 1] = [callv as *const ()];
static TYPE_FUN_VAR: ObjType = ObjType {
    base: ObjBase {
        type_: core::ptr::null(),
    },
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
    slots: unsafe { FV_SLOTS.as_ptr() },
};

fn callv(s: Obj, n: usize, k: usize, a: &[Obj]) -> Obj {
    let self_ = unsafe { &*(obj_mod::as_ptr(s) as *const ObjFunBuiltinVar) };
    argcheck::check_num(
        n,
        k,
        self_.min_args as usize,
        self_.max_args as usize,
        false,
    );
    (self_.fun)(n, a)
}

fn mkv(min: u8, max: u8, f: BuiltinFnVar) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltinVar>().expect("int fn");
    unsafe {
        (*o).base.type_ = &TYPE_FUN_VAR;
        (*o).min_args = min;
        (*o).max_args = max;
        (*o).fun = f;
        obj_mod::from_ptr(o as *const ObjFunBuiltinVar as *const ())
    }
}

/// `int.from_bytes` (classmethod — first arg is the type when bound).
fn int_from_bytes(n_args: usize, args: &[Obj]) -> Obj {
    // args: [cls, buf] or [cls, buf, byteorder]
    if n_args < 2 {
        raise::raise(MpRaise::TypeError("argument num/types mismatch"));
    }
    let mut buf = BufferInfo::default();
    obj_mod::get_buffer_raise(args[1], &mut buf, obj_mod::BUFFER_READ);
    let big_endian = n_args < 3 || {
        let order = objstr::str_get_str(args[2]);
        order != "little"
    };
    let data = buf.as_bytes();
    let mut value: u64 = 0;
    if big_endian {
        for &b in data {
            value = (value << 8) | b as u64;
        }
    } else {
        for &b in data.iter().rev() {
            value = (value << 8) | b as u64;
        }
    }
    new_int_from_ull(value)
}

/// `int.to_bytes(length, byteorder='big', *, signed=False)`.
fn int_to_bytes_method(n_args: usize, args: &[Obj]) -> Obj {
    // args[0] = self; optional length, byteorder
    let self_in = args[0];
    let length = if n_args >= 2 {
        obj_mod::get_int(args[1]) as isize
    } else {
        1
    };
    if length < 0 {
        raise::raise(MpRaise::ValueError(""));
    }
    let big_endian = if n_args >= 3 {
        objstr::str_get_str(args[2]) != "little"
    } else {
        true
    };
    let len = length as usize;
    let mut buf = vec![0u8; len];
    int_to_bytes(self_in, len, &mut buf, big_endian, false, true);
    objstr::new_bytes(&buf)
}

/// Install `from_bytes` / `to_bytes` on the int type (called from `type_int` init).
pub fn install_int_locals(type_: &mut ObjType, slots: &mut [*const (); 4]) {
    let from_bytes = objtype::new_classmethod(mkv(2, 4, int_from_bytes));
    let table = vec![
        MapElem {
            key: obj_mod::new_qstr(qstr::from_str("from_bytes")),
            value: from_bytes,
        },
        MapElem {
            key: obj_mod::new_qstr(qstr::from_str("to_bytes")),
            value: mkv(1, 3, int_to_bytes_method),
        },
    ];
    let ptr = obj_mod::malloc_helper(size_of::<ObjDict>(), objdict::type_dict()) as *mut ObjDict;
    unsafe {
        map::init_fixed_table(&mut (*ptr).map, table);
        // slots[0]=make_new, [1]=print, [2]=binary_op, [3]=locals
        slots[3] = obj_mod::from_ptr(ptr as *const ObjDict as *const ()).0 as *const ();
        type_.slot_index_locals_dict = 4;
        type_.slots = slots.as_ptr();
    }
}

pub fn int_make_new(_type_in: &ObjType, n_args: usize, n_kw: usize, args: &[Obj]) -> Obj {
    argcheck::check_num(n_args, n_kw, 0, 2, false);
    match n_args {
        0 => obj::new_small_int(0),
        1 => {
            let o = runtime::unary_op_obj(UnaryOp::IntMaybe, args[0]);
            if o != obj::OBJ_NULL {
                return o;
            }
            let mut bufinfo = BufferInfo::default();
            if obj::get_buffer(args[0], &mut bufinfo, obj::BUFFER_READ) {
                return crate::parsenum::parse_num_integer(bufinfo.as_bytes(), 10, None);
            }
            if mpconfig::PY_BUILTINS_FLOAT && crate::objfloat::is_float(args[0]) {
                let f = crate::objfloat::float_get(args[0]);
                if !f.is_finite() {
                    raise::raise(MpRaise::OverflowError(
                        "can't convert float infinity/nan to int",
                    ));
                }
                if f >= smallint::MIN as f64 && f <= smallint::MAX as f64 {
                    return obj::new_small_int(f as Int);
                }
                return new_int_from_ll(f as i64);
            }
            raise::raise(MpRaise::TypeError("can't convert to int"));
        }
        _ => {
            let (data, len) = objstr::str_get_data(args[0]);
            let base = obj::get_int(args[1]) as i32;
            crate::parsenum::parse_num_integer(&data[..len], base, None)
        }
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
    let num_commas = if comma != 0 {
        if base == 10 {
            num_digits / 3
        } else {
            num_digits / 4
        }
    } else {
        0
    };
    let prefix_len = prefix.map(|p| p.len()).unwrap_or(0);
    num_digits + num_commas + prefix_len + 2
}

pub fn int_formatted(
    self_in: Obj,
    base: i32,
    prefix: Option<&str>,
    base_char: u8,
    comma: u8,
) -> String {
    let mut buf = vec![0u8; 64];
    let len = if obj::is_small_int(self_in) {
        format_small(
            obj::small_int_value(self_in),
            base as u32,
            prefix,
            base_char,
            &mut buf,
        )
    } else {
        unsafe {
            mpz::as_str_inpl(
                unsafe { &(*(obj::as_ptr(self_in) as *const objint_mpz::ObjInt)).mpz },
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

fn format_small(
    mut num: Int,
    base: u32,
    prefix: Option<&str>,
    base_char: u8,
    buf: &mut [u8],
) -> usize {
    let mut sign = b'\0';
    if num < 0 {
        num = -num;
        sign = b'-';
    }
    let mut digits = Vec::new();
    if num == 0 {
        digits.push(b'0');
    } else {
        let mut n = num as u64;
        while n > 0 {
            let mut c = (n % base as u64) as u8;
            n /= base as u64;
            if c >= 10 {
                c += base_char - 10;
            } else {
                c += b'0';
            }
            digits.push(c);
        }
    }
    let mut pos = 0usize;
    if let Some(p) = prefix {
        buf[pos..pos + p.len()].copy_from_slice(p.as_bytes());
        pos += p.len();
    }
    if sign != 0 {
        buf[pos] = sign;
        pos += 1;
    }
    for d in digits.iter().rev() {
        buf[pos] = *d;
        pos += 1;
    }
    pos
}

pub fn int_sign(self_in: Obj) -> i32 {
    if obj::is_small_int(self_in) {
        let v = obj::small_int_value(self_in);
        return if v < 0 {
            -1
        } else if v > 0 {
            1
        } else {
            0
        };
    }
    unsafe {
        let z = unsafe { &(*(obj::as_ptr(self_in) as *const objint_mpz::ObjInt)).mpz };
        if mpz::is_zero(z) {
            0
        } else if z.neg {
            -1
        } else {
            1
        }
    }
}

pub fn int_unary_op(op: UnaryOp, o_in: Obj) -> Obj {
    if obj::is_small_int(o_in) {
        return obj::OBJ_NULL;
    }
    objint_mpz::int_unary_op(op, o_in)
}

pub fn int_binary_op(op: BinaryOp, lhs_in: Obj, rhs_in: Obj) -> Obj {
    if obj::is_small_int(lhs_in) && obj::is_small_int(rhs_in) {
        return obj::OBJ_NULL;
    }
    let rhs_is_int = obj::is_small_int(rhs_in) || obj::is_exact_type(rhs_in, type_int());
    if !rhs_is_int {
        // Bool / other types: `binary_op_extra_cases` (or type error upstream).
        return obj::OBJ_NULL;
    }
    if obj::is_small_int(lhs_in) || obj::is_exact_type(lhs_in, type_int()) {
        return objint_mpz::binary_op_mpz(op, lhs_in, rhs_in);
    }
    obj::OBJ_NULL
}

/// Type slot entry: mpz path first, then bool-as-0/1 (C `mp_obj_int_binary_op`).
pub fn int_binary_op_dispatch(op: BinaryOp, lhs_in: Obj, rhs_in: Obj) -> Obj {
    let r = int_binary_op(op, lhs_in, rhs_in);
    if r != obj::OBJ_NULL {
        return r;
    }
    binary_op_extra_cases(op, lhs_in, rhs_in)
}

pub fn binary_op_extra_cases(op: BinaryOp, lhs_in: Obj, rhs_in: Obj) -> Obj {
    if rhs_in == obj::CONST_FALSE {
        return runtime::binary_op_obj(op, lhs_in, obj::new_small_int(0));
    }
    if rhs_in == obj::CONST_TRUE {
        return runtime::binary_op_obj(op, lhs_in, obj::new_small_int(1));
    }
    // C: multiply is commutative for str/bytes/tuple/list — delegate with swapped args.
    if op == BinaryOp::Multiply
        && (obj::is_str_or_bytes(rhs_in)
            || obj::is_exact_type(rhs_in, obj::type_tuple())
            || obj::is_exact_type(rhs_in, crate::objlist::type_list()))
    {
        return runtime::binary_op_obj(op, rhs_in, lhs_in);
    }
    obj::OBJ_NULL
}

pub use objint_mpz::binary_op_mpz;

pub fn int_to_bytes(
    self_in: Obj,
    buf_len: usize,
    buf: &mut [u8],
    big_endian: bool,
    is_signed: bool,
    overflow_check: bool,
) {
    if obj::is_exact_type(self_in, type_int()) {
        unsafe {
            let z = unsafe { &(*(obj::as_ptr(self_in) as *const objint_mpz::ObjInt)).mpz };
            if overflow_check && !is_signed && z.neg {
                objint_impl::raise_unsigned_negative_overflow();
            }
            if !mpz::as_bytes(z, big_endian, is_signed, buf_len, buf) && overflow_check {
                objint_impl::raise_to_bytes_overflow(buf_len);
            }
        }
    } else {
        objint_impl::small_int_to_bytes(
            obj::get_int(self_in),
            buf_len,
            buf,
            big_endian,
            is_signed,
            overflow_check,
        );
    }
}
