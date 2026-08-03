//! Wired `pm_mpy_tls_*` accessors.
// symmetry: done

use super::tls::tls_export;
use py_rs::pm::mpy::pm_mpy_obj_t;

/// `pm_mpy_tls_SSLContext` — return the `SSLContext` export from `tls`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_tls_SSLContext() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(tls_export("SSLContext"))
}

/// `pm_mpy_tls_PROTOCOL_TLS_CLIENT` — return the `PROTOCOL_TLS_CLIENT` export from `tls`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_tls_PROTOCOL_TLS_CLIENT() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(tls_export("PROTOCOL_TLS_CLIENT"))
}

/// `pm_mpy_tls_PROTOCOL_TLS_SERVER` — return the `PROTOCOL_TLS_SERVER` export from `tls`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_tls_PROTOCOL_TLS_SERVER() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(tls_export("PROTOCOL_TLS_SERVER"))
}

/// `pm_mpy_tls_CERT_NONE` — return the `CERT_NONE` export from `tls`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_tls_CERT_NONE() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(tls_export("CERT_NONE"))
}
