//! Host pin HAL for bitbang buses (maps to virtpin / Pin protocol).
// symmetry: done

use py_rs::obj::Obj;
use py_rs::raise::{self, MpRaise};

use crate::virtpin::{self, has_pin_protocol};

/// `mp_hal_get_pin_obj` — any object exposing the pin protocol.
pub fn get_pin_obj(pin: Obj) -> Obj {
    if !has_pin_protocol(pin) {
        raise::raise(MpRaise::TypeError("pin"));
    }
    pin
}

/// `mp_hal_pin_read`
pub fn pin_read(pin: Obj) -> i32 {
    virtpin::virtual_pin_read(pin)
}

/// `mp_hal_pin_write`
pub fn pin_write(pin: Obj, v: i32) {
    virtpin::virtual_pin_write(pin, v);
}

/// `mp_hal_pin_output` — no-op on host (virtual pins have no direction register).
pub fn pin_output(_pin: Obj) {}

/// `mp_hal_pin_input` — no-op on host.
pub fn pin_input(_pin: Obj) {}

/// `mp_hal_pin_open_drain` — no-op on host (od_low/od_high drive via virtpin).
pub fn pin_open_drain(_pin: Obj) {}

/// `mp_hal_pin_od_low`
pub fn pin_od_low(pin: Obj) {
    pin_write(pin, 0);
}

/// `mp_hal_pin_od_high`
pub fn pin_od_high(pin: Obj) {
    pin_write(pin, 1);
}
