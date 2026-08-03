//! Shared helpers for `pm_mpy_math_*` accessors.
// symmetry: done

use super::export::module_global_export;
use crate::modmath;
use crate::obj;

/// Look up `math` module export `name` (null if absent).
pub(crate) fn math_export(name: &str) -> obj::Obj {
    module_global_export(modmath::init_module(), name)
}
