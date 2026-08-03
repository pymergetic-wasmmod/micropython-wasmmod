//! rewrite of py/objsingleton.c
// symmetry: done

use crate::mpconfig;
use crate::mpprint::{self, Print, PrintKind, VaArg};
use crate::obj::{self, Obj, ObjBase, ObjType};
use crate::qstr::{self, Qstr};

use std::sync::OnceLock;

#[repr(C)]
pub struct ObjSingleton {
    pub base: ObjBase,
    pub name: Qstr,
}

unsafe impl Send for ObjSingleton {}
unsafe impl Sync for ObjSingleton {}

static mut SINGLETON_SLOTS: [*const (); 1] = [core::ptr::null(); 1];

static mut TYPE_SINGLETON: ObjType = ObjType {
    base: ObjBase {
        type_: core::ptr::null(),
    },
    flags: obj::TYPE_FLAG_NONE,
    name: qstr::QSTR_NULL,
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

static ELLIPSIS: OnceLock<ObjSingleton> = OnceLock::new();
static NOT_IMPLEMENTED: OnceLock<ObjSingleton> = OnceLock::new();

fn ellipsis_obj() -> &'static ObjSingleton {
    ELLIPSIS.get_or_init(|| ObjSingleton {
        base: ObjBase {
            type_: type_singleton() as *const ObjType,
        },
        name: qstr::from_str("Ellipsis"),
    })
}

fn notimplemented_obj() -> Option<&'static ObjSingleton> {
    if mpconfig::PY_BUILTINS_NOTIMPLEMENTED {
        Some(NOT_IMPLEMENTED.get_or_init(|| ObjSingleton {
            base: ObjBase {
                type_: type_singleton() as *const ObjType,
            },
            name: qstr::from_str("NotImplemented"),
        }))
    } else {
        None
    }
}

/// `singleton_print` from objsingleton.c
pub fn singleton_print(print: &Print, self_in: Obj, _kind: PrintKind) {
    let self_ptr = unsafe { &*(obj::as_ptr(self_in) as *const ObjSingleton) };
    let _ = mpprint::vprintf(print, "%q", &mut [VaArg::Qstr(self_ptr.name)].into_iter());
}

pub fn const_ellipsis() -> Obj {
    obj::from_ptr(ellipsis_obj() as *const ObjSingleton as *const ())
}

pub fn const_notimplemented() -> Option<Obj> {
    notimplemented_obj().map(|o| obj::from_ptr(o as *const ObjSingleton as *const ()))
}

fn init_type_singleton() {
    static INIT: std::sync::OnceLock<()> = std::sync::OnceLock::new();
    INIT.get_or_init(|| unsafe {
        SINGLETON_SLOTS[0] = singleton_print as *const ();
        let t = core::ptr::addr_of_mut!(TYPE_SINGLETON);
        (*t).slots = SINGLETON_SLOTS.as_ptr();
    });
}

pub fn type_singleton() -> &'static ObjType {
    init_type_singleton();
    unsafe { &*core::ptr::addr_of!(TYPE_SINGLETON) }
}
