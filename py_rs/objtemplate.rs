//! rewrite of py/objtemplate.c
// symmetry: done

use crate::mpconfig;
use crate::obj::{self, Obj, ObjBase, ObjType};

#[repr(C)]
pub struct ObjTemplate {
    pub base: ObjBase,
    pub strings: Obj,
    pub interpolations: Obj,
}

#[repr(C)]
pub struct ObjInterpolation {
    pub base: ObjBase,
    pub value: Obj,
    pub expression: Obj,
    pub conversion: Obj,
    pub format_spec: Obj,
}

static mut TYPE_TEMPLATE: ObjType = ObjType {
    base: ObjBase { type_: core::ptr::null() },
    flags: 0,
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
};

static mut TYPE_INTERPOLATION: ObjType = ObjType {
    base: ObjBase { type_: core::ptr::null() },
    flags: 0,
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
};

pub fn type_template() -> Option<&'static ObjType> {
    if mpconfig::PY_TSTRINGS {
        Some(unsafe { &TYPE_TEMPLATE })
    } else {
        None
    }
}

pub fn type_interpolation() -> Option<&'static ObjType> {
    if mpconfig::PY_TSTRINGS {
        Some(unsafe { &TYPE_INTERPOLATION })
    } else {
        None
    }
}

pub fn new_template(_n_args: usize, _args: &[Obj]) -> Obj {
    if !mpconfig::PY_TSTRINGS {
        return obj::OBJ_NULL;
    }
    obj::OBJ_NULL
}
