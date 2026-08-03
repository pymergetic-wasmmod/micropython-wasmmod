//! Shared helpers for `pm_mpy_micropython_*` accessors.
// symmetry: done

use super::export::module_global_export;
use crate::modmicropython;
use crate::obj;

/// Look up `micropython` module export `name` (null if absent).
pub(crate) fn micropython_export(name: &str) -> obj::Obj {
    module_global_export(modmicropython::init_module(), name)
}
