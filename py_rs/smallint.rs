//! rewrite of py/smallint.h + py/smallint.c
// symmetry: done

use crate::mpconfig;
use crate::obj::{self, Int, Uint, WORD_MSBIT_HIGH};

/// Smallest small-int for REPR_A (`MP_SMALL_INT_MIN`).
pub const MIN: Int = (WORD_MSBIT_HIGH as Int) >> 1;

/// Largest small-int (`MP_SMALL_INT_MAX`).
pub const MAX: Int = !MIN;

/// Mask to truncate mp_int_t to positive value (`MP_SMALL_INT_POSITIVE_MASK`, REPR_A).
pub const POSITIVE_MASK: Int = !(WORD_MSBIT_HIGH | (WORD_MSBIT_HIGH >> 1)) as Int;

/// Number of bits in a small int including sign (`MP_SMALL_INT_BITS`).
pub const BITS: u32 = (usize::BITS - 1) as u32;

#[inline]
pub fn fits(n: Int) -> bool {
    debug_assert!(mpconfig::OBJ_REPR == mpconfig::OBJ_REPR_A);
    // ((((n) ^ ((mp_uint_t)(n) << 1)) & MP_OBJ_WORD_MSBIT_HIGH) == 0)
    let xor = (n as Uint) ^ (((n as Uint) << 1) as Uint);
    (xor & WORD_MSBIT_HIGH) == 0
}

/// Python-style modulo: result has the sign of the divisor (`mp_small_int_modulo`).
pub fn modulo(mut dividend: Int, divisor: Int) -> Int {
    dividend %= divisor;
    if (dividend < 0 && divisor > 0) || (dividend > 0 && divisor < 0) {
        dividend += divisor;
    }
    dividend
}

/// Python floor division (`mp_small_int_floor_divide`).
pub fn floor_divide(mut num: Int, denom: Int) -> Int {
    if num >= 0 {
        if denom < 0 {
            num += -denom - 1;
        }
    } else if denom >= 0 {
        num += -denom + 1;
    }
    num / denom
}

#[inline]
pub fn as_obj(v: Int) -> obj::Obj {
    debug_assert!(fits(v));
    obj::new_small_int(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn floor_div_matches_python() {
        assert_eq!(floor_divide(7, 3), 2);
        assert_eq!(floor_divide(-7, 3), -3);
        assert_eq!(floor_divide(7, -3), -3);
        assert_eq!(floor_divide(-7, -3), 2);
    }

    #[test]
    fn mod_matches_python() {
        assert_eq!(modulo(7, 3), 1);
        assert_eq!(modulo(-7, 3), 2);
        assert_eq!(modulo(7, -3), -2);
        assert_eq!(modulo(-7, -3), -1);
    }
}
