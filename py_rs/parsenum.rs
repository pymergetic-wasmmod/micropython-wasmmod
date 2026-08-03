//! rewrite of py/parsenum.c + py/parsenum.h
// symmetry: done

use crate::misc::{self, Unichar};
use crate::mpconfig;
use crate::obj::{self, Int, Obj};
use crate::objcomplex;
use crate::objfloat;
use crate::objint;
use crate::parsenumbase;
use crate::raise::{self, MpRaise};
use crate::smallint;

/// Minimal lexer context for syntax-error conversion (`mp_lexer_t` fields used by parsenum).
#[derive(Copy, Clone, Debug, Default)]
pub struct ParseLexer {
    pub source_name: u32,
    pub tok_line: u32,
}

fn raise_exc(exc: MpRaise, lex: Option<&ParseLexer>) -> ! {
    if lex.is_some() {
        if let Some(msg) = exc.message() {
            raise::raise(MpRaise::SyntaxError(msg));
        }
        raise::raise(MpRaise::SyntaxError("invalid syntax"));
    }
    raise::raise(exc);
}

type ParsedInt = Int;

fn parsed_int_mul_overflow(x: ParsedInt, y: ParsedInt, res: &mut ParsedInt) -> bool {
    misc::mp_mul_mp_int_t_overflow(x, y, res)
}

fn parsed_int_fits(n: ParsedInt) -> bool {
    smallint::fits(n)
}

/// `mp_parse_num_integer`
pub fn parse_num_integer(str: &[u8], mut base: i32, lex: Option<&ParseLexer>) -> Obj {
    if (base != 0 && base < 2) || base > 36 {
        raise_exc(MpRaise::ValueError("int() arg 2 must be >= 2 and <= 36"), lex);
    }

    let mut pos = 0usize;
    while pos < str.len() && misc::unichar_isspace(str[pos] as Unichar) {
        pos += 1;
    }

    let mut neg = false;
    if pos < str.len() {
        match str[pos] {
            b'+' => pos += 1,
            b'-' => {
                pos += 1;
                neg = true;
            }
            _ => {}
        }
    }

    let tail = &str[pos..];
    pos += parsenumbase::parse_num_base(tail, &mut base);
    let top = str.len();
    let str_val_start = pos;
    let mut parsed_val: ParsedInt = 0;

    while pos < top {
        let mut dig = str[pos];
        dig = if dig == b'_' {
            pos += 1;
            continue;
        } else if b'0' <= dig && dig <= b'9' {
            dig - b'0'
        } else {
            dig |= 0x20;
            if b'a' <= dig && dig <= b'z' {
                dig - (b'a' - 10)
            } else {
                break;
            }
        };
        if dig as u32 >= base as u32 {
            break;
        }
        if parsed_int_mul_overflow(parsed_val, base as ParsedInt, &mut parsed_val) {
            return parse_overflow(str, str_val_start, top, neg, base, lex);
        }
        parsed_val += dig as ParsedInt;
        if !parsed_int_fits(parsed_val) {
            return parse_overflow(str, str_val_start, top, neg, base, lex);
        }
        pos += 1;
    }

    let ret_val = obj::new_small_int(if neg { -parsed_val } else { parsed_val });

    if pos == str_val_start {
        raise_value_error(base, &str[str_val_start..top], lex);
    }

    while pos < top && misc::unichar_isspace(str[pos] as Unichar) {
        pos += 1;
    }
    if pos != top {
        raise_value_error(base, &str[str_val_start..top], lex);
    }
    ret_val
}

fn parse_overflow(str: &[u8], start: usize, top: usize, neg: bool, base: i32, lex: Option<&ParseLexer>) -> Obj {
    if mpconfig::LONGINT_IMPL == mpconfig::LONGINT_IMPL_LONGLONG {
        raise_exc(MpRaise::OverflowError("result overflows long long storage"), lex);
    }
    let digits = std::str::from_utf8(&str[start..top]).unwrap_or("");
    let (val, consumed) = objint::new_int_from_str(digits, neg, base as u32);
    if consumed == 0 {
        raise_value_error(base, &str[start..top], lex);
    }
    val
}

