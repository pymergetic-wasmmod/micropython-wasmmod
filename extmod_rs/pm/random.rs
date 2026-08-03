//! Shared helpers for `pm_mpy_random_*` accessors.
// symmetry: done

use crate::modrandom;
use py_rs::obj;
use py_rs::pm::mpy::module_global_export;

/// Look up `random` module export `name` (null if absent).
pub(crate) fn random_export(name: &str) -> obj::Obj {
    module_global_export(modrandom::init_module(), name)
}
