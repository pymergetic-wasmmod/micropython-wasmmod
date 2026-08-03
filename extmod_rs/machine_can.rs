//! rewrite of extmod/machine_can.c + extmod/machine_can.h
// symmetry: gaps
// gaps:
// - needs CAN controller HAL (TX/RX mailboxes, filters, bus state)
// - `CAN` class requires port `machine_can` backend and `machine_can_port` IRQ hooks
use py_rs::mpconfig;
use py_rs::obj::Obj;

/// Board-specific `machine_can` helpers — enabled with `feature = "machine"`.
#[cfg(feature = "machine")]
pub fn enabled() -> bool {
    mpconfig::PY_MACHINE
}

#[cfg(not(feature = "machine"))]
pub fn enabled() -> bool {
    false
}

/// Placeholder for port wiring of `machine_can` types.
pub fn init_types() -> Obj {
    if !mpconfig::PY_MACHINE {
        return Obj(0);
    }
    Obj(0)
}