fn raise_value_error(base: i32, _fragment: &[u8], lex: Option<&ParseLexer>) -> ! {
    if mpconfig::ERROR_REPORTING <= mpconfig::ERROR_REPORTING_TERSE as u8 {
        raise_exc(MpRaise::ValueError("invalid syntax for integer"), lex);
    } else if mpconfig::ERROR_REPORTING == mpconfig::ERROR_REPORTING_NORMAL as u8 {
        let _ = base;
        raise_exc(MpRaise::ValueError("invalid syntax for integer with base"), lex);
    } else {
        raise_exc(MpRaise::ValueError("invalid syntax for integer with base"), lex);
    }
}

mod float_parse {
    use super::*;

    const MANTISSA_MAX: u64 = if std::mem::size_of::<u64>() == 8 {
        0x1999999999999998
    } else {
        0x19999998
    };

    const MAX_EXACT_POWER_OF_5: i32 = if mpconfig::FLOAT_IMPL == mpconfig::FLOAT_IMPL_DOUBLE {
        22
    } else {
        10
    };

    #[derive(Copy, Clone, Eq, PartialEq)]
    enum ParseDecIn {
        Intg,
        Frac,
        Exp,
    }

    /// `mp_decimal_exp`
    pub fn decimal_exp(num: f64, dec_exp: i32) -> f64 {
        if dec_exp == 0 || num == 0.0 {
            return num;
        }
        if mpconfig::FLOAT_FORMAT_IMPL == mpconfig::FLOAT_FORMAT_IMPL_EXACT {
            let neg_exp = dec_exp < 0;
            let mut dec_exp = dec_exp.abs();
            let mut res = num;
            let mut expo = 10.0f64;
            while dec_exp != 0 {
                if dec_exp & 1 != 0 {
                    if neg_exp {
                        res /= expo;
                    } else {
                        res *= expo;
                    }
                }
                dec_exp >>= 1;
                if dec_exp != 0 {
                    expo *= expo;
                }
            }
            res
        } else {
            let mut bits = num.to_bits();
            let exp_field = ((bits >> 52) & 0x7ff) as i32;
            let new_exp = exp_field + dec_exp;
            bits = (bits & !0x7ff0000000000000) | ((new_exp as u64) << 52);
            let mut res = f64::from_bits(bits);
            if dec_exp < 0 && dec_exp >= -MAX_EXACT_POWER_OF_5 {
                res /= 5f64.powi(-dec_exp);
            } else {
                res *= 5f64.powi(dec_exp);
            }
            res
        }
    }

    fn accept_digit(
        mantissa: u64,
        dig: u32,
        exp_extra: &mut i32,
        in_: ParseDecIn,
    ) -> u64 {
        if mantissa < MANTISSA_MAX {
            if in_ == ParseDecIn::Frac {
                *exp_extra -= 1;
            }
            10 * mantissa + u64::from(dig)
        } else if in_ == ParseDecIn::Intg {
            *exp_extra += 1;
            mantissa
        } else {
            mantissa
        }
    }

