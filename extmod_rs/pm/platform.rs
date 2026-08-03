//! Shared helpers for `pm_mpy_platform_*` accessors.
// symmetry: done

use crate::modplatform;
use py_rs::obj;
use py_rs::pm::mpy::module_global_export;

/// Look up `platform` module export `name` (null if absent).
pub(crate) fn platform_export(name: &str) -> obj::Obj {
    module_global_export(modplatform::init_module(), name)
}
