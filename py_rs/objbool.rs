//! rewrite of py/objbool.c
// symmetry: done

use crate::argcheck;
use crate::mpconfig;
use crate::mpprint::{self, Print, PrintKind};
use crate::obj::{self, Obj, ObjBase, ObjType, TYPE_FLAG_EQ_CHECKS_OTHER_TYPE};
use crate::qstr;
use crate::raise::{self, MpRaise};
use crate::runtime;
use crate::runtime0::{BinaryOp, UnaryOp};

#[repr(C)]
pub struct ObjBool {
    pub base: ObjBase,
    pub value: bool,
}

static mut BOOL_SLOTS: [*const (); 4] = [core::ptr::null(); 4];

static mut TYPE_BOOL: ObjType = ObjType {
    base: ObjBase {
        type_: core::ptr::null(),
    },
    flags: TYPE_FLAG_EQ_CHECKS_OTHER_TYPE,
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
    slots: core::ptr::null(),
};

#[inline]
fn bool_value(o: Obj) -> bool {
    if mpconfig::OBJ_IMMEDIATE_OBJS {
        obj::bool_value(o)
    } else {
        unsafe { (*(obj::as_ptr(o) as *const ObjBool)).value }
    }
}

/// `bool_print` from objbool.c
pub fn bool_print(print: &Print, self_in: Obj, kind: PrintKind) {
    let value = bool_value(self_in);
    if mpconfig::PY_JSON && kind == PrintKind::Json {
        mpprint::print_str(print, if value { "true" } else { "false" });
    } else {
        mpprint::print_str(print, if value { "True" } else { "False" });
    }
}

/// `bool_make_new` from objbool.c
pub fn bool_make_new(_type_in: &ObjType, n_args: usize, n_kw: usize, args: &[Obj]) -> Obj {
    argcheck::check_num(n_args, n_kw, 0, 1, false);
    if n_args == 0 {
        obj::CONST_FALSE
    } else {
        obj::new_bool(obj::is_true(args[0]))
    }
}

/// `bool_unary_op` from objbool.c
pub fn bool_unary_op(op: UnaryOp, o_in: Obj) -> Obj {
    if op == UnaryOp::Len {
        return obj::OBJ_NULL;
    }
    let value = bool_value(o_in);
    // C calls `mp_unary_op` (full path), not the small-int-only smoke helper.
    runtime::unary_op_obj(op, obj::new_small_int(i32::from(value) as obj::Int))
}

/// `bool_binary_op` from objbool.c
pub fn bool_binary_op(op: BinaryOp, lhs_in: Obj, rhs_in: Obj) -> Obj {
    let value = bool_value(lhs_in);
    // C calls `mp_binary_op` (full path), not the small-int-only smoke helper.
    runtime::binary_op_obj(op, obj::new_small_int(i32::from(value) as obj::Int), rhs_in)
}

fn init_type_bool() {
    static INIT: std::sync::OnceLock<()> = std::sync::OnceLock::new();
    INIT.get_or_init(|| unsafe {
        BOOL_SLOTS[0] = bool_make_new as *const ();
        BOOL_SLOTS[1] = bool_print as *const ();
        BOOL_SLOTS[2] = bool_unary_op as *const ();
        BOOL_SLOTS[3] = bool_binary_op as *const ();
        let t = core::ptr::addr_of_mut!(TYPE_BOOL);
        (*t).slots = BOOL_SLOTS.as_ptr();
        (*t).name = qstr::from_str("bool");
    });
}

pub fn type_bool() -> &'static ObjType {
    init_type_bool();
    unsafe { &*core::ptr::addr_of!(TYPE_BOOL) }
}
