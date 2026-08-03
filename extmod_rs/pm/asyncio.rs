//! Shared helpers for `pm_mpy_asyncio_*` accessors.
// symmetry: done

use crate::modasyncio;
use py_rs::obj;
use py_rs::pm::mpy::module_global_export;

/// Look up `_asyncio` module export `name` (null if absent).
pub(crate) fn asyncio_export(name: &str) -> obj::Obj {
    module_global_export(modasyncio::init_module(), name)
}
