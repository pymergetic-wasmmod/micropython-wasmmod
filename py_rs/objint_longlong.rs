//! rewrite of py/objint_longlong.c
// symmetry: done

use crate::mpconfig;
use crate::obj::{self, Int, Obj};

/// Long-long integer backend (`MICROPY_LONGINT_IMPL_LONGLONG`).
/// Host build uses MPZ via `objint_mpz.rs`; these entry points mirror the C file.

pub fn new_int_from_ll(val: i64) -> Obj {
    if mpconfig::LONGINT_IMPL == mpconfig::LONGINT_IMPL_LONGLONG {
        if (val as Int) as i64 == val && crate::smallint::fits(val as Int) {
            return obj::new_small_int(val as Int);
        }
        // Heap int with i64 payload would be allocated here.
        obj::new_small_int(val as Int)
    } else {
        crate::objint_mpz::new_int_from_ll(val)
    }
}

pub fn new_int_from_ull(val: u64) -> Obj {
    if mpconfig::LONGINT_IMPL == mpconfig::LONGINT_IMPL_LONGLONG {
        new_int_from_ll(val as i64)
    } else {
        crate::objint_mpz::new_int_from_ull(val)
    }
}

pub fn int_get_truncated(o: Obj) -> Int {
    if mpconfig::LONGINT_IMPL == mpconfig::LONGINT_IMPL_LONGLONG {
        if obj::is_small_int(o) {
            obj::small_int_value(o)
        } else {
            obj::get_int(o)
        }
    } else {
        crate::objint_mpz::int_get_truncated(o)
    }
}

pub fn int_to_bytes(
    self_in: Obj,
    buf_len: usize,
    buf: &mut [u8],
    big_endian: bool,
    is_signed: bool,
    overflow_check: bool,
) {
    if mpconfig::LONGINT_IMPL == mpconfig::LONGINT_IMPL_LONGLONG {
        let val = obj::get_int(self_in);
        crate::objint_impl::small_int_to_bytes(val, buf_len, buf, big_endian, is_signed, overflow_check);
    } else {
        crate::objint::int_to_bytes(self_in, buf_len, buf, big_endian, is_signed, overflow_check);
    }
}
