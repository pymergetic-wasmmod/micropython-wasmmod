//! Shared helpers for `pm_mpy_webrepl_*` accessors.
// symmetry: done

use crate::modwebrepl;
use py_rs::obj;
use py_rs::pm::mpy::module_global_export;

/// Look up `_webrepl` module export `name` (null if absent).
pub(crate) fn webrepl_export(name: &str) -> obj::Obj {
    module_global_export(modwebrepl::init_module(), name)
}
