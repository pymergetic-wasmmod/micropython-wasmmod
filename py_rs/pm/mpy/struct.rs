//! Shared helpers for `pm_mpy_struct_*` accessors.
// symmetry: done

use super::export::module_global_export;
use crate::modstruct;
use crate::obj;

/// Look up `struct` module export `name` (null if absent).
pub(crate) fn struct_export(name: &str) -> obj::Obj {
    module_global_export(modstruct::init_module(), name)
}
