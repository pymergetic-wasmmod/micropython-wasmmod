//! Shared helpers for `pm_mpy_machine_*` accessors.
// symmetry: done

use crate::modmachine;
use py_rs::obj;
use py_rs::pm::mpy::module_global_export;

/// Look up `machine` module export `name` (null if absent).
pub(crate) fn machine_export(name: &str) -> obj::Obj {
    module_global_export(modmachine::init_module(), name)
}
