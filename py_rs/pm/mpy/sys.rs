//! Shared helpers for `pm_mpy_sys_*` accessors.
// symmetry: done

use crate::modsys;
use crate::obj;
use super::export::module_global_export;

/// Look up `sys` module export `name` (null if absent).
pub(crate) fn sys_export(name: &str) -> obj::Obj {
    module_global_export(modsys::init_module(), name)
}
