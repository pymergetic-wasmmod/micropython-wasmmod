//! Shared helpers for `pm_mpy_websocket_*` accessors.
// symmetry: done

use crate::modwebsocket;
use py_rs::obj;
use py_rs::pm::mpy::module_global_export;

/// Look up `websocket` module export `name` (null if absent).
pub(crate) fn websocket_export(name: &str) -> obj::Obj {
    module_global_export(modwebsocket::init_module(), name)
}
