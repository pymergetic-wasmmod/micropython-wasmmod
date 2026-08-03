//! Shared helpers for `pm_mpy_uctypes_*` accessors.
// symmetry: done

use crate::moductypes;
use py_rs::obj;
use py_rs::pm::mpy::module_global_export;

/// Look up `uctypes` module export `name` (null if absent).
pub(crate) fn uctypes_export(name: &str) -> obj::Obj {
    module_global_export(moductypes::init_module(), name)
}