    /// `mp_parse_float_internal`
    pub fn parse_float_internal(str: &[u8]) -> Option<(f64, usize)> {
        let top = str.len();
        let mut pos = 0usize;
        let mut in_ = ParseDecIn::Intg;
        let mut exp_neg = false;
        let mut mantissa = 0u64;
        let mut exp_val = 0i32;
        let mut exp_extra = 0i32;
        let mut trailing_zeros_intg = 0i32;
        let mut trailing_zeros_frac = 0i32;

        while pos < top {
            let dig = str[pos];
            pos += 1;
            if b'0' <= dig && dig <= b'9' {
                let dig = dig - b'0';
                if in_ == ParseDecIn::Exp {
                    if exp_val < (i32::MAX / 2 - 9) / 10 {
                        exp_val = 10 * exp_val + dig as i32;
                    }
                } else if dig == 0 || mantissa >= MANTISSA_MAX {
                    if in_ == ParseDecIn::Intg {
                        trailing_zeros_intg += 1;
                    } else {
                        trailing_zeros_frac += 1;
                    }
                } else {
                    while trailing_zeros_intg > 0 {
                        mantissa = accept_digit(mantissa, 0, &mut exp_extra, ParseDecIn::Intg);
                        trailing_zeros_intg -= 1;
                    }
                    while trailing_zeros_frac > 0 {
                        mantissa = accept_digit(mantissa, 0, &mut exp_extra, ParseDecIn::Frac);
                        trailing_zeros_frac -= 1;
                    }
                    mantissa = accept_digit(mantissa, dig as u32, &mut exp_extra, in_);
                }
            } else if in_ == ParseDecIn::Intg && dig == b'.' {
                in_ = ParseDecIn::Frac;
            } else if in_ != ParseDecIn::Exp && (dig | 0x20) == b'e' {
                in_ = ParseDecIn::Exp;
                if pos < top {
                    if str[pos] == b'+' {
                        pos += 1;
                    } else if str[pos] == b'-' {
                        pos += 1;
                        exp_neg = true;
                    }
                }
                if pos == top {
                    return None;
                }
            } else if dig == b'_' {
                continue;
            } else {
                pos -= 1;
                break;
            }
        }

        if exp_neg {
            exp_val = -exp_val;
        }
        exp_val += exp_extra + trailing_zeros_intg;
        let res = decimal_exp(mantissa as f64, exp_val);
        Some((res, pos))
    }
}

/// `mp_parse_num_decimal` / `mp_parse_num_float`
pub fn parse_num_decimal(
    str: &[u8],
    allow_imag: bool,
    force_complex: bool,
    lex: Option<&ParseLexer>,
) -> Obj {
    if !mpconfig::PY_BUILTINS_FLOAT {
        raise_exc(MpRaise::ValueError("decimal numbers not supported"), lex);
    }

    let top = str.len();
    let mut pos = 0usize;
    let mut dec_neg = false;

    #[derive(Copy, Clone, Eq, PartialEq)]
    enum RealImag {
        Start = 0,
        HaveReal = 1,
        HaveImag = 2,
    }
    let mut real_imag_state = RealImag::Start;
    let mut dec_real = 0.0f64;
    let mut dec_val = 0.0f64;

    loop {
        while pos < top && misc::unichar_isspace(str[pos] as Unichar) {
            pos += 1;
        }

        dec_neg = false;
        if pos < top {
            match str[pos] {
                b'+' => pos += 1,
                b'-' => {
                    pos += 1;
                    dec_neg = true;
                }
                _ => {}
            }
        }

        let str_val_start = pos;

        if pos + 2 < top
            && (str[pos] | 0x20) == b'i'
            && (str[pos + 1] | 0x20) == b'n'
            && (str[pos + 2] | 0x20) == b'f'
        {
            pos += 3;
            dec_val = f64::INFINITY;
            if pos + 4 < top
                && (str[pos] | 0x20) == b'i'
                && (str[pos + 1] | 0x20) == b'n'
                && (str[pos + 2] | 0x20) == b'i'
                && (str[pos + 3] | 0x20) == b't'
                && (str[pos + 4] | 0x20) == b'y'
            {
                pos += 5;
            }
        } else if pos + 2 < top
            && (str[pos] | 0x20) == b'n'
            && (str[pos + 1] | 0x20) == b'a'
            && (str[pos + 2] | 0x20) == b'n'
        {
            pos += 3;
            dec_val = f64::NAN;
        } else {
            let tail = &str[pos..];
            match float_parse::parse_float_internal(tail) {
                Some((v, consumed)) => {
                    dec_val = v;
                    pos += consumed;
                }
                None => raise_exc(MpRaise::ValueError("invalid syntax for number"), lex),
            }
        }

        if allow_imag && pos < top && (str[pos] | 0x20) == b'j' {
            if pos == str_val_start {
                dec_val = 1.0;
            }
            pos += 1;
            real_imag_state = RealImag::HaveImag;
        }

        if dec_neg {
            dec_val = -dec_val;
        }

        if pos == str_val_start {
            raise_exc(MpRaise::ValueError("invalid syntax for number"), lex);
        }

        while pos < top && misc::unichar_isspace(str[pos] as Unichar) {
            pos += 1;
        }

        if pos != top {
            if mpconfig::PY_BUILTINS_COMPLEX
                && force_complex
                && real_imag_state == RealImag::Start
            {
                dec_real = dec_val;
                dec_val = 0.0;
                real_imag_state = RealImag::HaveReal;
                continue;
            }
            raise_exc(MpRaise::ValueError("invalid syntax for number"), lex);
        }

        if mpconfig::PY_BUILTINS_COMPLEX && real_imag_state == RealImag::HaveReal {
            raise_exc(MpRaise::ValueError("invalid syntax for number"), lex);
        }

        if mpconfig::PY_BUILTINS_COMPLEX && real_imag_state != RealImag::Start {
            return objcomplex::new_complex(dec_real, dec_val);
        }
        if mpconfig::PY_BUILTINS_COMPLEX && force_complex {
            return objcomplex::new_complex(dec_val, 0.0);
        }
        return objfloat::new_float(dec_val);
    }
}

