//! rewrite of py/objcell.c
// symmetry: done

use crate::malloc;
use crate::mpconfig;
use crate::mpprint::{self, Print, PrintKind};
use crate::obj::{self, Obj, ObjBase, ObjType};

#[repr(C)]
pub struct ObjCell {
    pub base: ObjBase,
    pub obj: Obj,
}

static mut CELL_SLOTS: [*const (); 1] = [cell_print as *const ()];

static TYPE: ObjType = ObjType {
    base: ObjBase { type_: core::ptr::null() },
    flags: obj::TYPE_FLAG_NONE,
    name: 0,
    slot_index_make_new: 0,
    slot_index_print: if mpconfig::ERROR_REPORTING == mpconfig::ERROR_REPORTING_DETAILED {
        1
    } else {
        0
    },
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
    slots: unsafe { CELL_SLOTS.as_ptr() },
};

pub fn type_cell() -> &'static ObjType {
    &TYPE
}

fn cell_print(print: &Print, o_in: Obj, _kind: PrintKind) {
    let o = unsafe { &*(obj::as_ptr(o_in) as *const ObjCell) };
    mpprint::print_str(print, "<cell ");
    if o.obj == obj::OBJ_NULL {
        mpprint::print_str(print, "(nil)");
    } else {
        obj::print_helper(print, o.obj, PrintKind::Repr);
    }
    mpprint::print_str(print, ">");
}

/// `mp_obj_new_cell`
pub fn new_cell(obj: Obj) -> Obj {
    let o = malloc::new_obj::<ObjCell>().expect("cell alloc");
    unsafe {
        (*o).base.type_ = type_cell() as *const ObjType;
        (*o).obj = obj;
        obj::from_ptr(o as *const ObjCell as *const ())
    }
}

/// `mp_obj_cell_get`
pub fn cell_get(self_in: Obj) -> Obj {
    unsafe { (*(obj::as_ptr(self_in) as *const ObjCell)).obj }
}

/// `mp_obj_cell_set`
pub fn cell_set(self_in: Obj, val: Obj) {
    unsafe {
        (*(obj::as_ptr(self_in) as *mut ObjCell)).obj = val;
    }
}
