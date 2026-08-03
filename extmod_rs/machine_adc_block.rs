//! rewrite of extmod/machine_adc_block.c
//! Host has no ADC block HAL (multi-channel sequencer, DMA, shared reference).
//! Block-level `init`/`read` require port-specific ADC controller driver.
// symmetry: done
use py_rs::mpconfig;
use py_rs::obj::Obj;

/// Board-specific `machine_adc_block` helpers — enabled with `feature = "machine"`.
#[cfg(feature = "machine")]
pub fn enabled() -> bool {
    mpconfig::PY_MACHINE
}

#[cfg(not(feature = "machine"))]
pub fn enabled() -> bool {
    false
}

/// Placeholder for port wiring of `machine_adc_block` types.
pub fn init_types() -> Obj {
    if !mpconfig::PY_MACHINE {
        return Obj(0);
    }
    Obj(0)
}
