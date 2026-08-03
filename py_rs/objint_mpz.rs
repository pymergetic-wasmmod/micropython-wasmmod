//! rewrite of py/objint_mpz.c
// symmetry: done

use crate::mpconfig;
use crate::mpz::{self, Mpz};
use crate::obj::{self, Int, Obj, ObjBase, ObjType};
use crate::raise::{self, MpRaise};
use crate::runtime0::{BinaryOp, UnaryOp};
use crate::smallint;

#[repr(C)]
pub struct ObjInt {
    pub base: ObjBase,
    pub mpz: Mpz,
}

static TYPE_INT: ObjType = make_type_int();

const fn make_type_int() -> ObjType {
    ObjType {
        base: ObjBase { type_: core::ptr::null() },
        flags: obj::TYPE_FLAG_NONE,
        name: 0,
        slot_index_make_new: 0,
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
        slot_index_locals_dict: 0,
        slots: core::ptr::null(),
    }
}

pub fn type_int() -> &'static ObjType {
    static INIT: std::sync::OnceLock<()> = std::sync::OnceLock::new();
    INIT.get_or_init(|| {});
    &TYPE_INT
}

pub fn new_mpz() -> *mut ObjInt {
    let o = crate::malloc::new_obj::<ObjInt>().expect("objint alloc");
    unsafe {
        (*o).base.type_ = type_int() as *const ObjType;
        mpz::init_zero(&mut (*o).mpz);
    }
    o
}

pub fn new_int(val: Int) -> Obj {
    if smallint::fits(val) { obj::new_small_int(val) } else { new_int_from_ll(val as i64) }
}

pub fn new_int_from_ll(val: i64) -> Obj {
    let o = new_mpz();
    unsafe { mpz::set_from_ll(&mut (*o).mpz, val, true); obj::from_ptr(o as *const ObjInt as *const ()) }
}

pub fn new_int_from_uint(val: obj::Uint) -> Obj {
    if (val & !smallint::POSITIVE_MASK as obj::Uint) == 0 {
        obj::new_small_int(val as Int)
    } else {
        new_int_from_ull(val as u64)
    }
}

pub fn new_int_from_ull(val: u64) -> Obj {
    let o = new_mpz();
    unsafe { mpz::set_from_ll(&mut (*o).mpz, val as i64, false); obj::from_ptr(o as *const ObjInt as *const ()) }
}

/// `mp_obj_new_int_from_str_len`
pub fn new_int_from_str(s: &str, neg: bool, base: u32) -> (Obj, usize) {
    let mut z = Mpz::default();
    let consumed = mpz::set_from_str(&mut z, s, neg, base);
    let mut v = 0;
    if mpz::as_int_checked(&z, &mut v) && smallint::fits(v) {
        (obj::new_small_int(v), consumed)
    } else {
        let o = new_mpz();
        unsafe {
            (*o).mpz = z;
            (obj::from_ptr(o as *const ObjInt as *const ()), consumed)
        }
    }
}

pub fn int_get_truncated(o: Obj) -> Int {
    if obj::is_small_int(o) { obj::small_int_value(o) } else { mpz::hash(unsafe { &(*(obj::as_ptr(o) as *const ObjInt)).mpz }) }
}

pub fn int_get_checked(o: Obj) -> Int {
    if obj::is_small_int(o) { return obj::small_int_value(o); }
    let mut v = 0;
    unsafe {
        if mpz::as_int_checked(unsafe { &(*(obj::as_ptr(o) as *const ObjInt)).mpz }, &mut v) { v } else {
            raise::raise(MpRaise::OverflowError("overflow converting long int"));
        }
    }
}

pub fn int_get_uint_checked(o: Obj) -> obj::Uint {
    if obj::is_small_int(o) {
        let v = obj::small_int_value(o);
        if v >= 0 { return v as obj::Uint; }
    } else {
        let mut v = 0;
        unsafe {
            if mpz::as_uint_checked(unsafe { &(*(obj::as_ptr(o) as *const ObjInt)).mpz }, &mut v) { return v; }
        }
    }
    raise::raise(MpRaise::OverflowError("overflow converting long int"));
}

