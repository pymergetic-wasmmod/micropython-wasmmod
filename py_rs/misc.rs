//! rewrite of py/misc.h (+ misc helpers from py/misc.c as needed)
// symmetry: done

use crate::mpconfig;

pub type Byte = u8;
/// MicroPython `uint` (platform unsigned int; host = 32-bit).
pub type Uint = u32;

// --- generic min/max (MicroPython MIN/MAX) ---

#[inline]
pub const fn min_i32(a: i32, b: i32) -> i32 {
    if a < b {
        a
    } else {
        b
    }
}

#[inline]
pub const fn max_i32(a: i32, b: i32) -> i32 {
    if a > b {
        a
    } else {
        b
    }
}

#[inline]
pub const fn min_u32(a: u32, b: u32) -> u32 {
    if a < b {
        a
    } else {
        b
    }
}

#[inline]
pub const fn max_u32(a: u32, b: u32) -> u32 {
    if a > b {
        a
    } else {
        b
    }
}

#[inline]
pub const fn min_usize(a: usize, b: usize) -> usize {
    if a < b {
        a
    } else {
        b
    }
}

#[inline]
pub const fn max_usize(a: usize, b: usize) -> usize {
    if a > b {
        a
    } else {
        b
    }
}

#[inline]
pub const fn min_isize(a: isize, b: isize) -> isize {
    if a < b {
        a
    } else {
        b
    }
}

#[inline]
pub const fn max_isize(a: isize, b: isize) -> isize {
    if a > b {
        a
    } else {
        b
    }
}

/// MicroPython `MP_STRINGIFY(x)` — use Rust's `stringify!` at call sites, or this alias.
#[macro_export]
macro_rules! mp_stringify {
    ($x:expr) => {
        ::core::stringify!($x)
    };
}

/// Compile-time assertion (`MP_STATIC_ASSERT`).
#[macro_export]
macro_rules! static_assert {
    ($cond:expr) => {
        const _: () = assert!($cond);
    };
}

#[inline]
pub const fn ceil_divide(a: usize, b: usize) -> usize {
    debug_assert!(b > 0);
    (a + b - 1) / b
}

#[inline]
pub const fn round_divide(a: usize, b: usize) -> usize {
    debug_assert!(b > 0);
    (a + b / 2) / b
}

/// MicroPython `MP_ARRAY_SIZE(a)`.
#[macro_export]
macro_rules! array_size {
    ($a:expr) => {
        $a.len()
    };
}

/// Align `ptr` down to `alignment` boundary (power of two).
#[inline]
pub const fn align_ptr(ptr: usize, alignment: usize) -> usize {
    debug_assert!(alignment.is_power_of_two());
    ptr & !(alignment - 1)
}

/// Round `n` up to a multiple of `alignment` (`MP_ALIGN`).
#[inline]
pub const fn align_up(n: usize, alignment: usize) -> usize {
    debug_assert!(alignment.is_power_of_two());
    (n + alignment - 1) & !(alignment - 1)
}

// --- unichar / UTF-8 ---

/// Unicode code unit (`unichar` in misc.h). Host unix defaults enable unicode (0x10ffff).
pub type Unichar = u32;

#[inline]
pub const fn utf8_is_nonascii(ch: u8) -> bool {
    (ch & 0x80) != 0
}

#[inline]
pub const fn utf8_is_cont(ch: u8) -> bool {
    (ch & 0xC0) == 0x80
}

#[inline]
pub fn utf8_get_char(s: &[u8]) -> Unichar {
    if !mpconfig::PY_BUILTINS_STR_UNICODE {
        return s.first().copied().unwrap_or(0) as Unichar;
    }
    let mut i = 0;
    let mut ord = s[i] as Unichar;
    i += 1;
    if !utf8_is_nonascii(ord as u8) {
        return ord;
    }
    ord &= 0x7F;
    let mut mask: Unichar = 0x40;
    while ord & mask != 0 {
        ord &= !mask;
        mask >>= 1;
    }
    while i < s.len() && utf8_is_cont(s[i]) {
        ord = (ord << 6) | ((s[i] & 0x3F) as Unichar);
        i += 1;
    }
    ord
}

