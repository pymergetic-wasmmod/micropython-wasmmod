//! Shared helpers for `pm_mpy_binascii_*` accessors.
// symmetry: done

use crate::modbinascii;
use py_rs::obj;
use py_rs::pm::mpy::module_global_export;

/// Look up `binascii` module export `name` (null if absent).
pub(crate) fn binascii_export(name: &str) -> obj::Obj {
    module_global_export(modbinascii::init_module(), name)
}
