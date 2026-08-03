//! Shared helpers for `pm_mpy_tls_*` accessors.
// symmetry: done

use crate::modtls_mbedtls;
use py_rs::obj;
use py_rs::pm::mpy::module_global_export;

/// Look up `tls` module export `name` (null if absent).
pub(crate) fn tls_export(name: &str) -> obj::Obj {
    module_global_export(modtls_mbedtls::init_module(), name)
}
