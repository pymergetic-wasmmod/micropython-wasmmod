//! Shared helpers for `pm_mpy_vfs_*` accessors.
// symmetry: done

use crate::modvfs;
use py_rs::obj;
use py_rs::pm::mpy::module_global_export;

/// Look up `vfs` module export `name` (null if absent).
pub(crate) fn vfs_export(name: &str) -> obj::Obj {
    module_global_export(modvfs::init_module(), name)
}
