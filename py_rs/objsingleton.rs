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

static TYPE_SINGLETON: ObjType = obj::empty_type(qstr::QSTR_NULL);

static ELLIPSIS: OnceLock<ObjSingleton> = OnceLock::new();
static NOT_IMPLEMENTED: OnceLock<ObjSingleton> = OnceLock::new();

fn ellipsis_obj() -> &'static ObjSingleton {
    ELLIPSIS.get_or_init(|| ObjSingleton {
        base: ObjBase {
            type_: &TYPE_SINGLETON as *const ObjType,
        },
        name: qstr::from_str("Ellipsis"),
    })
}

fn notimplemented_obj() -> Option<&'static ObjSingleton> {
    if mpconfig::PY_BUILTINS_NOTIMPLEMENTED {
        Some(NOT_IMPLEMENTED.get_or_init(|| ObjSingleton {
            base: ObjBase {
                type_: &TYPE_SINGLETON as *const ObjType,
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

pub fn type_singleton() -> &'static ObjType {
    &TYPE_SINGLETON
}
