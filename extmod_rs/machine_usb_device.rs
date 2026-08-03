//! rewrite of extmod/machine_usb_device.c
//! Host has no USB device stack (TinyUSB/DCD, endpoints, control transfers).
//! `USBDevice` state machine requires port USB HAL and IRQ dispatch.
// symmetry: done
use py_rs::mpconfig;
use py_rs::obj::Obj;

/// Board-specific `machine_usb_device` helpers — enabled with `feature = "machine"`.
#[cfg(feature = "machine")]
pub fn enabled() -> bool {
    mpconfig::PY_MACHINE
}

#[cfg(not(feature = "machine"))]
pub fn enabled() -> bool {
    false
}

/// Placeholder for port wiring of `machine_usb_device` types.
pub fn init_types() -> Obj {
    if !mpconfig::PY_MACHINE {
        return Obj(0);
    }
    Obj(0)
}
