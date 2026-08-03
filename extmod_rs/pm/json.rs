//! Shared helpers for `pm_mpy_json_*` accessors.
// symmetry: done

use crate::modjson;
use py_rs::obj;
use py_rs::pm::mpy::module_global_export;

/// Look up `json` module export `name` (null if absent).
pub(crate) fn json_export(name: &str) -> obj::Obj {
    module_global_export(modjson::init_module(), name)
}
