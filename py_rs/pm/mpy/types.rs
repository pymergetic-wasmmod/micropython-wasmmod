//! Opaque C ABI types for the MetalPython runtime façade.
// symmetry: done

use crate::obj::Obj;

/// Status codes returned by `pm_mpy_*` entry points (`pm_mpy_status_t`).
#[repr(C)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum pm_mpy_status_t {
    Ok = 0,
    Err = -1,
    Type = -2,
    Value = -3,
    Runtime = -4,
}

/// Opaque MicroPython object handle (`pm_mpy_obj_t`), distinct from upstream `mp_obj_t`.
#[repr(C)]
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct pm_mpy_obj_t {
    word: usize,
}

impl pm_mpy_obj_t {
    pub const NULL: Self = Self { word: 0 };

    pub fn from_obj(obj: Obj) -> Self {
        Self { word: obj.0 }
    }

    pub fn to_obj(self) -> Obj {
        Obj(self.word)
    }

    pub fn is_null(self) -> bool {
        self.word == 0
    }
}

/// Opaque module handle (`pm_mpy_module_t`).
#[repr(C)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct pm_mpy_module_t {
    word: usize,
}

impl pm_mpy_module_t {
    pub const NULL: Self = Self { word: 0 };

    pub fn from_obj(module: Obj) -> Self {
        Self { word: module.0 }
    }

    pub fn to_obj(self) -> Obj {
        Obj(self.word)
    }
}

/// Opaque interned-string handle (`pm_mpy_qstr_t`).
#[repr(C)]
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct pm_mpy_qstr_t {
    id: usize,
}

impl pm_mpy_qstr_t {
    pub const NULL: Self = Self { id: crate::qstr::QSTR_NULL };

    pub fn from_qstr(q: crate::qstr::Qstr) -> Self {
        Self { id: q }
    }

    pub fn to_qstr(self) -> crate::qstr::Qstr {
        self.id
    }
}
