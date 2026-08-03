//! Shared helpers for `pm_mpy_thread_*` accessors.
// symmetry: done

use crate::modthread;
use crate::obj;
use super::export::module_global_export;

/// Look up `_thread` module export `name` (null if absent).
pub(crate) fn thread_export(name: &str) -> obj::Obj {
    module_global_export(modthread::init_module(), name)
}
