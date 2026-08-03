//! rewrite of extmod/machine_adc.c
// symmetry: gaps
// gaps:
// - host has no ADC hardware; no soft mock (unlike WDT/PWM soft paths)
// - needs MCU ADC peripheral HAL (channel select, sampling, reference voltage)
// - `read`/`read_u16`/block/atten/width API require port `machine_adc` backend
use py_rs::mpconfig;
use py_rs::obj::Obj;

/// Board-specific `machine_adc` helpers — enabled with `feature = "machine"`.
#[cfg(feature = "machine")]
pub fn enabled() -> bool {
    mpconfig::PY_MACHINE
}

#[cfg(not(feature = "machine"))]
pub fn enabled() -> bool {
    false
}

/// Placeholder for port wiring of `machine_adc` types.
pub fn init_types() -> Obj {
    if !mpconfig::PY_MACHINE {
        return Obj(0);
    }
    Obj(0)
}
