//! Shared helpers for `pm_mpy_btree_*` accessors.
// symmetry: done

use crate::modbtree;
use py_rs::obj;
use py_rs::pm::mpy::module_global_export;

/// Look up `btree` module export `name` (null if absent).
pub(crate) fn btree_export(name: &str) -> obj::Obj {
    module_global_export(modbtree::init_module(), name)
}
