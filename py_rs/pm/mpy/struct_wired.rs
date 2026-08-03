//! Wired `pm_mpy_struct_*` accessors.
// symmetry: done

use super::r#struct::struct_export;
use super::types::pm_mpy_obj_t;

/// `pm_mpy_struct_calcsize` — return the `calcsize` export from `struct`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_struct_calcsize() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(struct_export("calcsize"))
}

/// `pm_mpy_struct_pack` — return the `pack` export from `struct`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_struct_pack() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(struct_export("pack"))
}

/// `pm_mpy_struct_pack_into` — return the `pack_into` export from `struct`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_struct_pack_into() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(struct_export("pack_into"))
}

/// `pm_mpy_struct_unpack` — return the `unpack` export from `struct`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_struct_unpack() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(struct_export("unpack"))
}

/// `pm_mpy_struct_unpack_from` — return the `unpack_from` export from `struct`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_struct_unpack_from() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(struct_export("unpack_from"))
}
