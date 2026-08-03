//! rewrite of shared/runtime/interrupt_char.c + shared/runtime/interrupt_char.h
// symmetry: done

use py_rs::mpconfig;

static mut MP_INTERRUPT_CHAR: i32 = -1;

/// Keyboard-interrupt character currently honoured by the HAL (`mp_interrupt_char`).
pub fn interrupt_char() -> i32 {
    unsafe { MP_INTERRUPT_CHAR }
}

/// `mp_hal_set_interrupt_char` — enable/disable Ctrl-C style interruption.
pub fn set_interrupt_char(c: i32) {
    if mpconfig::KBD_EXCEPTION {
        unsafe {
            MP_INTERRUPT_CHAR = c;
        }
    }
}
