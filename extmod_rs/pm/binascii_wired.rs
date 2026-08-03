//! Wired `pm_mpy_binascii_*` accessors.
// symmetry: done

use super::binascii::binascii_export;
use py_rs::pm::mpy::pm_mpy_obj_t;

#[no_mangle]
pub unsafe extern "C" fn pm_mpy_binascii_hexlify() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(binascii_export("hexlify"))
}

#[no_mangle]
pub unsafe extern "C" fn pm_mpy_binascii_unhexlify() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(binascii_export("unhexlify"))
}

#[no_mangle]
pub unsafe extern "C" fn pm_mpy_binascii_a2b_base64() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(binascii_export("a2b_base64"))
}

#[no_mangle]
pub unsafe extern "C" fn pm_mpy_binascii_b2a_base64() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(binascii_export("b2a_base64"))
}

#[no_mangle]
pub unsafe extern "C" fn pm_mpy_binascii_crc32() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(binascii_export("crc32"))
}
