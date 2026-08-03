//! Shared helpers for `pm_mpy_onewire_*` accessors.
// symmetry: done

use crate::modonewire;
use py_rs::obj;
use py_rs::pm::mpy::module_global_export;

/// Look up `_onewire` module export `name` (null if absent).
pub(crate) fn onewire_export(name: &str) -> obj::Obj {
    module_global_export(modonewire::init_module(), name)
}