#[inline]
pub fn utf8_next_char(s: &[u8]) -> &[u8] {
    if !mpconfig::PY_BUILTINS_STR_UNICODE {
        return &s[1.min(s.len())..];
    }
    let mut i = 1;
    while i < s.len() && utf8_is_cont(s[i]) {
        i += 1;
    }
    &s[i..]
}

#[inline]
pub fn utf8_charlen(str: &[u8], len: usize) -> usize {
    if !mpconfig::PY_BUILTINS_STR_UNICODE {
        return len;
    }
    let top = len.min(str.len());
    let mut charlen = 0;
    let mut i = 0;
    while i < top {
        if !utf8_is_cont(str[i]) {
            charlen += 1;
        }
        i += 1;
    }
    charlen
}

// ASCII attribute table from py/unicode.c (128 entries).
const ATTR: [u8; 128] = [
    0, 0, 0, 0, 0, 0, 0, 0, 0, 3, 3, 3, 3, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    3, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0x45, 0x45, 0x45, 0x45, 0x45, 0x45, 0x45, 0x45,
    0x45, 0x45, 1, 1, 1, 1, 1, 1, 1, 0x59, 0x59, 0x59, 0x59, 0x59, 0x59, 0x19, 0x19, 0x19, 0x19,
    0x19, 0x19, 0x19, 0x19, 0x19, 0x19, 0x19, 0x19, 0x19, 0x19, 0x19, 0x19, 0x19, 0x19, 0x19, 0x19,
    1, 1, 1, 1, 1, 1, 0x69, 0x69, 0x69, 0x69, 0x69, 0x69, 0x29, 0x29, 0x29, 0x29, 0x29, 0x29, 0x29,
    0x29, 0x29, 0x29, 0x29, 0x29, 0x29, 0x29, 0x29, 0x29, 0x29, 0x29, 0x29, 0x29, 1, 1, 1, 1, 0,
];

const FL_SPACE: u8 = 0x02;
const FL_DIGIT: u8 = 0x04;
const FL_ALPHA: u8 = 0x08;
const FL_UPPER: u8 = 0x10;
const FL_LOWER: u8 = 0x20;
const FL_XDIGIT: u8 = 0x40;

#[inline]
pub fn unichar_isspace(c: Unichar) -> bool {
    (c as u32) < 128 && (ATTR[c as usize] & FL_SPACE) != 0
}

#[inline]
pub fn unichar_isalpha(c: Unichar) -> bool {
    (c as u32) < 128 && (ATTR[c as usize] & FL_ALPHA) != 0
}

#[inline]
pub fn unichar_isprint(c: Unichar) -> bool {
    (c as u32) < 128 && (ATTR[c as usize] & 0x01) != 0
}

#[inline]
pub fn unichar_isdigit(c: Unichar) -> bool {
    (c as u32) < 128 && (ATTR[c as usize] & FL_DIGIT) != 0
}

#[inline]
pub fn unichar_isxdigit(c: Unichar) -> bool {
    (c as u32) < 128 && (ATTR[c as usize] & FL_XDIGIT) != 0
}

#[inline]
pub fn unichar_isident(c: Unichar) -> bool {
    (c as u32) < 128 && ((ATTR[c as usize] & (FL_ALPHA | FL_DIGIT)) != 0 || c == b'_' as Unichar)
}

#[inline]
pub fn unichar_isalnum(c: Unichar) -> bool {
    (c as u32) < 128 && (ATTR[c as usize] & (FL_ALPHA | FL_DIGIT)) != 0
}

#[inline]
pub fn unichar_isupper(c: Unichar) -> bool {
    (c as u32) < 128 && (ATTR[c as usize] & FL_UPPER) != 0
}

#[inline]
pub fn unichar_islower(c: Unichar) -> bool {
    (c as u32) < 128 && (ATTR[c as usize] & FL_LOWER) != 0
}

#[inline]
pub fn unichar_tolower(c: Unichar) -> Unichar {
    if unichar_isupper(c) {
        c + 0x20
    } else {
        c
    }
}

#[inline]
pub fn unichar_toupper(c: Unichar) -> Unichar {
    if unichar_islower(c) {
        c - 0x20
    } else {
        c
    }
}

