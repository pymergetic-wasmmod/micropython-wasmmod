//! Shared helpers for `pm_mpy_gc_*` accessors.
// symmetry: done

use crate::modgc;
use crate::obj;
use super::export::module_global_export;

/// Look up `gc` module export `name` (null if absent).
pub(crate) fn gc_export(name: &str) -> obj::Obj {
    module_global_export(modgc::init_module(), name)
}
