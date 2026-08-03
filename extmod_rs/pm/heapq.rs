//! Shared helpers for `pm_mpy_heapq_*` accessors.
// symmetry: done

use crate::modheapq;
use py_rs::obj;
use py_rs::pm::mpy::module_global_export;

/// Look up `heapq` module export `name` (null if absent).
pub(crate) fn heapq_export(name: &str) -> obj::Obj {
    module_global_export(modheapq::init_module(), name)
}
