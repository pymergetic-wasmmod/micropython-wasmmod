//! Module construction and registration for the public MetalPython ABI.
// symmetry: done

use crate::obj;
use crate::objdict::{self, ObjDict};
use crate::objmodule;
use crate::qstr;

use super::types::{pm_mpy_module_t, pm_mpy_obj_t, pm_mpy_qstr_t, pm_mpy_status_t};

/// Create a fresh module object (`pm::mpy::module_new`).
pub fn module_new(name: &str) -> pm_mpy_module_t {
    let module = objmodule::new_module(qstr::from_str(name));
    pm_mpy_module_t::from_obj(module)
}

/// Register a module in `sys.modules` (`pm::mpy::module_register`).
pub fn module_register(name: &str, module: pm_mpy_module_t) -> pm_mpy_status_t {
    objmodule::register_builtin_module(qstr::from_str(name), module.to_obj());
    pm_mpy_status_t::Ok
}

/// Fetch a registered module by name (`pm::mpy::module_get`).
pub fn module_get(name: &str) -> pm_mpy_module_t {
    let module = objmodule::module_get_builtin(qstr::from_str(name), false);
    pm_mpy_module_t::from_obj(module)
}

/// Set a module global (`pm::mpy::module_set_attr`).
pub fn module_set_attr(module: pm_mpy_module_t, attr: pm_mpy_qstr_t, value: pm_mpy_obj_t) -> pm_mpy_status_t {
    let globals = objmodule::module_get_globals(module.to_obj());
    objdict::dict_store(
        obj::from_ptr(globals as *const ObjDict as *const ()),
        obj::new_qstr(attr.to_qstr()),
        value.to_obj(),
    );
    pm_mpy_status_t::Ok
}

/// Get a module global (`pm::mpy::module_get_attr`).
pub fn module_get_attr(module: pm_mpy_module_t, attr: pm_mpy_qstr_t, out: &mut pm_mpy_obj_t) -> pm_mpy_status_t {
    let globals = objmodule::module_get_globals(module.to_obj());
    let value = objdict::dict_get(
        obj::from_ptr(globals as *const ObjDict as *const ()),
        obj::new_qstr(attr.to_qstr()),
    );
    *out = pm_mpy_obj_t::from_obj(value);
    pm_mpy_status_t::Ok
}

/// Return a module's globals dict (`pm::mpy::module_globals`).
pub fn module_globals(module: pm_mpy_module_t) -> pm_mpy_obj_t {
    let globals = objmodule::module_get_globals(module.to_obj());
    pm_mpy_obj_t::from_obj(obj::from_ptr(globals as *const ObjDict as *const ()))
}

/// Build a module from a fixed name/value map (`pm::mpy::module_from_map`).
pub fn module_from_map(name: &str, entries: &[(&str, pm_mpy_obj_t)]) -> pm_mpy_module_t {
    let module = module_new(name);
    for (key, value) in entries {
        let _ = module_set_attr(
            module,
            pm_mpy_qstr_t::from_qstr(qstr::from_str(key)),
            *value,
        );
    }
    module
}

#[no_mangle]
pub unsafe extern "C" fn pm_mpy_module_new(name: *const core::ffi::c_char) -> pm_mpy_module_t {
    if name.is_null() {
        return pm_mpy_module_t::NULL;
    }
    let c_str = unsafe { core::ffi::CStr::from_ptr(name) };
    let Ok(name) = c_str.to_str() else {
        return pm_mpy_module_t::NULL;
    };
    module_new(name)
}

#[no_mangle]
pub unsafe extern "C" fn pm_mpy_module_register(
    name: *const core::ffi::c_char,
    module: pm_mpy_module_t,
) -> pm_mpy_status_t {
    if name.is_null() {
        return pm_mpy_status_t::Value;
    }
    let c_str = unsafe { core::ffi::CStr::from_ptr(name) };
    let Ok(name) = c_str.to_str() else {
        return pm_mpy_status_t::Value;
    };
    module_register(name, module)
}

#[no_mangle]
pub unsafe extern "C" fn pm_mpy_module_get(name: *const core::ffi::c_char) -> pm_mpy_module_t {
    if name.is_null() {
        return pm_mpy_module_t::NULL;
    }
    let c_str = unsafe { core::ffi::CStr::from_ptr(name) };
    let Ok(name) = c_str.to_str() else {
        return pm_mpy_module_t::NULL;
    };
    module_get(name)
}

#[no_mangle]
pub unsafe extern "C" fn pm_mpy_module_set_attr(
    module: pm_mpy_module_t,
    attr: pm_mpy_qstr_t,
    value: pm_mpy_obj_t,
) -> pm_mpy_status_t {
    module_set_attr(module, attr, value)
}

#[no_mangle]
pub unsafe extern "C" fn pm_mpy_module_get_attr(
    module: pm_mpy_module_t,
    attr: pm_mpy_qstr_t,
    out: *mut pm_mpy_obj_t,
) -> pm_mpy_status_t {
    if out.is_null() {
        return pm_mpy_status_t::Value;
    }
    module_get_attr(module, attr, unsafe { &mut *out })
}

#[no_mangle]
pub extern "C" fn pm_mpy_module_globals(module: pm_mpy_module_t) -> pm_mpy_obj_t {
    module_globals(module)
}

#[no_mangle]
pub unsafe extern "C" fn pm_mpy_module_from_map(
    name: *const core::ffi::c_char,
    keys: *const *const core::ffi::c_char,
    values: *const pm_mpy_obj_t,
    n: usize,
) -> pm_mpy_module_t {
    if name.is_null() {
        return pm_mpy_module_t::NULL;
    }
    let c_str = unsafe { core::ffi::CStr::from_ptr(name) };
    let Ok(name) = c_str.to_str() else {
        return pm_mpy_module_t::NULL;
    };
    let mut entries = Vec::with_capacity(n);
    for i in 0..n {
        let key_ptr = if keys.is_null() {
            core::ptr::null()
        } else {
            unsafe { *keys.add(i) }
        };
        if key_ptr.is_null() {
            continue;
        }
        let key_c = unsafe { core::ffi::CStr::from_ptr(key_ptr) };
        let Ok(key) = key_c.to_str() else {
            continue;
        };
        let v = if values.is_null() {
            pm_mpy_obj_t::NULL
        } else {
            unsafe { *values.add(i) }
        };
        entries.push((key, v));
    }
    module_from_map(name, &entries)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modbuiltins;

    #[test]
    fn module_roundtrip() {
        crate::gc::init();
        crate::runtime::init();
        let builtins = pm_mpy_module_t::from_obj(modbuiltins::init_builtins_module());
        let mut len_out = pm_mpy_obj_t::NULL;
        assert_eq!(
            module_get_attr(
                builtins,
                pm_mpy_qstr_t::from_qstr(qstr::from_str("len")),
                &mut len_out,
            ),
            pm_mpy_status_t::Ok,
        );
        assert_ne!(len_out, pm_mpy_obj_t::NULL);
    }
}
