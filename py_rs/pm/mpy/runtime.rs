//! Runtime init, call, and eval entry points for the public MetalPython ABI.
// symmetry: done

use crate::gc;
use crate::nlr::{self, NlrBuf};
use crate::obj;
use crate::runtime;

use super::types::{pm_mpy_obj_t, pm_mpy_status_t};

/// Ensure the GC and VM are initialised once before any other `pm_mpy_*` call.
static INIT: std::sync::Once = std::sync::Once::new();

fn ensure_init() {
    INIT.call_once(|| {
        gc::init();
        runtime::init();
        let _ = crate::modbuiltins::init_builtins_module();
    });
}

/// Rust-side runtime initialisation (`pm::mpy::runtime_init`).
pub fn runtime_init() -> pm_mpy_status_t {
    ensure_init();
    pm_mpy_status_t::Ok
}

/// Rust-side runtime shutdown (`pm::mpy::runtime_deinit`).
pub fn runtime_deinit() -> pm_mpy_status_t {
    runtime::deinit();
    pm_mpy_status_t::Ok
}

/// Call a callable with one positional argument, catching MicroPython exceptions.
pub fn runtime_call(fun: pm_mpy_obj_t, arg: pm_mpy_obj_t, out: &mut pm_mpy_obj_t) -> pm_mpy_status_t {
    ensure_init();
    let mut nlr_buf = NlrBuf::default();
    match nlr::protect(&mut nlr_buf, || runtime::call_function_1(fun.to_obj(), arg.to_obj())) {
        Ok(ret) => {
            *out = pm_mpy_obj_t::from_obj(ret);
            pm_mpy_status_t::Ok
        }
        Err(_) => pm_mpy_status_t::Runtime,
    }
}

/// Call a callable with positional and keyword arguments.
pub fn runtime_call_n_kw(
    fun: pm_mpy_obj_t,
    n_args: usize,
    n_kw: usize,
    args: *const pm_mpy_obj_t,
    out: &mut pm_mpy_obj_t,
) -> pm_mpy_status_t {
    ensure_init();
    if args.is_null() && (n_args + 2 * n_kw) != 0 {
        return pm_mpy_status_t::Value;
    }
    let mut arg_objs = Vec::with_capacity(n_args + 2 * n_kw);
    if !args.is_null() {
        let slice = unsafe { std::slice::from_raw_parts(args, n_args + 2 * n_kw) };
        for handle in slice {
            arg_objs.push(handle.to_obj());
        }
    }
    let mut nlr_buf = NlrBuf::default();
    match nlr::protect(&mut nlr_buf, || {
        runtime::call_function_n_kw(fun.to_obj(), n_args, n_kw, &arg_objs)
    }) {
        Ok(ret) => {
            *out = pm_mpy_obj_t::from_obj(ret);
            pm_mpy_status_t::Ok
        }
        Err(_) => pm_mpy_status_t::Runtime,
    }
}

/// Evaluate Python source and return the resulting object.
pub fn runtime_eval(src: &str, out: &mut pm_mpy_obj_t) -> pm_mpy_status_t {
    ensure_init();
    let mut nlr_buf = NlrBuf::default();
    match nlr::protect(&mut nlr_buf, || runtime::eval_str(src)) {
        Ok(ret) => {
            *out = pm_mpy_obj_t::from_obj(ret);
            pm_mpy_status_t::Ok
        }
        Err(_) => pm_mpy_status_t::Runtime,
    }
}

/// Execute Python source as a module body.
pub fn runtime_exec(src: &str) -> pm_mpy_status_t {
    ensure_init();
    let mut nlr_buf = NlrBuf::default();
    match nlr::protect(&mut nlr_buf, || runtime::exec_str(src)) {
        Ok(_) => pm_mpy_status_t::Ok,
        Err(_) => pm_mpy_status_t::Runtime,
    }
}