#[inline]
pub fn unichar_xdigit_value(c: Unichar) -> usize {
    let mut n = c as usize - b'0' as usize;
    if n > 9 {
        n &= !(b'a' as usize - b'A' as usize);
        n -= b'A' as usize - (b'9' as usize + 1);
    }
    n
}

// --- bit utilities (mp_clz, mp_ctz, mp_popcount, bswap) ---

#[inline]
pub fn mp_clz(x: u32) -> u32 {
    x.leading_zeros()
}

#[inline]
pub fn mp_clzl(x: u64) -> u32 {
    x.leading_zeros()
}

#[inline]
pub fn mp_clzll(x: u64) -> u32 {
    x.leading_zeros()
}

#[inline]
pub fn mp_ctz(x: u32) -> u32 {
    x.trailing_zeros()
}

#[inline]
pub fn mp_popcount(x: u32) -> u32 {
    x.count_ones()
}

#[inline]
pub const fn mp_bswap16(x: u16) -> u16 {
    x.swap_bytes()
}

#[inline]
pub const fn mp_bswap32(x: u32) -> u32 {
    x.swap_bytes()
}

#[inline]
pub const fn mp_bswap64(x: u64) -> u64 {
    x.swap_bytes()
}

#[inline]
pub const fn fit_unsigned(bits: u32, value: u32) -> bool {
    (value & (!0u32 << bits)) == 0
}

#[inline]
pub const fn fit_signed(bits: u32, value: i32) -> bool {
    fit_unsigned(bits - 1, value as u32)
        || ((value as u32) & (!0u32 << (bits - 1))) == (!0u32 << (bits - 1))
}

// --- float internals (when MICROPY_PY_BUILTINS_FLOAT) ---

pub const FLOAT_EXP_BITS: u32 = if mpconfig::FLOAT_IMPL == mpconfig::FLOAT_IMPL_DOUBLE {
    11
} else {
    8
};

pub const FLOAT_EXP_OFFSET: u32 = if mpconfig::FLOAT_IMPL == mpconfig::FLOAT_IMPL_DOUBLE {
    1023
} else {
    127
};

pub const FLOAT_FRAC_BITS: u32 = if mpconfig::FLOAT_IMPL == mpconfig::FLOAT_IMPL_DOUBLE {
    52
} else {
    23
};

pub const FLOAT_EXP_BIAS: u32 = (1 << (FLOAT_EXP_BITS - 1)) - 1;

// --- overflow-checked arithmetic ---

#[inline]
pub fn mp_mul_mp_int_t_overflow(
    x: crate::obj::Int,
    y: crate::obj::Int,
    res: &mut crate::obj::Int,
) -> bool {
    if x > 0 {
        if y > 0 {
            if x > (crate::mpconfig::INT_MAX as crate::obj::Int / y) {
                return true;
            }
        } else if y < (crate::mpconfig::INT_MIN as crate::obj::Int / x) {
            return true;
        }
    } else if y > 0 {
        if x < (crate::mpconfig::INT_MIN as crate::obj::Int / y) {
            return true;
        }
    } else if x != 0 && y < (crate::mpconfig::INT_MAX as crate::obj::Int / x) {
        return true;
    }
    *res = x * y;
    false
}

#[inline]
pub fn mp_mul_ll_overflow(x: i64, y: i64, res: &mut i64) -> bool {
    match x.checked_mul(y) {
        Some(v) => {
            *res = v;
            false
        }
        None => true,
    }
}

#[inline]
pub fn mp_mul_ull_overflow(x: u64, y: u64, res: &mut u64) -> bool {
    if y > 0 && x > u64::MAX / y {
        return true;
    }
    *res = x * y;
    false
}

#[inline]
pub fn mp_add_ll_overflow(lhs: i64, rhs: i64, res: &mut i64) -> bool {
    match lhs.checked_add(rhs) {
        Some(v) => {
            *res = v;
            false
        }
        None => true,
    }
}

#[inline]
pub fn mp_sub_ll_overflow(lhs: i64, rhs: i64, res: &mut i64) -> bool {
    match lhs.checked_sub(rhs) {
        Some(v) => {
            *res = v;
            false
        }
        None => true,
    }
}
