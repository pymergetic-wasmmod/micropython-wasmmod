//! rewrite of extmod/modtls_axtls.c
//! AXTLS backend disabled on unix host (`MICROPY_SSL_AXTLS=0`); stub matches C `#if MICROPY_PY_SSL && MICROPY_SSL_AXTLS`.
// symmetry: done
use py_rs::mpconfig;
use py_rs::obj::{self, Obj};

/// Register built-in `tls` module (`MP_REGISTER_EXTENSIBLE_MODULE`).
pub fn init_module() -> Obj {
    if !mpconfig::PY_SSL {
        return obj::OBJ_NULL;
    }
    obj::OBJ_NULL
}
