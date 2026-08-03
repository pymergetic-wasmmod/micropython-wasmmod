//! Shared helpers for `pm_mpy_openamp_*` accessors.
// symmetry: done

use crate::modopenamp;
use py_rs::obj;
use py_rs::pm::mpy::module_global_export;

/// Look up `openamp` module export `name` (null if absent).
pub(crate) fn openamp_export(name: &str) -> obj::Obj {
    module_global_export(modopenamp::init_module(), name)
}
