//! rewrite of py/formatfloat.c + py/formatfloat.h
// symmetry: done

use crate::mpconfig;
use crate::objfloat::MpFloat;

/// Magic `prec` value for optimal `repr` behaviour (`MP_FLOAT_REPR_PREC`).
pub const FLOAT_REPR_PREC: i32 = 99;

/// Normalize `e`/`E` exponent to MicroPython style: always signed, ≥2 digits (`e+00`).
fn normalize_exp(s: &mut String, upper: bool) {
    let idx = match s.find(['e', 'E']) {
        Some(i) => i,
        None => return,
    };
    let exp: i32 = s[idx + 1..].parse().unwrap_or(0);
    let letter = if upper { 'E' } else { 'e' };
    *s = format!("{}{}{:+03}", &s[..idx], letter, exp);
}

/// Strip trailing zeros after the decimal point (and the point if nothing remains).
fn strip_trailing_frac_zeros(s: &mut String) {
    if let Some(eidx) = s.find(['e', 'E']) {
        let mut head = s[..eidx].to_string();
        let exp = s[eidx..].to_string();
        strip_trailing_frac_zeros(&mut head);
        *s = head + &exp;
        return;
    }
    if !s.contains('.') {
        return;
    }
    while s.ends_with('0') {
        s.pop();
    }
    if s.ends_with('.') {
        s.pop();
    }
}

/// `%g` / `%G` — significant digits, choose `f` or `e` like C `mp_format_float`.
fn format_g(f: MpFloat, prec: usize, upper: bool) -> String {
    let prec = if prec == 0 { 1 } else { prec };
    if f == 0.0 {
        return "0".into();
    }
    let abs = f.abs();
    let exp = abs.log10().floor() as i32;
    // C: use e when exp < -4 or exp >= prec.
    if exp < -4 || exp >= prec as i32 {
        let eprec = prec.saturating_sub(1);
        let mut s = format!("{:.prec$e}", f, prec = eprec);
        normalize_exp(&mut s, upper);
        strip_trailing_frac_zeros(&mut s);
        s
    } else {
        // Digits after decimal so total significant digits ≈ prec.
        let decimals = if exp >= 0 {
            prec.saturating_sub(exp as usize + 1)
        } else {
            prec - 1
        };
        let mut s = format!("{:.prec$}", f, prec = decimals);
        strip_trailing_frac_zeros(&mut s);
        s
    }
}

/// `mp_format_float` — format a float into `buf`, returning length written.
pub fn format_float(f: MpFloat, buf: &mut [u8], fmt: u8, prec: i32, sign: u8) -> usize {
    if !mpconfig::PY_BUILTINS_FLOAT || mpconfig::FLOAT_IMPL == mpconfig::FLOAT_IMPL_NONE {
        return 0;
    }
    let mut s = if f.is_nan() {
        "nan".to_string()
    } else if f.is_infinite() {
        if f > 0.0 {
            "inf".into()
        } else {
            "-inf".into()
        }
    } else {
        // C `MP_FLOAT_REPR_PREC` (99): shortest round-trip style for complex/float repr.
        let p = if prec == FLOAT_REPR_PREC {
            6usize
        } else if prec >= 0 {
            prec as usize
        } else {
            6
        };
        let upper = fmt.is_ascii_uppercase();
        let use_repr_g = prec == FLOAT_REPR_PREC;
        match fmt.to_ascii_uppercase() {
            b'E' => {
                let mut s = format!("{:.prec$e}", f, prec = p);
                normalize_exp(&mut s, upper);
                s
            }
            b'F' => format!("{:.prec$}", f, prec = p),
            b'G' => {
                if use_repr_g {
                    // Prefer shortest decimal like C float repr / complex print.
                    let mut s = if f == 0.0 {
                        "0".into()
                    } else {
                        // Trim via g with enough digits then strip.
                        format_g(f, 6, upper)
                    };
                    // Integer-valued floats print without trailing `.0` in complex context.
                    if let Ok(v) = s.parse::<f64>() {
                        if v == f && f.fract() == 0.0 && f.abs() < 1e15 {
                            s = format!("{}", f as i64);
                        }
                    }
                    s
                } else {
                    format_g(f, p, upper)
                }
            }
            _ => format!("{}", f),
        }
    };
    if sign == b'-' && !s.starts_with('-') {
        s.insert(0, '-');
    } else if sign != 0 && sign != b'-' {
        // SHOW_SIGN: add '+' only when the number has no sign yet (not for negatives).
        if !s.starts_with('+') && !s.starts_with('-') {
            s.insert(0, sign as char);
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

    #[test]
    fn format_e_signed_exp() {
        let mut buf = [0u8; 32];
        let n = format_float(1.5, &mut buf, b'e', 6, 0);
        let s = std::str::from_utf8(&buf[..n]).unwrap();
        assert_eq!(s, "1.500000e+00");
    }
}
