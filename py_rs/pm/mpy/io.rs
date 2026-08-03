//! Shared helpers for `pm_mpy_io_*` accessors.
// symmetry: done

use crate::modio;
use crate::obj;
use super::export::module_global_export;

/// Look up `io` module export `name` (null if absent).
pub(crate) fn io_export(name: &str) -> obj::Obj {
    module_global_export(modio::init_module(), name)
}
