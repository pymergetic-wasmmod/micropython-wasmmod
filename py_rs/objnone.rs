//! rewrite of py/objnone.c
// symmetry: done

use crate::mpconfig;
use crate::mpprint::{self, Print, PrintKind};
use crate::obj::{self, Obj, ObjBase, ObjType};
use crate::qstr;

#[repr(C)]
pub struct ObjNone {
    pub base: ObjBase,
}

static mut NONE_SLOTS: [*const (); 1] = [core::ptr::null(); 1];

static mut TYPE_NONE: ObjType = ObjType {
    base: ObjBase {
        type_: core::ptr::null(),
    },
    flags: obj::TYPE_FLAG_NONE,
    name: 0,
    slot_index_make_new: 0,
    slot_index_print: 1,
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

/// `none_print` from objnone.c
pub fn none_print(print: &Print, _self_in: Obj, kind: PrintKind) {
    if mpconfig::PY_JSON && kind == PrintKind::Json {
        mpprint::print_str(print, "null");
    } else {
        mpprint::print_str(print, "None");
    }
}

fn init_type_none() {
    static INIT: std::sync::OnceLock<()> = std::sync::OnceLock::new();
    INIT.get_or_init(|| unsafe {
        NONE_SLOTS[0] = none_print as *const ();
        let t = core::ptr::addr_of_mut!(TYPE_NONE);
        (*t).slots = NONE_SLOTS.as_ptr();
        (*t).name = qstr::from_str("NoneType");
    });
}

pub fn type_none() -> &'static ObjType {
    init_type_none();
    unsafe { &*core::ptr::addr_of!(TYPE_NONE) }
}
