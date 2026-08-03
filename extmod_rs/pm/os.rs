//! Shared helpers for `pm_mpy_os_*` accessors.
// symmetry: done

use crate::modos;
use py_rs::obj;
use py_rs::pm::mpy::module_global_export;

/// Look up `os` module export `name` (null if absent).
pub(crate) fn os_export(name: &str) -> obj::Obj {
    module_global_export(modos::init_module(), name)
}
