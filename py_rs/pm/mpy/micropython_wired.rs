//! Wired `pm_mpy_micropython_*` accessors.
// symmetry: done

use super::micropython::micropython_export;
use super::types::pm_mpy_obj_t;

/// `pm_mpy_micropython_const` — return the `const` export from `micropython`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_micropython_const() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(micropython_export("const"))
}

/// `pm_mpy_micropython_opt_level` — return the `opt_level` export from `micropython`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_micropython_opt_level() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(micropython_export("opt_level"))
}

/// `pm_mpy_micropython_mem_total` — return the `mem_total` export from `micropython`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_micropython_mem_total() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(micropython_export("mem_total"))
}

/// `pm_mpy_micropython_mem_current` — return the `mem_current` export from `micropython`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_micropython_mem_current() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(micropython_export("mem_current"))
}

/// `pm_mpy_micropython_mem_peak` — return the `mem_peak` export from `micropython`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_micropython_mem_peak() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(micropython_export("mem_peak"))
}

/// `pm_mpy_micropython_mem_info` — return the `mem_info` export from `micropython`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_micropython_mem_info() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(micropython_export("mem_info"))
}

/// `pm_mpy_micropython_qstr_info` — return the `qstr_info` export from `micropython`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_micropython_qstr_info() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(micropython_export("qstr_info"))
}

/// `pm_mpy_micropython_stack_use` — return the `stack_use` export from `micropython`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_micropython_stack_use() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(micropython_export("stack_use"))
}

/// `pm_mpy_micropython_alloc_emergency_exception_buf` — return the `alloc_emergency_exception_buf` export from `micropython`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_micropython_alloc_emergency_exception_buf() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(micropython_export("alloc_emergency_exception_buf"))
}

/// `pm_mpy_micropython_pystack_use` — return the `pystack_use` export from `micropython`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_micropython_pystack_use() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(micropython_export("pystack_use"))
}

/// `pm_mpy_micropython_heap_lock` — return the `heap_lock` export from `micropython`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_micropython_heap_lock() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(micropython_export("heap_lock"))
}

/// `pm_mpy_micropython_heap_unlock` — return the `heap_unlock` export from `micropython`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_micropython_heap_unlock() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(micropython_export("heap_unlock"))
}

/// `pm_mpy_micropython_heap_locked` — return the `heap_locked` export from `micropython`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_micropython_heap_locked() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(micropython_export("heap_locked"))
}

/// `pm_mpy_micropython_kbd_intr` — return the `kbd_intr` export from `micropython`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_micropython_kbd_intr() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(micropython_export("kbd_intr"))
}

/// `pm_mpy_micropython_RingIO` — return the `RingIO` export from `micropython`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_micropython_RingIO() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(micropython_export("RingIO"))
}

/// `pm_mpy_micropython_schedule` — return the `schedule` export from `micropython`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_micropython_schedule() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(micropython_export("schedule"))
}
