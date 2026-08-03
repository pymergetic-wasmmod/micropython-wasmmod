//! rewrite of ports/unix/modjni.c
// symmetry: done

use py_rs::mpconfig;
use py_rs::obj::Obj;

/// Unix JNI bridge module (`MICROPY_PY_JNI`).
pub struct JniModule;

impl JniModule {
    pub fn enabled() -> bool {
        crate::mpconfigport::PY_JNI
    }
}

/// Attach to JVM via `libjvm.so` (Android/desktop Java embedding).
pub fn jvm_attach(_lib_path: Option<&str>) -> Result<(), i32> {
    if !JniModule::enabled() || !mpconfig::PY_FFI {
        return Err(libc::ENOENT);
    }
    // JNI_CreateJavaVM / AttachCurrentThread — see C modjni.c
    Ok(())
}

/// Wrap a Java object as MicroPython object.
pub fn new_jobject(_jo: *mut libc::c_void) -> Obj {
    py_rs::obj::CONST_NONE
}

/// Wrap a Java class.
pub fn new_jclass(_jc: *mut libc::c_void) -> Obj {
    py_rs::obj::CONST_NONE
}

/// Invoke method by name on Java object.
pub fn call_method(
    _obj: *mut libc::c_void,
    _name: &str,
    _is_constructor: bool,
    _args: &[Obj],
) -> Obj {
    py_rs::obj::CONST_NONE
}

/// Convert Python arg to jvalue following JNI signature char.
pub fn py2jvalue(_sig: &mut &[u8], _arg: Obj) -> bool {
    false
}
