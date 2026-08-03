//! Shared helpers for `pm_mpy_re_*` accessors.
// symmetry: done

use crate::modre;
use py_rs::obj;
use py_rs::pm::mpy::module_global_export;

/// Look up `re` module export `name` (null if absent).
pub(crate) fn re_export(name: &str) -> obj::Obj {
    module_global_export(modre::init_module(), name)
}
