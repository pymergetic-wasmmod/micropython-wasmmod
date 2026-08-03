//! Wired `pm_mpy_onewire_*` accessors.
// symmetry: done

use super::onewire::onewire_export;
use py_rs::pm::mpy::pm_mpy_obj_t;

#[no_mangle]
pub unsafe extern "C" fn pm_mpy_onewire_reset() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(onewire_export("reset"))
}

#[no_mangle]
pub unsafe extern "C" fn pm_mpy_onewire_readbit() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(onewire_export("readbit"))
}

#[no_mangle]
pub unsafe extern "C" fn pm_mpy_onewire_readbyte() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(onewire_export("readbyte"))
}

#[no_mangle]
pub unsafe extern "C" fn pm_mpy_onewire_writebit() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(onewire_export("writebit"))
}

#[no_mangle]
pub unsafe extern "C" fn pm_mpy_onewire_writebyte() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(onewire_export("writebyte"))
}

#[no_mangle]
pub unsafe extern "C" fn pm_mpy_onewire_crc8() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(onewire_export("crc8"))
}
