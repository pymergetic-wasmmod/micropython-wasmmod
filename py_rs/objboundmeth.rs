//! rewrite of py/objboundmeth.c
// symmetry: done

use crate::malloc;
use crate::mpconfig;
use crate::mpprint::{self, Print, PrintKind};
use crate::obj::{self, Obj, ObjBase, ObjType};
use crate::qstr::{self, Qstr};
use crate::runtime;
use crate::runtime0::{BinaryOp, UnaryOp};

#[repr(C)]
pub struct ObjBoundMeth {
    pub base: ObjBase,
    pub meth: Obj,
    pub self_: Obj,
}

static mut BOUND_METH_SLOTS: [*const (); 4] = [
    bound_meth_call as *const (),
    bound_meth_unary_op as *const (),
    bound_meth_binary_op as *const (),
    core::ptr::null(),
];

static mut TYPE: ObjType = ObjType {
    base: ObjBase {
        type_: core::ptr::null(),
    },
    flags: obj::TYPE_FLAG_NONE,
    name: 0,
    slot_index_make_new: 0,
    slot_index_print: if mpconfig::ERROR_REPORTING == mpconfig::ERROR_REPORTING_DETAILED {
        4
    } else {
        0
    },
    slot_index_call: 1,
    slot_index_unary_op: 2,
    slot_index_binary_op: 3,
    slot_index_attr: if mpconfig::PY_FUNCTION_ATTRS { 4 } else { 0 },
    slot_index_subscr: 0,
    slot_index_iter: 0,
    slot_index_buffer: 0,
    slot_index_protocol: 0,
    slot_index_parent: 0,
    slot_index_locals_dict: 0,
    slots: unsafe { BOUND_METH_SLOTS.as_ptr() },
};

static INIT: std::sync::OnceLock<()> = std::sync::OnceLock::new();

fn init_type() {
    INIT.get_or_init(|| unsafe {
        TYPE.name = qstr::from_str("bound_method");
        if mpconfig::ERROR_REPORTING == mpconfig::ERROR_REPORTING_DETAILED {
            BOUND_METH_SLOTS[3] = bound_meth_print as *const ();
        }
        if mpconfig::PY_FUNCTION_ATTRS {
            BOUND_METH_SLOTS[3] = bound_meth_attr as *const ();
        }
    });
}

pub fn type_bound_meth() -> &'static ObjType {
    init_type();
    unsafe { &*core::ptr::addr_of!(TYPE) }
}

/// `mp_call_method_self_n_kw` (also in runtime.rs; kept for C API parity).
pub fn call_method_self_n_kw(
    meth: Obj,
    self_: Obj,
    n_args: usize,
    n_kw: usize,
    args: &[Obj],
) -> Obj {
    runtime::call_method_self_n_kw(meth, self_, n_args, n_kw, args)
}

fn bound_meth_call(self_in: Obj, n_args: usize, n_kw: usize, args: &[Obj]) -> Obj {
    let self_ = unsafe { &*(obj::as_ptr(self_in) as *const ObjBoundMeth) };
    call_method_self_n_kw(self_.meth, self_.self_, n_args, n_kw, args)
}

fn bound_meth_unary_op(op: UnaryOp, self_in: Obj) -> Obj {
    let self_ = unsafe { &*(obj::as_ptr(self_in) as *const ObjBoundMeth) };
    if op == UnaryOp::Hash {
        obj::new_small_int((self_.self_.0 ^ self_.meth.0) as obj::Int)
    } else {
        obj::OBJ_NULL
    }
}

fn bound_meth_binary_op(op: BinaryOp, lhs_in: Obj, rhs_in: Obj) -> Obj {
    if op != BinaryOp::Equal {
        return obj::OBJ_NULL;
    }
    let lhs = unsafe { &*(obj::as_ptr(lhs_in) as *const ObjBoundMeth) };
    let rhs = unsafe { &*(obj::as_ptr(rhs_in) as *const ObjBoundMeth) };
    if mpconfig::PY_BOUND_METHOD_FULL_EQUALITY_CHECK {
        obj::new_bool(obj::equal(lhs.self_, rhs.self_) && obj::equal(lhs.meth, rhs.meth))
    } else {
        obj::new_bool(lhs.self_ == rhs.self_ && lhs.meth == rhs.meth)
    }
}

fn bound_meth_print(print: &Print, o_in: Obj, _kind: PrintKind) {
    let o = unsafe { &*(obj::as_ptr(o_in) as *const ObjBoundMeth) };
    mpprint::print_str(print, "<bound_method ");
    mpprint::print_str(print, &format!("{:?} ", o_in));
    obj::print_helper(print, o.self_, PrintKind::Repr);
    mpprint::print_str(print, ".");
    obj::print_helper(print, o.meth, PrintKind::Repr);
    mpprint::print_str(print, ">");
}

fn bound_meth_attr(self_in: Obj, attr: Qstr, dest: &mut [Obj; 2]) {
    if dest[0] != obj::OBJ_NULL {
        return;
    }
    let self_ = unsafe { &*(obj::as_ptr(self_in) as *const ObjBoundMeth) };
    runtime::load_method_maybe(self_.meth, attr, dest);
}

/// `mp_obj_new_bound_meth`
pub fn new_bound_meth(meth: Obj, self_: Obj) -> Obj {
    let o = malloc::new_obj::<ObjBoundMeth>().expect("bound_meth alloc");
    unsafe {
        (*o).base.type_ = type_bound_meth() as *const ObjType;
        (*o).meth = meth;
        (*o).self_ = self_;
        obj::from_ptr(o as *const ObjBoundMeth as *const ())
    }
}
