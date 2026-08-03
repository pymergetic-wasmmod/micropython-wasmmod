//! Shared helpers for `pm_mpy_cryptolib_*` accessors.
// symmetry: done

use crate::modcryptolib;
use py_rs::obj;
use py_rs::pm::mpy::module_global_export;

/// Look up `cryptolib` module export `name` (null if absent).
pub(crate) fn cryptolib_export(name: &str) -> obj::Obj {
    module_global_export(modcryptolib::init_module(), name)
}
