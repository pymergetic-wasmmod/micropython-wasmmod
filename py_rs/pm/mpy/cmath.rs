//! Shared helpers for `pm_mpy_cmath_*` accessors.
// symmetry: done

use super::export::module_global_export;
use crate::modcmath;
use crate::obj;

/// Look up `cmath` module export `name` (null if absent).
pub(crate) fn cmath_export(name: &str) -> obj::Obj {
    module_global_export(modcmath::init_module(), name)
}
