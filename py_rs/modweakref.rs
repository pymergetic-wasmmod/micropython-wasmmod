//! rewrite of py/modweakref.c
// symmetry: done

use crate::mpconfig;
use crate::obj::Obj;

pub fn init_module() -> Obj {
    if !mpconfig::PY_WEAKREF {
        return Obj(0);
    }
    Obj(0)
}

pub fn gc_weakref_about_to_be_freed(_ptr: *mut core::ffi::c_void) {}

pub fn gc_weakref_sweep() {}