pub fn int_unary_op(op: UnaryOp, o: Obj) -> Obj {
    if obj::is_small_int(o) { return obj::OBJ_NULL; }
    let self_ptr = obj::as_ptr(o) as *mut ObjInt;
    unsafe {
        let z = &mut (*self_ptr).mpz;
        match op {
            UnaryOp::Bool => obj::new_bool(!mpz::is_zero(z)),
            UnaryOp::Hash => obj::new_small_int(mpz::hash(z)),
            UnaryOp::Positive | UnaryOp::IntMaybe => o,
            UnaryOp::Negative => { let o2 = new_mpz(); mpz::neg_inpl(&mut (*o2).mpz, z); obj::from_ptr(o2 as *const ()) }
            UnaryOp::Invert => { let o2 = new_mpz(); mpz::not_inpl(&mut (*o2).mpz, z); obj::from_ptr(o2 as *const ()) }
            UnaryOp::Abs => if z.neg { let o2 = new_mpz(); mpz::abs_inpl(&mut (*o2).mpz, z); obj::from_ptr(o2 as *const ()) } else { o }
            _ => obj::OBJ_NULL,
        }
    }
}

pub fn int_binary_op(op: BinaryOp, lhs: Obj, rhs: Obj) -> Obj {
    binary_op_mpz(op, lhs, rhs)
}

pub fn binary_op_mpz(op: BinaryOp, lhs_in: Obj, rhs_in: Obj) -> Obj {
    let mut zlhs = Mpz::default();
    let mut zrhs = Mpz::default();
    load_mpz(lhs_in, &mut zlhs);
    load_mpz(rhs_in, &mut zrhs);
    if matches!(op, BinaryOp::Less | BinaryOp::More | BinaryOp::LessEqual | BinaryOp::MoreEqual | BinaryOp::Equal) {
        let cmp = mpz::cmp(&zlhs, &zrhs);
        return obj::new_bool(match op {
            BinaryOp::Less => cmp < 0,
            BinaryOp::More => cmp > 0,
            BinaryOp::LessEqual => cmp <= 0,
            BinaryOp::MoreEqual => cmp >= 0,
            BinaryOp::Equal => cmp == 0,
            _ => false,
        });
    }
    let o = new_mpz();
    unsafe {
        let res = &mut (*o).mpz;
        match op {
            BinaryOp::Add | BinaryOp::InplaceAdd => mpz::add_inpl(res, &zlhs, &zrhs),
            BinaryOp::Subtract | BinaryOp::InplaceSubtract => mpz::sub_inpl(res, &zlhs, &zrhs),
            BinaryOp::Multiply | BinaryOp::InplaceMultiply => mpz::mul_inpl(res, &zlhs, &zrhs),
            BinaryOp::FloorDivide | BinaryOp::InplaceFloorDivide | BinaryOp::Modulo | BinaryOp::InplaceModulo => {
                let mut quo = Mpz::default();
                let mut rem = Mpz::default();
                mpz::divmod_inpl(&mut quo, &mut rem, &zlhs, &zrhs);
                if matches!(op, BinaryOp::Modulo | BinaryOp::InplaceModulo) { *res = rem; } else { *res = quo; }
            }
            _ => return obj::OBJ_NULL,
        }
        let small = mpz::hash(res);
        if smallint::fits(small) { obj::new_small_int(small) } else { obj::from_ptr(o as *const ()) }
    }
}

fn load_mpz(o: Obj, z: &mut Mpz) {
    if obj::is_small_int(o) {
        mpz::set_from_int(z, obj::small_int_value(o));
    } else {
        unsafe { mpz::set(z, &(*(obj::as_ptr(o) as *const ObjInt)).mpz); }
    }
}
