//! Wired `pm_mpy_random_*` accessors.
// symmetry: done

use super::random::random_export;
use py_rs::pm::mpy::pm_mpy_obj_t;

/// `pm_mpy_random___init__` — return the `__init__` export from `random`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_random___init__() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(random_export("__init__"))
}

/// `pm_mpy_random_getrandbits` — return the `getrandbits` export from `random`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_random_getrandbits() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(random_export("getrandbits"))
}

/// `pm_mpy_random_seed` — return the `seed` export from `random`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_random_seed() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(random_export("seed"))
}

/// `pm_mpy_random_randrange` — return the `randrange` export from `random`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_random_randrange() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(random_export("randrange"))
}

/// `pm_mpy_random_randint` — return the `randint` export from `random`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_random_randint() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(random_export("randint"))
}

/// `pm_mpy_random_choice` — return the `choice` export from `random`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_random_choice() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(random_export("choice"))
}

/// `pm_mpy_random_random` — return the `random` export from `random`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_random_random() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(random_export("random"))
}

/// `pm_mpy_random_uniform` — return the `uniform` export from `random`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_random_uniform() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(random_export("uniform"))
}
