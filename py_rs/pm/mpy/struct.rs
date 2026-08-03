//! Shared helpers for `pm_mpy_struct_*` accessors.
// symmetry: done

use crate::modstruct;
use crate::obj;
use super::export::module_global_export;

/// Look up `struct` module export `name` (null if absent).
pub(crate) fn struct_export(name: &str) -> obj::Obj {
    module_global_export(modstruct::init_module(), name)
}
