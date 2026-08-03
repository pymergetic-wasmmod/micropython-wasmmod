//! rewrite of py/objnone.c
// symmetry: done

use crate::qstr;
use crate::mpconfig;
use crate::mpprint::{self, Print, PrintKind};
use crate::obj::{self, Obj, ObjBase, ObjType};

#[repr(C)]
pub struct ObjNone {
    pub base: ObjBase,
}

static TYPE_NONE: ObjType = obj::empty_type(0);

/// `none_print` from objnone.c
pub fn none_print(print: &Print, _self_in: Obj, kind: PrintKind) {
    if mpconfig::PY_JSON && kind == PrintKind::Json {
        mpprint::print_str(print, "null");
    } else {
        mpprint::print_str(print, "None");
    }
}

pub fn type_none() -> &'static ObjType {
    &TYPE_NONE
}
