//! Shared helpers for `pm_mpy_lwip_*` accessors.
// symmetry: done

use crate::modlwip;
use py_rs::obj;
use py_rs::pm::mpy::module_global_export;

/// Look up `lwip` module export `name` (null if absent).
pub(crate) fn lwip_export(name: &str) -> obj::Obj {
    module_global_export(modlwip::init_module(), name)
}
