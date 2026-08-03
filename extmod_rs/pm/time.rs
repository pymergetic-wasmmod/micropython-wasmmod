//! Shared helpers for `pm_mpy_time_*` accessors.
// symmetry: done

use crate::modtime;
use py_rs::obj;
use py_rs::pm::mpy::module_global_export;

/// Look up `time` module export `name` (null if absent).
pub(crate) fn time_export(name: &str) -> obj::Obj {
    module_global_export(modtime::init_module(), name)
}
