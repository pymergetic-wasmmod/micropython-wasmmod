//! Shared helpers for `pm_mpy_marshal_*` accessors.
// symmetry: done

use crate::modmarshal;
use py_rs::obj;
use py_rs::pm::mpy::module_global_export;

/// Look up `marshal` module export `name` (null if absent).
pub(crate) fn marshal_export(name: &str) -> obj::Obj {
    module_global_export(modmarshal::init_module(), name)
}