/// Execute Python source from a file path.
pub fn runtime_execfile(path: &str) -> pm_mpy_status_t {
    ensure_init();
    let mut nlr_buf = NlrBuf::default();
    let path_q = crate::qstr::from_str(path);
    match nlr::protect(&mut nlr_buf, || runtime::execfile(path_q)) {
        Ok(_) => pm_mpy_status_t::Ok,
        Err(_) => pm_mpy_status_t::Runtime,
    }
}

#[no_mangle]
pub extern "C" fn pm_mpy_runtime_init() -> pm_mpy_status_t {
    runtime_init()
}

#[no_mangle]
pub extern "C" fn pm_mpy_runtime_deinit() -> pm_mpy_status_t {
    runtime_deinit()
}

#[no_mangle]
pub unsafe extern "C" fn pm_mpy_runtime_call(
    fun: pm_mpy_obj_t,
    arg: pm_mpy_obj_t,
    out: *mut pm_mpy_obj_t,
) -> pm_mpy_status_t {
    if out.is_null() {
        return pm_mpy_status_t::Value;
    }
    runtime_call(fun, arg, &mut *out)
}

#[no_mangle]
pub unsafe extern "C" fn pm_mpy_runtime_call_n_kw(
    fun: pm_mpy_obj_t,
    n_args: usize,
    n_kw: usize,
    args: *const pm_mpy_obj_t,
    out: *mut pm_mpy_obj_t,
) -> pm_mpy_status_t {
    if out.is_null() {
        return pm_mpy_status_t::Value;
    }
    runtime_call_n_kw(fun, n_args, n_kw, args, &mut *out)
}

#[no_mangle]
pub unsafe extern "C" fn pm_mpy_runtime_eval(
    src: *const core::ffi::c_char,
    out: *mut pm_mpy_obj_t,
) -> pm_mpy_status_t {
    if src.is_null() || out.is_null() {
        return pm_mpy_status_t::Value;
    }
    let c_str = unsafe { core::ffi::CStr::from_ptr(src) };
    let Ok(src) = c_str.to_str() else {
        return pm_mpy_status_t::Value;
    };
    runtime_eval(src, &mut *out)
}

#[no_mangle]
pub unsafe extern "C" fn pm_mpy_runtime_exec(src: *const core::ffi::c_char) -> pm_mpy_status_t {
    if src.is_null() {
        return pm_mpy_status_t::Value;
    }
    let c_str = unsafe { core::ffi::CStr::from_ptr(src) };
    let Ok(src) = c_str.to_str() else {
        return pm_mpy_status_t::Value;
    };
    runtime_exec(src)
}

#[no_mangle]
pub unsafe extern "C" fn pm_mpy_runtime_execfile(path: *const core::ffi::c_char) -> pm_mpy_status_t {
    if path.is_null() {
        return pm_mpy_status_t::Value;
    }
    let c_str = unsafe { core::ffi::CStr::from_ptr(path) };
    let Ok(path) = c_str.to_str() else {
        return pm_mpy_status_t::Value;
    };
    runtime_execfile(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::obj;
    use crate::pm::mpy::builtins::builtins_export;

    #[test]
    fn runtime_init_and_call_len() {
        assert_eq!(runtime_init(), pm_mpy_status_t::Ok);
        let len_fn = builtins_export("len");
        assert_ne!(len_fn, obj::OBJ_NULL);
        let mut out = pm_mpy_obj_t::NULL;
        let status = runtime_call(
            pm_mpy_obj_t::from_obj(len_fn),
            pm_mpy_obj_t::from_obj(crate::objstr::new_str(b"abc")),
            &mut out,
        );
        assert_eq!(status, pm_mpy_status_t::Ok);
        assert_eq!(obj::small_int_value(out.to_obj()), 3);
    }

    #[test]
    fn runtime_eval_arithmetic() {
        assert_eq!(runtime_init(), pm_mpy_status_t::Ok);
        let mut out = pm_mpy_obj_t::NULL;
        assert_eq!(runtime_eval("1+2*3", &mut out), pm_mpy_status_t::Ok);
        assert_eq!(obj::small_int_value(out.to_obj()), 7);
    }
}
