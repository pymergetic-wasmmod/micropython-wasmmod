//! rewrite of ports/unix/modos.c
// symmetry: done

use py_rs::obj::Obj;
use py_rs::objstr;
use py_rs::raise::{self, MpRaise};
use std::ffi::CString;

fn errno() -> i32 {
    unsafe { *libc::__errno_location() }
}

/// `mp_os_getenv`
pub fn getenv(name: Obj, default: Option<Obj>) -> Obj {
    let key = CString::new(objstr::str_get_str(name)).unwrap_or_default();
    let val = unsafe { libc::getenv(key.as_ptr()) };
    if val.is_null() {
        return default.unwrap_or(py_rs::obj::CONST_NONE);
    }
    let s = unsafe { std::ffi::CStr::from_ptr(val) }.to_string_lossy();
    objstr::new_str(s.as_bytes())
}

/// `mp_os_putenv`
pub fn putenv(key: Obj, value: Obj) -> Obj {
    let k = CString::new(objstr::str_get_str(key)).unwrap_or_default();
    let v = CString::new(objstr::str_get_str(value)).unwrap_or_default();
    if unsafe { libc::setenv(k.as_ptr(), v.as_ptr(), 1) } == -1 {
        raise::raise(MpRaise::OSError(errno()));
    }
    py_rs::obj::CONST_NONE
}

/// `mp_os_unsetenv`
pub fn unsetenv(key: Obj) -> Obj {
    let k = CString::new(objstr::str_get_str(key)).unwrap_or_default();
    if unsafe { libc::unsetenv(k.as_ptr()) } == -1 {
        raise::raise(MpRaise::OSError(errno()));
    }
    py_rs::obj::CONST_NONE
}

/// `mp_os_system`
pub fn system(cmd: Obj) -> Obj {
    let c = CString::new(objstr::str_get_str(cmd)).unwrap_or_default();
    let r = unsafe { libc::system(c.as_ptr()) };
    if r == -1 {
        raise::raise(MpRaise::OSError(errno()));
    }
    py_rs::obj::new_small_int(r as isize)
}

/// `mp_os_errno`
pub fn os_errno(set: Option<i32>) -> Obj {
    match set {
        None => py_rs::obj::new_small_int(errno() as isize),
        Some(v) => {
            unsafe {
                *libc::__errno_location() = v;
            }
            py_rs::obj::CONST_NONE
        }
    }
}

/// Port `os` extras wired via `MICROPY_PY_OS_INCLUDEFILE`.
pub const EXTRA_GLOBAL_NAMES: &[&str] = &["getenv", "putenv", "unsetenv", "system", "errno"];
