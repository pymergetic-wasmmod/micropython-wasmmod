//! Shared helpers for `pm_mpy_socket_*` accessors.
// symmetry: done

use crate::modsocket;
use py_rs::obj;
use py_rs::pm::mpy::module_global_export;

/// Look up `socket` module export `name` (null if absent).
pub(crate) fn socket_export(name: &str) -> obj::Obj {
    module_global_export(modsocket::init_module(), name)
}
