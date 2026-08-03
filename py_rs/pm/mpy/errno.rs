//! Shared helpers for `pm_mpy_errno_*` accessors.
// symmetry: done

use super::export::module_global_export;
use crate::moderrno;
use crate::obj;

/// Look up `errno` module export `name` (null if absent).
pub(crate) fn errno_export(name: &str) -> obj::Obj {
    module_global_export(moderrno::init_module(), name)
}
