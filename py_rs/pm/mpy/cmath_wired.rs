//! Wired `pm_mpy_cmath_*` accessors.
// symmetry: done

use super::cmath::cmath_export;
use super::types::pm_mpy_obj_t;

/// `pm_mpy_cmath_e` — return the `e` export from `cmath`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_cmath_e() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(cmath_export("e"))
}

/// `pm_mpy_cmath_pi` — return the `pi` export from `cmath`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_cmath_pi() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(cmath_export("pi"))
}

/// `pm_mpy_cmath_phase` — return the `phase` export from `cmath`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_cmath_phase() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(cmath_export("phase"))
}

/// `pm_mpy_cmath_polar` — return the `polar` export from `cmath`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_cmath_polar() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(cmath_export("polar"))
}

/// `pm_mpy_cmath_rect` — return the `rect` export from `cmath`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_cmath_rect() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(cmath_export("rect"))
}

/// `pm_mpy_cmath_exp` — return the `exp` export from `cmath`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_cmath_exp() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(cmath_export("exp"))
}

/// `pm_mpy_cmath_log` — return the `log` export from `cmath`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_cmath_log() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(cmath_export("log"))
}

/// `pm_mpy_cmath_log10` — return the `log10` export from `cmath`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_cmath_log10() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(cmath_export("log10"))
}

/// `pm_mpy_cmath_sqrt` — return the `sqrt` export from `cmath`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_cmath_sqrt() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(cmath_export("sqrt"))
}

/// `pm_mpy_cmath_acos` — return the `acos` export from `cmath`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_cmath_acos() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(cmath_export("acos"))
}

/// `pm_mpy_cmath_asin` — return the `asin` export from `cmath`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_cmath_asin() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(cmath_export("asin"))
}

/// `pm_mpy_cmath_atan` — return the `atan` export from `cmath`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_cmath_atan() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(cmath_export("atan"))
}

/// `pm_mpy_cmath_cos` — return the `cos` export from `cmath`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_cmath_cos() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(cmath_export("cos"))
}

/// `pm_mpy_cmath_sin` — return the `sin` export from `cmath`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_cmath_sin() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(cmath_export("sin"))
}

/// `pm_mpy_cmath_tan` — return the `tan` export from `cmath`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_cmath_tan() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(cmath_export("tan"))
}

/// `pm_mpy_cmath_acosh` — return the `acosh` export from `cmath`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_cmath_acosh() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(cmath_export("acosh"))
}

/// `pm_mpy_cmath_asinh` — return the `asinh` export from `cmath`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_cmath_asinh() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(cmath_export("asinh"))
}

/// `pm_mpy_cmath_atanh` — return the `atanh` export from `cmath`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_cmath_atanh() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(cmath_export("atanh"))
}

/// `pm_mpy_cmath_cosh` — return the `cosh` export from `cmath`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_cmath_cosh() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(cmath_export("cosh"))
}

/// `pm_mpy_cmath_sinh` — return the `sinh` export from `cmath`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_cmath_sinh() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(cmath_export("sinh"))
}

/// `pm_mpy_cmath_tanh` — return the `tanh` export from `cmath`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_cmath_tanh() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(cmath_export("tanh"))
}

/// `pm_mpy_cmath_isfinite` — return the `isfinite` export from `cmath`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_cmath_isfinite() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(cmath_export("isfinite"))
}

/// `pm_mpy_cmath_isinf` — return the `isinf` export from `cmath`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_cmath_isinf() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(cmath_export("isinf"))
}

/// `pm_mpy_cmath_isnan` — return the `isnan` export from `cmath`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_cmath_isnan() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(cmath_export("isnan"))
}
