//! Qstr and exception helpers for the public MetalPython ABI.
// symmetry: done

use crate::nlr::{self, NlrBuf};
use crate::obj;
use crate::qstr;
use crate::raise::{self, MpRaise};

use super::types::{pm_mpy_obj_t, pm_mpy_qstr_t, pm_mpy_status_t};

/// Intern a UTF-8 string (`pm::mpy::qstr_from_str`).
pub fn qstr_from_str(text: &str) -> pm_mpy_qstr_t {
    pm_mpy_qstr_t::from_qstr(qstr::from_str(text))
}

#[no_mangle]
pub unsafe extern "C" fn pm_mpy_qstr_from_str(text: *const core::ffi::c_char) -> pm_mpy_qstr_t {
    if text.is_null() {
        return pm_mpy_qstr_t::NULL;
    }
    let c_str = unsafe { core::ffi::CStr::from_ptr(text) };
    let Ok(text) = c_str.to_str() else {
        return pm_mpy_qstr_t::NULL;
    };
    qstr_from_str(text)
}

#[no_mangle]
pub unsafe extern "C" fn pm_mpy_exc_raise(exc: pm_mpy_obj_t) -> pm_mpy_status_t {
    raise::raise_obj(exc.to_obj());
}

#[no_mangle]
pub unsafe extern "C" fn pm_mpy_exc_raise_type_msg(
    kind: pm_mpy_status_t,
    msg: *const core::ffi::c_char,
) -> pm_mpy_status_t {
    let message = if msg.is_null() {
        ""
    } else {
        let c_str = unsafe { core::ffi::CStr::from_ptr(msg) };
        c_str.to_str().unwrap_or("")
    };
    match kind {
        pm_mpy_status_t::Type => raise::raise(MpRaise::TypeError(leak_msg(message))),
        pm_mpy_status_t::Value => raise::raise(MpRaise::ValueError(leak_msg(message))),
        _ => raise::raise(MpRaise::RuntimeError(leak_msg(message))),
    }
}

fn leak_msg(message: &str) -> &'static str {
    Box::leak(message.to_owned().into_boxed_str())
}

/// Protected attribute load for callers that need status codes instead of NLR jumps.
pub fn obj_getattr_protected(
    base: pm_mpy_obj_t,
    attr: pm_mpy_qstr_t,
    out: &mut pm_mpy_obj_t,
) -> pm_mpy_status_t {
    let mut nlr_buf = NlrBuf::default();
    match nlr::protect(&mut nlr_buf, || {
        crate::runtime::load_attr(base.to_obj(), attr.to_qstr())
    }) {
        Ok(value) => {
            *out = pm_mpy_obj_t::from_obj(value);
            pm_mpy_status_t::Ok
        }
        Err(_) => pm_mpy_status_t::Runtime,
    }
}

/// Protected attribute store for callers that need status codes instead of NLR jumps.
pub fn obj_setattr_protected(
    base: pm_mpy_obj_t,
    attr: pm_mpy_qstr_t,
    value: pm_mpy_obj_t,
) -> pm_mpy_status_t {
    let mut nlr_buf = NlrBuf::default();
    match nlr::protect(&mut nlr_buf, || {
        crate::runtime::store_attr(base.to_obj(), attr.to_qstr(), value.to_obj());
        obj::OBJ_NULL
    }) {
        Ok(_) => pm_mpy_status_t::Ok,
        Err(_) => pm_mpy_status_t::Runtime,
    }
}
