//! Object constructors and attribute helpers for the public MetalPython ABI.
// symmetry: done

use crate::map::{self, LookupKind};
use crate::obj;
use crate::objdict;
use crate::objlist;
use crate::objstr;
use crate::objtuple;

use super::types::{pm_mpy_obj_t, pm_mpy_status_t};

/// Create a small integer object (`pm::mpy::obj_new_int`).
pub fn obj_new_int(value: i64) -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(obj::new_int(value as crate::obj::Int))
}

/// Create a str/bytes object from UTF-8 data (`pm::mpy::obj_new_str`).
pub fn obj_new_str(data: &[u8]) -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(objstr::new_str(data))
}

/// Create a bytes object (`pm::mpy::obj_new_bytes`).
pub fn obj_new_bytes(data: &[u8]) -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(objstr::new_bytes(data))
}

/// Create a bool object (`pm::mpy::obj_new_bool`).
pub fn obj_new_bool(value: bool) -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(obj::new_bool(value))
}

/// Create an empty or fixed-size list (`pm::mpy::obj_new_list`).
pub fn obj_new_list(n: usize, items: Option<&[pm_mpy_obj_t]>) -> pm_mpy_obj_t {
    let objs: Vec<obj::Obj> = items
        .map(|slice| slice.iter().map(|h| h.to_obj()).collect())
        .unwrap_or_default();
    pm_mpy_obj_t::from_obj(objlist::new_list(n, Some(&objs)))
}

/// Create an empty dict (`pm::mpy::obj_new_dict`).
pub fn obj_new_dict(n: usize) -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(objdict::new_dict(n))
}

/// Create a tuple (`pm::mpy::obj_new_tuple`).
pub fn obj_new_tuple(n: usize, items: Option<&[pm_mpy_obj_t]>) -> pm_mpy_obj_t {
    let objs: Vec<obj::Obj> = items
        .map(|slice| slice.iter().map(|h| h.to_obj()).collect())
        .unwrap_or_default();
    pm_mpy_obj_t::from_obj(objtuple::new_tuple(n, Some(&objs)))
}

/// Load an attribute (`pm::mpy::obj_getattr`).
pub fn obj_getattr(base: pm_mpy_obj_t, attr: super::types::pm_mpy_qstr_t, out: &mut pm_mpy_obj_t) -> pm_mpy_status_t {
    super::infra::obj_getattr_protected(base, attr, out)
}

/// Store an attribute (`pm::mpy::obj_setattr`).
pub fn obj_setattr(base: pm_mpy_obj_t, attr: super::types::pm_mpy_qstr_t, value: pm_mpy_obj_t) -> pm_mpy_status_t {
    super::infra::obj_setattr_protected(base, attr, value)
}

#[no_mangle]
pub extern "C" fn pm_mpy_obj_new_int(value: i64) -> pm_mpy_obj_t {
    obj_new_int(value)
}

#[no_mangle]
pub unsafe extern "C" fn pm_mpy_obj_new_str(data: *const u8, len: usize) -> pm_mpy_obj_t {
    if data.is_null() && len != 0 {
        return pm_mpy_obj_t::NULL;
    }
    let slice = if len == 0 {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(data, len) }
    };
    obj_new_str(slice)
}

#[no_mangle]
pub unsafe extern "C" fn pm_mpy_obj_new_bytes(data: *const u8, len: usize) -> pm_mpy_obj_t {
    if data.is_null() && len != 0 {
        return pm_mpy_obj_t::NULL;
    }
    let slice = if len == 0 {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(data, len) }
    };
    obj_new_bytes(slice)
}

#[no_mangle]
pub extern "C" fn pm_mpy_obj_new_bool(value: bool) -> pm_mpy_obj_t {
    obj_new_bool(value)
}

#[no_mangle]
pub extern "C" fn pm_mpy_obj_new_list(n: usize, items: *const pm_mpy_obj_t) -> pm_mpy_obj_t {
    let slice = if items.is_null() || n == 0 {
        None
    } else {
        Some(unsafe { std::slice::from_raw_parts(items, n) })
    };
    obj_new_list(n, slice)
}

#[no_mangle]
pub extern "C" fn pm_mpy_obj_new_dict(n: usize) -> pm_mpy_obj_t {
    obj_new_dict(n)
}

#[no_mangle]
pub extern "C" fn pm_mpy_obj_new_tuple(n: usize, items: *const pm_mpy_obj_t) -> pm_mpy_obj_t {
    let slice = if items.is_null() || n == 0 {
        None
    } else {
        Some(unsafe { std::slice::from_raw_parts(items, n) })
    };
    obj_new_tuple(n, slice)
}

#[no_mangle]
pub unsafe extern "C" fn pm_mpy_obj_getattr(
    base: pm_mpy_obj_t,
    attr: super::types::pm_mpy_qstr_t,
    out: *mut pm_mpy_obj_t,
) -> pm_mpy_status_t {
    if out.is_null() {
        return pm_mpy_status_t::Value;
    }
    obj_getattr(base, attr, unsafe { &mut *out })
}

#[no_mangle]
pub unsafe extern "C" fn pm_mpy_obj_setattr(
    base: pm_mpy_obj_t,
    attr: super::types::pm_mpy_qstr_t,
    value: pm_mpy_obj_t,
) -> pm_mpy_status_t {
    obj_setattr(base, attr, value)
}

/// Map lookup helper used by the public ABI (`pm::mpy::lookup`).
pub fn lookup(map_obj: pm_mpy_obj_t, index: pm_mpy_obj_t, out: &mut pm_mpy_obj_t) -> pm_mpy_status_t {
    let dict = map_obj.to_obj();
    let dict_ptr = objdict::dict_ptr(dict);
    let elem = unsafe {
        map::lookup(&mut (*dict_ptr).map, index.to_obj(), LookupKind::Lookup)
    };
    match elem {
        Some(e) => {
            *out = pm_mpy_obj_t::from_obj(e.value);
            pm_mpy_status_t::Ok
        }
        None => {
            *out = pm_mpy_obj_t::NULL;
            pm_mpy_status_t::Err
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn pm_mpy_lookup(
    map_obj: pm_mpy_obj_t,
    index: pm_mpy_obj_t,
    out: *mut pm_mpy_obj_t,
) -> pm_mpy_status_t {
    if out.is_null() {
        return pm_mpy_status_t::Value;
    }
    lookup(map_obj, index, unsafe { &mut *out })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn obj_new_int_roundtrip() {
        crate::gc::init();
        crate::runtime::init();
        let handle = obj_new_int(42);
        assert_eq!(obj::small_int_value(handle.to_obj()), 42);
    }
}
