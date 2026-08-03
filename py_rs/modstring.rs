//! rewrite of py/modstring.c
// symmetry: done

use crate::bc::ModuleContext;
use crate::map::{self, MapElem};
use crate::malloc;
use crate::mpconfig;
use crate::obj::{self, Obj};
use crate::objdict;
use crate::objmodule;
use crate::qstr;

pub fn init_module() -> Obj {
    if !mpconfig::PY_TSTRINGS {
        return obj::OBJ_NULL;
    }
    let table = vec![MapElem {
        key: obj::new_qstr(qstr::from_str("__name__")),
        value: obj::new_qstr(qstr::from_str("string")),
    }];
    let ctx = malloc::new_obj::<ModuleContext>().expect("string module");
    let dict = objdict::new_dict(table.len());
    unsafe {
        map::init_fixed_table(&mut (*objdict::dict_ptr(dict)).map, table);
        (*ctx).module.base.type_ = objmodule::type_module();
        (*ctx).module.globals = objdict::dict_ptr(dict);
        (*ctx).constants = Default::default();
    }
    let module = obj::from_ptr(ctx as *const ModuleContext as *const ());
    objmodule::register_builtin_module(qstr::from_str("string"), module);
    module
}
