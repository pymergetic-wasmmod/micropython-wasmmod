//! rewrite of py/parsenumbase.h + py/parsenumbase.c
// symmetry: done

use crate::misc::Byte;

/// Find real radix base and strip preceding `0x` / `0o` / `0b`.
///
/// Writes the resolved base into `base`. Returns the number of prefix bytes to skip.
/// In base-0, sets `*base = 1` for a leading `0` without a valid radix letter, so a later
/// parser can raise `ValueError` unless the rest is all-zero digits.
pub fn parse_num_base(str: &[u8], base: &mut i32) -> usize {
    let len = str.len();
    let p0 = str.as_ptr();
    let mut p: *const Byte = p0;
    unsafe {
        if len <= 1 {
            if *base == 0 {
                *base = 10;
            }
            return 0;
        }
        let mut c = *p;
        p = p.add(1);
        if c == b'0' {
            c = (*p) | 32;
            p = p.add(1);
            let b = *base;
            if c == b'x' && (b == 0 || b == 16) {
                *base = 16;
            } else if c == b'o' && (b == 0 || b == 8) {
                *base = 8;
            } else if c == b'b' && (b == 0 || b == 2) {
                *base = 2;
            } else {
                p = p.sub(2);
                if b == 0 {
                    *base = 1;
                }
            }
        } else {
            p = p.sub(1);
            if *base == 0 {
                *base = 10;
            }
        }
        p.offset_from(p0) as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefixes() {
        let mut b = 0;
        assert_eq!(parse_num_base(b"0x10", &mut b), 2);
        assert_eq!(b, 16);
        b = 0;
        assert_eq!(parse_num_base(b"0o7", &mut b), 2);
        assert_eq!(b, 8);
        b = 0;
        assert_eq!(parse_num_base(b"0b10", &mut b), 2);
        assert_eq!(b, 2);
        b = 0;
        assert_eq!(parse_num_base(b"42", &mut b), 0);
        assert_eq!(b, 10);
        b = 0;
        assert_eq!(parse_num_base(b"0123", &mut b), 0);
        assert_eq!(b, 1);
    }
}
