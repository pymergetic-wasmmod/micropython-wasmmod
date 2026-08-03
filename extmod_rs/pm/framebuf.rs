//! Shared helpers for `pm_mpy_framebuf_*` accessors.
// symmetry: done

use crate::modframebuf;
use py_rs::obj;
use py_rs::pm::mpy::module_global_export;

/// Look up `framebuf` module export `name` (null if absent).
pub(crate) fn framebuf_export(name: &str) -> obj::Obj {
    module_global_export(modframebuf::init_module(), name)
}
