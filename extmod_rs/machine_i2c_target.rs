//! rewrite of extmod/machine_i2c_target.c
//! Host has no I2C target/peripheral HAL (slave address match, clock stretch, FIFO).
//! `I2CTarget` read/write/irq paths require port slave-mode driver.
// symmetry: done
use py_rs::mpconfig;
use py_rs::obj::Obj;

/// Board-specific `machine_i2c_target` helpers — enabled with `feature = "machine"`.
#[cfg(feature = "machine")]
pub fn enabled() -> bool {
    mpconfig::PY_MACHINE
}

#[cfg(not(feature = "machine"))]
pub fn enabled() -> bool {
    false
}

/// Placeholder for port wiring of `machine_i2c_target` types.
pub fn init_types() -> Obj {
    if !mpconfig::PY_MACHINE {
        return Obj(0);
    }
    Obj(0)
}
