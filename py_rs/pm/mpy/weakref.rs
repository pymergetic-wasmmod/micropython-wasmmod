//! Shared helpers for `pm_mpy_weakref_*` accessors.
// symmetry: done

use super::export::module_global_export;
use crate::modweakref;
use crate::obj;

/// Look up `weakref` module export `name` (null if absent).
pub(crate) fn weakref_export(name: &str) -> obj::Obj {
    module_global_export(modweakref::init_module(), name)
}
