//! rewrite of py/formatfloat.c + py/formatfloat.h
// symmetry: done

use crate::objfloat::MpFloat;
use crate::mpconfig;

/// Magic `prec` value for optimal `repr` behaviour (`MP_FLOAT_REPR_PREC`).
pub const FLOAT_REPR_PREC: i32 = 99;

/// `mp_format_float` — format a float into `buf`, returning length written.
pub fn format_float(f: MpFloat, buf: &mut [u8], fmt: u8, prec: i32, sign: u8) -> usize {
    if !mpconfig::PY_BUILTINS_FLOAT || mpconfig::FLOAT_IMPL == mpconfig::FLOAT_IMPL_NONE {
        return 0;
    }
    let mut s = if f.is_nan() {
        "nan".to_string()
    } else if f.is_infinite() {
        if f > 0.0 { "inf".into() } else { "-inf".into() }
    } else {
        let p = if prec >= 0 { prec as usize } else { 6 };
        let ch = (fmt & !0x20) as char;
        match ch {
            'e' | 'E' => format!("{:.prec$e}", f, prec = p),
            'f' | 'F' => format!("{:.prec$}", f, prec = p),
            'g' | 'G' => format!("{:.prec$}", f, prec = p),
            _ => format!("{}", f),
        }
    };
    if sign == b'-' && !s.starts_with('-') {
        s.insert(0, '-');
    } else if sign != 0 && sign != b'-' {
        let c = sign as char;
        if !s.starts_with(c) {
            s.insert(0, c);
        }
    }
    let bytes = s.as_bytes();
    let n = bytes.len().min(buf.len().saturating_sub(1));
    buf[..n].copy_from_slice(&bytes[..n]);
    if n < buf.len() {
        buf[n] = 0;
    }
    n
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_basic() {
        let mut buf = [0u8; 32];
        let n = format_float(1.5, &mut buf, b'g', 6, 0);
        assert!(n > 0);
        let s = std::str::from_utf8(&buf[..n]).unwrap();
        assert!(s.contains("1.5"), "got {s}");
    }
}
