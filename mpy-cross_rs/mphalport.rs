//! rewrite of mpy-cross/mphalport.h
// symmetry: done
//! Empty HAL pin type — upstream `#define mp_hal_pin_obj_t` prevents virtpin inclusion.

/// Placeholder pin object type (no HAL on mpy-cross host).
pub type MpHalPinObj = ();
