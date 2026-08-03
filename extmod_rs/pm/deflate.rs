//! Shared helpers for `pm_mpy_deflate_*` accessors.
// symmetry: done

use crate::moddeflate;
use py_rs::obj;
use py_rs::pm::mpy::module_global_export;

/// Look up `deflate` module export `name` (null if absent).
pub(crate) fn deflate_export(name: &str) -> obj::Obj {
    module_global_export(moddeflate::init_module(), name)
}
