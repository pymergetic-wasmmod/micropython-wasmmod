//! rewrite of extmod/machine_i2s.c
// symmetry: gaps
// gaps:
// - needs I2S audio peripheral HAL (MCLK/BCLK/LRCK, DMA, sample formats)
// - `I2S` init/read/write/irq require port audio block driver
use py_rs::mpconfig;
use py_rs::obj::Obj;

/// Board-specific `machine_i2s` helpers — enabled with `feature = "machine"`.
#[cfg(feature = "machine")]
pub fn enabled() -> bool {
    mpconfig::PY_MACHINE
}

#[cfg(not(feature = "machine"))]
pub fn enabled() -> bool {
    false
}

/// Placeholder for port wiring of `machine_i2s` types.
pub fn init_types() -> Obj {
    if !mpconfig::PY_MACHINE {
        return Obj(0);
    }
    Obj(0)
}
