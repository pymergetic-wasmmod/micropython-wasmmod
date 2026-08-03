//! Shared helpers for `pm_mpy_select_*` accessors.
// symmetry: done

use crate::modselect;
use py_rs::obj;
use py_rs::pm::mpy::module_global_export;

/// Look up `select` module export `name` (null if absent).
pub(crate) fn select_export(name: &str) -> obj::Obj {
    module_global_export(modselect::init_module(), name)
}
