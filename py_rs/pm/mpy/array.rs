//! Shared helpers for `pm_mpy_array_*` accessors.
// symmetry: done

use crate::modarray;
use crate::obj;
use super::export::module_global_export;

/// Look up `array` module export `name` (null if absent).
pub(crate) fn array_export(name: &str) -> obj::Obj {
    module_global_export(modarray::init_module(), name)
}
