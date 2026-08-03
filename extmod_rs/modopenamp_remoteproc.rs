//! rewrite of extmod/modopenamp_remoteproc.c + extmod/modopenamp_remoteproc.h
//! Host has no remoteproc HAL (firmware load, start/stop, resource table parsing).
//! `RemoteProc` lifecycle requires port OpenAMP remote processor driver.
// symmetry: done
use py_rs::bc::ModuleContext;
use py_rs::malloc;
use py_rs::map::{self, MapElem};
use py_rs::mpconfig;
use py_rs::obj::{self, Obj};
use py_rs::objdict;
use py_rs::objmodule;
use py_rs::qstr;

/// Register built-in `openamp_remoteproc` module (`MP_REGISTER_EXTENSIBLE_MODULE`).
pub fn init_module() -> Obj {
    if !mpconfig::PY_VFS {
        return obj::OBJ_NULL;
    }
    let table = [MapElem {
        key: obj::new_qstr(qstr::from_str("__name__")),
        value: obj::new_qstr(qstr::from_str("openamp_remoteproc")),
    }];
    let ctx = malloc::new_obj::<ModuleContext>().expect("openamp_remoteproc module");
    let dict = objdict::new_dict(table.len());
    unsafe {
        map::init_fixed_table(&mut (*objdict::dict_ptr(dict)).map, table.to_vec());
        (*ctx).module.base.type_ = objmodule::type_module();
        (*ctx).module.globals = objdict::dict_ptr(dict);
        (*ctx).constants = Default::default();
    }
    let module = obj::from_ptr(ctx as *const ModuleContext as *const ());
    objmodule::register_builtin_module(qstr::from_str("openamp_remoteproc"), module);
    module
}
