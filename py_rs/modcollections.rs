//! rewrite of py/modcollections.c
// symmetry: done

use crate::bc::ModuleContext;
use crate::map::{self, MapElem};
use crate::malloc;
use crate::mpconfig;
use crate::obj::{self, Obj, ObjType};
use crate::objdeque;
use crate::objdict;
use crate::objmodule;
use crate::objnamedtuple;
use crate::qstr;

pub fn init_module() -> Obj {
    if !mpconfig::PY_COLLECTIONS {
        return obj::OBJ_NULL;
    }
    let mut table = vec![
        MapElem {
            key: obj::new_qstr(qstr::from_str("__name__")),
            value: obj::new_qstr(qstr::from_str("collections")),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("namedtuple")),
            value: objnamedtuple::namedtuple_obj(),
        },
    ];
    if mpconfig::PY_COLLECTIONS_DEQUE {
        table.push(MapElem {
            key: obj::new_qstr(qstr::from_str("deque")),
            value: obj::from_ptr(objdeque::type_deque() as *const ObjType as *const ()),
        });
    }
    if mpconfig::PY_COLLECTIONS_ORDEREDDICT {
        table.push(MapElem {
            key: obj::new_qstr(qstr::from_str("OrderedDict")),
            value: obj::from_ptr(objdict::type_dict() as *const ObjType as *const ()),
        });
    }
    let ctx = malloc::new_obj::<ModuleContext>().expect("collections module");
    let dict = objdict::new_dict(table.len());
    unsafe {
        map::init_fixed_table(&mut (*objdict::dict_ptr(dict)).map, table);
        (*ctx).module.base.type_ = objmodule::type_module();
        (*ctx).module.globals = objdict::dict_ptr(dict);
        (*ctx).constants = Default::default();
    }
    let module = obj::from_ptr(ctx as *const ModuleContext as *const ());
    objmodule::register_builtin_module(qstr::from_str("collections"), module);
    module
}
