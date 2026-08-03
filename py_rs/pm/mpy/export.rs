//! Look up a name from a module globals table without raising.
// symmetry: done

use crate::obj;
use crate::objmodule;
use crate::map::{self, LookupKind};
use crate::qstr;

/// Return a module global by name, or [`obj::OBJ_NULL`] if absent.
pub fn module_global_export(module: obj::Obj, name: &str) -> obj::Obj {
    if module == obj::OBJ_NULL {
        return obj::OBJ_NULL;
    }
    let globals = objmodule::module_get_globals(module);
    let dict = unsafe { &mut *globals };
    map::lookup(&mut dict.map, obj::new_qstr(qstr::from_str(name)), LookupKind::Lookup)
        .map(|elem| elem.value)
        .unwrap_or(obj::OBJ_NULL)
}
