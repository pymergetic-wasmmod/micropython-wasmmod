//! Shared helpers for `pm_mpy_weakref_*` accessors.
// symmetry: done

use crate::modweakref;
use crate::obj;
use super::export::module_global_export;

/// Look up `weakref` module export `name` (null if absent).
pub(crate) fn weakref_export(name: &str) -> obj::Obj {
    module_global_export(modweakref::init_module(), name)
}
