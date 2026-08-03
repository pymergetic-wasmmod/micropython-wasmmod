//! Shared helpers for `pm_mpy_string_*` accessors.
// symmetry: done

use super::export::module_global_export;
use crate::modstring;
use crate::obj;

/// Look up `string` module export `name` (null if absent).
pub(crate) fn string_export(name: &str) -> obj::Obj {
    module_global_export(modstring::init_module(), name)
}
