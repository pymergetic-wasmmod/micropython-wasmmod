//! rewrite of py/unicode.c + py/unicode.h
// symmetry: done

use crate::misc::{self, Byte, Unichar, Uint};
use crate::mpconfig;

pub use misc::{
    unichar_isalnum, unichar_isalpha, unichar_isdigit, unichar_isident, unichar_islower,
    unichar_isprint, unichar_isupper, unichar_isspace, unichar_isxdigit, unichar_tolower,
    unichar_toupper, unichar_xdigit_value, utf8_charlen, utf8_get_char, utf8_is_cont,
    utf8_is_nonascii, utf8_next_char,
};

/// Encoding kind (`mp_encoding_t`).
#[repr(u8)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Encoding {
    Utf8 = 0,
    Ascii = 1,
}

/// Byte offset to character index (`utf8_ptr_to_index`).
pub fn utf8_ptr_to_index(s: &[u8], ptr_offset: usize) -> Uint {
    let mut i = 0usize;
    let mut pos = ptr_offset;
    while pos > 0 {
        pos -= 1;
        if !utf8_is_cont(s[pos]) {
            i += 1;
        }
    }
    i as Uint
}

/// Validate buffer encoding (`unicode_encoding_check`).
pub fn unicode_encoding_check(encoding: Encoding, p: &[Byte]) -> bool {
    if !mpconfig::PY_BUILTINS_STR_UNICODE {
        return p.iter().all(|&c| c < 0x80);
    }
    let mut need = 0u8;
    for &c in p {
        if need > 0 {
            if utf8_is_cont(c) {
                need -= 1;
            } else {
                return false;
            }
        } else if encoding == Encoding::Utf8 && c >= 0xc0 {
            if c >= 0xf8 {
                return false;
            }
            need = (0xe5u8 >> ((c >> 3) & 0x6)) & 3;
        } else if c >= 0x80 {
            return false;
        }
    }
    need == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_attrs() {
        assert!(unichar_isdigit(b'5' as Unichar));
        assert!(unichar_isspace(b' ' as Unichar));
    }

    #[test]
    fn utf8_roundtrip() {
        let s = "aβ";
        assert_eq!(utf8_get_char(s.as_bytes()), b'a' as Unichar);
        let rest = utf8_next_char(s.as_bytes());
        assert_eq!(utf8_get_char(rest), 0x03b2);
    }
}
