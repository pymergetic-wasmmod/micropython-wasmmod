//! rewrite of py/objint_impl.h
// symmetry: done

use crate::binary;
use crate::misc::Byte;
use crate::obj::{self, Int};
use crate::raise::{self, MpRaise};

pub fn raise_to_bytes_overflow(nbytes: usize) -> ! {
    raise::raise(MpRaise::OverflowError("value would overflow buffer"));
}

pub fn raise_unsigned_negative_overflow() -> ! {
    raise::raise(MpRaise::OverflowError("can't convert negative int to unsigned"));
}

fn small_int_buffer_overflow_check(val: Int, nbytes: usize, is_signed: bool) {
    if val == 0 { return; }
    if !is_signed && val < 0 { raise_unsigned_negative_overflow(); }
    if nbytes >= std::mem::size_of::<Int>() { return; }
    if nbytes == 0 { raise_to_bytes_overflow(nbytes); }
    if is_signed {
        let edge = 1i64 << (nbytes * 8 - 1);
        if (-edge..edge).contains(&(val as i64)) { return; }
    } else if val >= 0 {
        let edge = 1i64 << (nbytes * 8);
        if (val as i64) < edge { return; }
    }
    raise_to_bytes_overflow(nbytes);
}

pub fn small_int_to_bytes(val: Int, buf_len: usize, buf: &mut [Byte], big_endian: bool, is_signed: bool, overflow_check: bool) {
    if overflow_check {
        small_int_buffer_overflow_check(val, buf_len, is_signed);
    }
    binary::set_int_signed(buf_len, buf, val, big_endian);
}
