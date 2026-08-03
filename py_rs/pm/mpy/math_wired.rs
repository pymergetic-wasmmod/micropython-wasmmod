//! Wired `pm_mpy_math_*` accessors.
// symmetry: done

use super::math::math_export;
use super::types::pm_mpy_obj_t;

/// `pm_mpy_math_e` — return the `e` export from `math`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_math_e() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(math_export("e"))
}

/// `pm_mpy_math_pi` — return the `pi` export from `math`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_math_pi() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(math_export("pi"))
}

/// `pm_mpy_math_tau` — return the `tau` export from `math`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_math_tau() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(math_export("tau"))
}

/// `pm_mpy_math_inf` — return the `inf` export from `math`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_math_inf() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(math_export("inf"))
}

/// `pm_mpy_math_nan` — return the `nan` export from `math`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_math_nan() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(math_export("nan"))
}

/// `pm_mpy_math_sqrt` — return the `sqrt` export from `math`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_math_sqrt() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(math_export("sqrt"))
}

/// `pm_mpy_math_pow` — return the `pow` export from `math`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_math_pow() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(math_export("pow"))
}

/// `pm_mpy_math_exp` — return the `exp` export from `math`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_math_exp() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(math_export("exp"))
}

/// `pm_mpy_math_expm1` — return the `expm1` export from `math`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_math_expm1() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(math_export("expm1"))
}

/// `pm_mpy_math_log` — return the `log` export from `math`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_math_log() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(math_export("log"))
}

/// `pm_mpy_math_log2` — return the `log2` export from `math`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_math_log2() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(math_export("log2"))
}

/// `pm_mpy_math_log10` — return the `log10` export from `math`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_math_log10() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(math_export("log10"))
}

/// `pm_mpy_math_cosh` — return the `cosh` export from `math`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_math_cosh() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(math_export("cosh"))
}

/// `pm_mpy_math_sinh` — return the `sinh` export from `math`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_math_sinh() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(math_export("sinh"))
}

/// `pm_mpy_math_tanh` — return the `tanh` export from `math`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_math_tanh() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(math_export("tanh"))
}

/// `pm_mpy_math_acosh` — return the `acosh` export from `math`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_math_acosh() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(math_export("acosh"))
}

/// `pm_mpy_math_asinh` — return the `asinh` export from `math`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_math_asinh() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(math_export("asinh"))
}

/// `pm_mpy_math_atanh` — return the `atanh` export from `math`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_math_atanh() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(math_export("atanh"))
}

/// `pm_mpy_math_cos` — return the `cos` export from `math`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_math_cos() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(math_export("cos"))
}

/// `pm_mpy_math_sin` — return the `sin` export from `math`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_math_sin() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(math_export("sin"))
}

/// `pm_mpy_math_tan` — return the `tan` export from `math`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_math_tan() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(math_export("tan"))
}

/// `pm_mpy_math_acos` — return the `acos` export from `math`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_math_acos() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(math_export("acos"))
}

/// `pm_mpy_math_asin` — return the `asin` export from `math`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_math_asin() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(math_export("asin"))
}

/// `pm_mpy_math_atan` — return the `atan` export from `math`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_math_atan() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(math_export("atan"))
}

/// `pm_mpy_math_atan2` — return the `atan2` export from `math`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_math_atan2() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(math_export("atan2"))
}

/// `pm_mpy_math_ceil` — return the `ceil` export from `math`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_math_ceil() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(math_export("ceil"))
}

/// `pm_mpy_math_copysign` — return the `copysign` export from `math`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_math_copysign() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(math_export("copysign"))
}

/// `pm_mpy_math_fabs` — return the `fabs` export from `math`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_math_fabs() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(math_export("fabs"))
}

/// `pm_mpy_math_floor` — return the `floor` export from `math`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_math_floor() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(math_export("floor"))
}

/// `pm_mpy_math_fmod` — return the `fmod` export from `math`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_math_fmod() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(math_export("fmod"))
}

/// `pm_mpy_math_frexp` — return the `frexp` export from `math`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_math_frexp() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(math_export("frexp"))
}

/// `pm_mpy_math_ldexp` — return the `ldexp` export from `math`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_math_ldexp() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(math_export("ldexp"))
}

/// `pm_mpy_math_modf` — return the `modf` export from `math`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_math_modf() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(math_export("modf"))
}

/// `pm_mpy_math_isfinite` — return the `isfinite` export from `math`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_math_isfinite() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(math_export("isfinite"))
}

/// `pm_mpy_math_isinf` — return the `isinf` export from `math`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_math_isinf() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(math_export("isinf"))
}

/// `pm_mpy_math_isnan` — return the `isnan` export from `math`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_math_isnan() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(math_export("isnan"))
}

/// `pm_mpy_math_isclose` — return the `isclose` export from `math`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_math_isclose() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(math_export("isclose"))
}

/// `pm_mpy_math_trunc` — return the `trunc` export from `math`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_math_trunc() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(math_export("trunc"))
}

/// `pm_mpy_math_radians` — return the `radians` export from `math`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_math_radians() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(math_export("radians"))
}

/// `pm_mpy_math_degrees` — return the `degrees` export from `math`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_math_degrees() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(math_export("degrees"))
}

/// `pm_mpy_math_factorial` — return the `factorial` export from `math`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_math_factorial() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(math_export("factorial"))
}

/// `pm_mpy_math_erf` — return the `erf` export from `math`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_math_erf() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(math_export("erf"))
}

/// `pm_mpy_math_erfc` — return the `erfc` export from `math`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_math_erfc() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(math_export("erfc"))
}

/// `pm_mpy_math_gamma` — return the `gamma` export from `math`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_math_gamma() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(math_export("gamma"))
}

/// `pm_mpy_math_lgamma` — return the `lgamma` export from `math`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_math_lgamma() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(math_export("lgamma"))
}
