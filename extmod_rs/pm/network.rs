//! Shared helpers for `pm_mpy_network_*` accessors.
// symmetry: done

use crate::modnetwork;
use py_rs::obj;
use py_rs::pm::mpy::module_global_export;

/// Look up `network` module export `name` (null if absent).
pub(crate) fn network_export(name: &str) -> obj::Obj {
    module_global_export(modnetwork::init_module(), name)
}
