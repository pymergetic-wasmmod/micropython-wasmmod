//! rewrite of py/objcode.c + py/objcode.h
// symmetry: done

use crate::bc::ModuleConstants;
use crate::emitglue::{self, ProtoFun};
use crate::malloc;
use crate::mpconfig;
use crate::obj::{self, Obj, ObjBase, ObjType, TYPE_FLAG_NONE};

/// Code object at `MICROPY_PY_BUILTINS_CODE_BASIC` (`mp_obj_code_t`).
#[repr(C)]
pub struct ObjCode {
    pub base: ObjBase,
    pub constants: ModuleConstants,
    pub proto_fun: ProtoFun,
}

static mut TYPE: ObjType = obj::empty_type(0);

static TYPE_INIT: std::sync::OnceLock<()> = std::sync::OnceLock::new();

fn init_type() {
    TYPE_INIT.get_or_init(|| {
        unsafe {
            TYPE.name = crate::qstr::from_str("code");
        }
    });
}

pub fn type_code() -> &'static ObjType {
    init_type();
    unsafe { &TYPE }
}

/// `mp_obj_new_code` (basic level).
pub fn new_code(constants: ModuleConstants, proto_fun: ProtoFun) -> Obj {
    debug_assert!(mpconfig::PY_BUILTINS_CODE >= mpconfig::PY_BUILTINS_CODE_BASIC);
    let o = malloc::new_obj::<ObjCode>().expect("code alloc");
    unsafe {
        (*o).base.type_ = type_code() as *const ObjType;
        (*o).constants = constants;
        (*o).proto_fun = proto_fun;
        obj::from_ptr(o as *const ObjCode as *const ())
    }
}

/// `mp_code_get_constants`
pub fn code_get_constants(code: &ObjCode) -> &ModuleConstants {
    &code.constants
}

/// `mp_code_get_proto_fun`
pub fn code_get_proto_fun(code: &ObjCode) -> ProtoFun {
    code.proto_fun
}

pub fn obj_is_code(o: Obj) -> bool {
    obj::is_exact_type(o, type_code())
}

pub fn as_code(o: Obj) -> Option<&'static ObjCode> {
    if obj_is_code(o) {
        Some(unsafe { &*(obj::as_ptr(o) as *const ObjCode) })
    } else {
        None
    }
}
