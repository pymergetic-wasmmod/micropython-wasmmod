//! Shared helpers for `pm_mpy_bluetooth_*` accessors.
// symmetry: done

use crate::modbluetooth;
use py_rs::obj;
use py_rs::pm::mpy::module_global_export;

/// Look up `bluetooth` module export `name` (null if absent).
pub(crate) fn bluetooth_export(name: &str) -> obj::Obj {
    module_global_export(modbluetooth::init_module(), name)
}
