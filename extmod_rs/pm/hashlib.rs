//! Shared helpers for `pm_mpy_hashlib_*` accessors.
// symmetry: done

use crate::modhashlib;
use py_rs::obj;
use py_rs::pm::mpy::module_global_export;

/// Look up `hashlib` module export `name` (null if absent).
pub(crate) fn hashlib_export(name: &str) -> obj::Obj {
    module_global_export(modhashlib::init_module(), name)
}
