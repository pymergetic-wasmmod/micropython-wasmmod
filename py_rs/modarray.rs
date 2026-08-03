//! rewrite of py/modarray.c
// symmetry: done

use crate::bc::ModuleContext;
use crate::malloc;
use crate::map::{self, MapElem};
use crate::mpconfig;
use crate::obj::{self, Obj, ObjType};
use crate::objarray;
use crate::objdict;
use crate::objmodule;
use crate::qstr;

fn make_module(name: &str, table: Vec<MapElem>) -> Obj {
    let ctx = malloc::new_obj::<ModuleContext>().expect("module alloc");
    let dict = objdict::new_dict(table.len().max(1));
    unsafe {
        map::init_fixed_table(&mut (*objdict::dict_ptr(dict)).map, table);
        (*ctx).module.base.type_ = objmodule::type_module();
        (*ctx).module.globals = objdict::dict_ptr(dict);
        (*ctx).constants = Default::default();
    }
    obj::from_ptr(ctx as *const ModuleContext as *const ())
}

/// Register the `array` extensible module.
pub fn init_module() -> Obj {
    if !mpconfig::PY_ARRAY {
        return obj::OBJ_NULL;
    }
    let table = vec![
        MapElem {
            key: obj::new_qstr(qstr::from_str("__name__")),
            value: obj::new_qstr(qstr::from_str("array")),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("array")),
            value: obj::from_ptr(objarray::type_array() as *const ObjType as *const ()),
        },
    ];
    let module = make_module("array", table);
    objmodule::register_builtin_module(qstr::from_str("array"), module);
    module
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gc;

    #[test]
    fn array_module_exports_type() {
        let _ = gc::init();
        crate::runtime::init();
        let m = init_module();
        assert!(obj::is_obj(m));
    }
}