/// `mp_parse_num_float` when complex is enabled (inline in C header).
pub fn parse_num_float(str: &[u8], allow_imag: bool, lex: Option<&ParseLexer>) -> Obj {
    parse_num_decimal(str, allow_imag, false, lex)
}

/// `mp_parse_num_complex`
pub fn parse_num_complex(str: &[u8], lex: Option<&ParseLexer>) -> Obj {
    parse_num_decimal(str, true, true, lex)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gc;
    use crate::nlr;
    use crate::raise;

    fn setup() {
        let _ = gc::init();
    }

    #[test]
    fn parse_decimal_int() {
        setup();
        let v = parse_num_integer(b"42", 10, None);
        assert_eq!(obj::small_int_value(v), 42);
        let v = parse_num_integer(b"-0x10", 0, None);
        assert_eq!(obj::small_int_value(v), -16);
        let v = parse_num_integer(b"0b1010", 0, None);
        assert_eq!(obj::small_int_value(v), 10);
    }

    #[test]
    fn parse_underscore_int() {
        setup();
        let v = parse_num_integer(b"1_000", 10, None);
        assert_eq!(obj::small_int_value(v), 1000);
    }

    #[test]
    fn parse_bigint() {
        setup();
        let s = b"999999999999999999999";
        let v = parse_num_integer(s, 10, None);
        assert!(obj::is_exact_type(v, objint::type_int()));
    }

    #[test]
    fn parse_float_basic() {
        if !mpconfig::PY_BUILTINS_FLOAT {
            return;
        }
        setup();
        let v = parse_num_float(b"3.14", false, None);
        assert!((objfloat::float_get(v) - 3.14).abs() < 1e-10);
    }

    #[test]
    fn parse_complex_literal() {
        if !mpconfig::PY_BUILTINS_COMPLEX {
            return;
        }
        setup();
        let v = parse_num_complex(b"3+4j", None);
        assert!(obj::is_exact_type(v, objcomplex::type_complex()));
    }

    #[test]
    fn invalid_int_raises() {
        let mut buf = nlr::NlrBuf::default();
        let err = nlr::protect(&mut buf, || {
            parse_num_integer(b"12abc", 10, None);
        });
        assert!(err.is_err());
        if let Err(code) = err {
            assert!(matches!(raise::decode(code), MpRaise::ValueError(_)));
        }
    }
}
