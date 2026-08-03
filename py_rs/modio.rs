//! rewrite of py/modio.c
// symmetry: done

use crate::bc::ModuleContext;
use crate::builtin;
use crate::map::{self, MapElem};
use crate::malloc;
use crate::mpconfig;
use crate::obj::{self, Obj};
use crate::objdict;
use crate::objmodule;
use crate::objstringio;
use crate::qstr;

pub fn init_module() -> Obj {
    if !mpconfig::PY_IO {
        return obj::OBJ_NULL;
    }
    let mut table = vec![
        MapElem {
            key: obj::new_qstr(qstr::from_str("__name__")),
            value: obj::new_qstr(qstr::from_str("io")),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("open")),
            value: obj::from_ptr(builtin::builtin_open as *const ()),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("StringIO")),
            value: obj::from_ptr(objstringio::type_stringio() as *const obj::ObjType as *const ()),
        },
    ];
    if mpconfig::PY_IO_BYTESIO {
        table.push(MapElem {
            key: obj::new_qstr(qstr::from_str("BytesIO")),
            value: obj::from_ptr(objstringio::type_bytesio() as *const obj::ObjType as *const ()),
        });
    }
    let ctx = malloc::new_obj::<ModuleContext>().expect("io module");
    let dict = objdict::new_dict(table.len());
    unsafe {
        map::init_fixed_table(&mut (*objdict::dict_ptr(dict)).map, table);
        (*ctx).module.base.type_ = objmodule::type_module();
        (*ctx).module.globals = objdict::dict_ptr(dict);
        (*ctx).constants = Default::default();
    }
    let module = obj::from_ptr(ctx as *const ModuleContext as *const ());
    objmodule::register_builtin_module(qstr::from_str("io"), module);
    module
}
