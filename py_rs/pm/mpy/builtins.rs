//! Shared helpers for `pm_mpy_builtins_*` accessors.
// symmetry: done

use crate::modbuiltins;
use crate::obj;
use crate::objdict::{self, ObjDict};
use crate::objmodule;
use crate::qstr;

/// Look up a name from the `builtins` module globals table.
pub(crate) fn builtins_export(name: &str) -> obj::Obj {
    let module = modbuiltins::init_builtins_module();
    let globals = objmodule::module_get_globals(module);
    objdict::dict_get(
        obj::from_ptr(globals as *const ObjDict as *const ()),
        obj::new_qstr(qstr::from_str(name)),
    )
}
