//! rewrite of py/opmethods.c
// symmetry: done

use crate::malloc;
use crate::obj::{
    self, Obj, ObjBase, ObjType, OBJ_NULL, OBJ_SENTINEL, TYPE_FLAG_BINDS_SELF,
    TYPE_FLAG_BUILTIN_FUN,
};
use crate::runtime0::BinaryOp;

type BuiltinFn2 = fn(Obj, Obj) -> Obj;
type BuiltinFn3 = fn(Obj, Obj, Obj) -> Obj;

#[repr(C)]
struct ObjFunBuiltinFixed {
    base: ObjBase,
    fun: ObjFunBuiltinFixedFun,
}

#[repr(C)]
union ObjFunBuiltinFixedFun {
    f2: BuiltinFn2,
    f3: BuiltinFn3,
}

static mut OPMETHOD_SLOTS_2: [*const (); 1] = [op_fun_call_2 as *const ()];
static mut OPMETHOD_SLOTS_3: [*const (); 1] = [op_fun_call_3 as *const ()];

static mut TYPE_FUN_BUILTIN_2: ObjType = ObjType {
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
    slots: core::ptr::null(),
};

static mut TYPE_FUN_BUILTIN_3: ObjType = ObjType {
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
    slots: core::ptr::null(),
};

static INIT: std::sync::OnceLock<()> = std::sync::OnceLock::new();

fn init_types() {
    INIT.get_or_init(|| unsafe {
        TYPE_FUN_BUILTIN_2.slots = OPMETHOD_SLOTS_2.as_ptr();
        TYPE_FUN_BUILTIN_3.slots = OPMETHOD_SLOTS_3.as_ptr();
    });
}

fn op_fun_call_2(self_in: Obj, n_args: usize, n_kw: usize, args: &[Obj]) -> Obj {
    crate::argcheck::check_num(n_args, n_kw, 2, 2, false);
    let self_ = unsafe { &*(obj::as_ptr(self_in) as *const ObjFunBuiltinFixed) };
    unsafe { (self_.fun.f2)(args[0], args[1]) }
}

fn op_fun_call_3(self_in: Obj, n_args: usize, n_kw: usize, args: &[Obj]) -> Obj {
    crate::argcheck::check_num(n_args, n_kw, 3, 3, false);
    let self_ = unsafe { &*(obj::as_ptr(self_in) as *const ObjFunBuiltinFixed) };
    unsafe { (self_.fun.f3)(args[0], args[1], args[2]) }
}

fn new_fun_builtin_2(fun: BuiltinFn2) -> Obj {
    init_types();
    let o = malloc::new_obj::<ObjFunBuiltinFixed>().expect("builtin fun");
    unsafe {
        (*o).base.type_ = &raw const TYPE_FUN_BUILTIN_2 as *const ObjType;
        (*o).fun.f2 = fun;
    }
    obj::from_ptr(o as *const ObjFunBuiltinFixed as *const ())
}

fn new_fun_builtin_3(fun: BuiltinFn3) -> Obj {
    init_types();
    let o = malloc::new_obj::<ObjFunBuiltinFixed>().expect("builtin fun");
    unsafe {
        (*o).base.type_ = &raw const TYPE_FUN_BUILTIN_3 as *const ObjType;
        (*o).fun.f3 = fun;
    }
    obj::from_ptr(o as *const ObjFunBuiltinFixed as *const ())
}

/// `op_getitem` / `mp_op_getitem_obj`.
pub fn op_getitem(self_in: Obj, key_in: Obj) -> Obj {
    let t = obj::get_type(self_in);
    if let Some(subscr) = obj::type_get_subscr(t) {
        subscr(self_in, key_in, OBJ_SENTINEL)
    } else {
        crate::raise::raise(crate::raise::MpRaise::TypeError(
            "object doesn't support item assignment",
        ));
    }
}

/// `op_setitem` / `mp_op_setitem_obj`.
pub fn op_setitem(self_in: Obj, key_in: Obj, value_in: Obj) -> Obj {
    let t = obj::get_type(self_in);
    if let Some(subscr) = obj::type_get_subscr(t) {
        subscr(self_in, key_in, value_in)
    } else {
        crate::raise::raise(crate::raise::MpRaise::TypeError(
            "object doesn't support item assignment",
        ));
    }
}

/// `op_delitem` / `mp_op_delitem_obj`.
pub fn op_delitem(self_in: Obj, key_in: Obj) -> Obj {
    let t = obj::get_type(self_in);
    if let Some(subscr) = obj::type_get_subscr(t) {
        subscr(self_in, key_in, OBJ_NULL)
    } else {
        crate::raise::raise(crate::raise::MpRaise::TypeError(
            "object doesn't support item deletion",
        ));
    }
}

/// `op_contains` / `mp_op_contains_obj`.
pub fn op_contains(lhs_in: Obj, rhs_in: Obj) -> Obj {
    let t = obj::get_type(lhs_in);
    if let Some(binary) = obj::type_get_binary_op(t) {
        binary(BinaryOp::Contains, lhs_in, rhs_in)
    } else {
        crate::raise::raise(crate::raise::MpRaise::TypeError(
            "unsupported operand type(s)",
        ));
    }
}

/// `mp_op_getitem_obj`.
pub fn op_getitem_obj() -> Obj {
    new_fun_builtin_2(op_getitem)
}

/// `mp_op_setitem_obj`.
pub fn op_setitem_obj() -> Obj {
    new_fun_builtin_3(op_setitem)
}

/// `mp_op_delitem_obj`.
pub fn op_delitem_obj() -> Obj {
    new_fun_builtin_2(op_delitem)
}

/// `mp_op_contains_obj`.
pub fn op_contains_obj() -> Obj {
    new_fun_builtin_2(op_contains)
}
