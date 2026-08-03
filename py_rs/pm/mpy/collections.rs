//! Shared helpers for `pm_mpy_collections_*` accessors.
// symmetry: done

use crate::modcollections;
use crate::obj;
use super::export::module_global_export;

/// Look up `collections` module export `name` (null if absent).
pub(crate) fn collections_export(name: &str) -> obj::Obj {
    module_global_export(modcollections::init_module(), name)
}
