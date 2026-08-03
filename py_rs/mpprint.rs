//! rewrite of py/mpprint.c + py/mpprint.h
// symmetry: done

use crate::mphal;
use crate::mpconfig;
use crate::obj::{self, Int, Obj, Uint};
use crate::qstr::{self, Qstr};

/// Print kind (`mp_print_kind_t`).
#[repr(u8)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum PrintKind {
    Str = 0,
    Repr = 1,
    Exc = 2,
    Json = 3,
    Raw = 4,
}

pub const PF_FLAG_LEFT_ADJUST: u32 = 0x001;
pub const PF_FLAG_SHOW_SIGN: u32 = 0x002;
pub const PF_FLAG_SPACE_SIGN: u32 = 0x004;
pub const PF_FLAG_SHOW_PREFIX: u32 = 0x008;
pub const PF_FLAG_PAD_AFTER_SIGN: u32 = 0x010;
pub const PF_FLAG_CENTER_ADJUST: u32 = 0x020;
pub const PF_FLAG_ADD_PERCENT: u32 = 0x040;
pub const PF_FLAG_SHOW_OCTAL_LETTER: u32 = 0x080;
pub const PF_FLAG_ALWAYS_DECIMAL: u32 = 0x100;
pub const PF_FLAG_SEP_POS: u32 = 9;

pub type PrintStrnFn = extern "C" fn(*mut (), *const u8, usize);

/// Print sink (`mp_print_t`).
pub struct Print {
    pub data: *mut (),
    pub print_strn: Option<PrintStrnFn>,
}

unsafe impl Send for Print {}
unsafe impl Sync for Print {}

pub struct PrintExt {
    pub base: Print,
    pub item_separator: *const u8,
    pub key_separator: *const u8,
}

/// Cast print sink to extended JSON print state (`MP_PRINT_GET_EXT`).
pub fn print_get_ext(print: &Print) -> &PrintExt {
    unsafe { &*(print as *const Print as *const PrintExt) }
}

fn cstr_at(ptr: *const u8, default: &str) -> &str {
    if ptr.is_null() {
        return default;
    }
    unsafe {
        let mut len = 0usize;
        while *ptr.add(len) != 0 {
            len += 1;
        }
        std::str::from_utf8(std::slice::from_raw_parts(ptr, len)).unwrap_or(default)
    }
}

/// JSON list/tuple item separator from `PrintExt` when enabled.
pub fn json_item_separator(print: &Print) -> &str {
    if mpconfig::PY_JSON_SEPARATORS {
        cstr_at(print_get_ext(print).item_separator, ", ")
    } else {
        ", "
    }
}

/// JSON dict key/value separator from `PrintExt` when enabled.
pub fn json_key_separator(print: &Print) -> &str {
    if mpconfig::PY_JSON_SEPARATORS {
        cstr_at(print_get_ext(print).key_separator, ": ")
    } else {
        ": "
    }
}

extern "C" fn plat_print_strn(_env: *mut (), str: *const u8, len: usize) {
    let s = unsafe { std::slice::from_raw_parts(str, len) };
    if let Ok(text) = std::str::from_utf8(s) {
        mphal::plat_print_strn(text);
    }
}

pub static PLAT_PRINT: Print = Print {
    data: std::ptr::null_mut(),
    print_strn: Some(plat_print_strn),
};

fn emit_strn(print: &Print, str: &[u8]) {
    if let Some(f) = print.print_strn {
        f(print.data, str.as_ptr(), str.len());
    }
}

/// `mp_print_str`
pub fn print_str(print: &Print, str: &str) -> i32 {
    if !str.is_empty() {
        emit_strn(print, str.as_bytes());
    }
    str.len() as i32
}

/// `mp_print_strn`
pub fn print_strn(
    print: &Print,
    str: &[u8],
    flags: u32,
    fill: u8,
    width: i32,
) -> i32 {
    let len = str.len() as i32;
    let mut left_pad = 0;
    let mut right_pad = 0;
    let mut pad = width - len;
    let grouping = flags >> PF_FLAG_SEP_POS;

    let (pad_chars, pad_size): (&[u8], i32) = if fill == 0 || fill == b' ' {
        (b"                ", 16)
    } else if fill == b'0' && grouping == 0 {
        (b"0000000000000000", 16)
    } else if fill == b'0' {
        if grouping == b'_' as u32 {
            (b"00000", 5)
        } else {
            (b"000,00", 4)
        }
    } else {
        (&[fill], 1)
    };

    let mut pad_chars = pad_chars;
    let mut width = width;
    if fill == b'0' && grouping != 0 {
        if width % pad_size == 0 {
            pad += 1;
            width += 1;
        }
        let offset = (pad_size - 1 - width % pad_size) as usize;
        pad_chars = &pad_chars[offset..];
    }

    if flags & PF_FLAG_CENTER_ADJUST != 0 {
        left_pad = pad / 2;
        right_pad = pad - left_pad;
    } else if flags & PF_FLAG_LEFT_ADJUST != 0 {
        right_pad = pad;
    } else {
        left_pad = pad;
    }

    let mut total = 0;
    if left_pad > 0 {
        total += left_pad;
        let mut left = left_pad;
        while left > 0 {
            let p = left.min(pad_size);
            emit_strn(print, &pad_chars[..p as usize]);
            left -= p;
        }
    }
    if len > 0 {
        emit_strn(print, str);
        total += len;
    }
    if right_pad > 0 {
        total += right_pad;
        let mut right = right_pad;
        while right > 0 {
            let p = right.min(pad_size);
            emit_strn(print, &pad_chars[..p as usize]);
            right -= p;
        }
    }
    total
}

const INT_BUF_SIZE: usize = std::mem::size_of::<Int>() * 4;

fn print_int(
    print: &Print,
    mut x: Uint,
    sgn: bool,
    base: i32,
    base_char: u8,
    flags: u32,
    fill: u8,
    mut width: i32,
) -> i32 {
    let mut sign = 0u8;
    if sgn {
        if (x as Int) < 0 {
            sign = b'-';
            x = x.wrapping_neg();
        } else if flags & PF_FLAG_SHOW_SIGN != 0 {
            sign = b'+';
        } else if flags & PF_FLAG_SPACE_SIGN != 0 {
            sign = b' ';
        }
    }

    let mut buf = [0u8; INT_BUF_SIZE];
    let mut b = INT_BUF_SIZE;
    if x == 0 {
        b -= 1;
        buf[b] = b'0';
    } else {
        while b > 0 && x != 0 {
            b -= 1;
            let mut c = (x % base as Uint) as u8;
            x /= base as Uint;
            if c >= 10 {
                c = c.wrapping_add(base_char.wrapping_sub(10));
            } else {
                c += b'0';
            }
            buf[b] = c;
        }
    }

    let mut len = 0;
    if flags & PF_FLAG_PAD_AFTER_SIGN != 0 {
        if sign != 0 {
            len += print_strn(print, &[sign], flags, fill, 1);
            width -= 1;
        }
    } else {
        if sign != 0 && b > 0 {
            b -= 1;
            buf[b] = sign;
        }
    }
    len += print_strn(print, &buf[b..INT_BUF_SIZE], flags, fill, width);
    len
}

fn int_formatted_small(
    x: Obj,
    base: i32,
    prefix: Option<&str>,
    base_char: u8,
    comma: u8,
) -> (Vec<u8>, usize) {
    let mut num = if obj::is_small_int(x) {
        obj::small_int_value(x)
    } else {
        0
    };
    let mut sign = 0u8;
    if num < 0 {
        num = num.wrapping_neg();
        sign = b'-';
    }
    let n_comma = if base == 10 { 3 } else { 4 };
    let prefix_len = prefix.map(|p| p.len()).unwrap_or(0);
    let needed = INT_BUF_SIZE;
    let mut buf = vec![0u8; needed];
    let mut b = needed;
    b -= 1;
    buf[b] = 0;
    let mut last_comma = b;
    if num == 0 {
        b -= 1;
        buf[b] = b'0';
    } else {
        while b > 0 && num != 0 {
            b -= 1;
            let mut c = (num % base as Int).unsigned_abs() as u8;
            num /= base as Int;
            if c >= 10 {
                c = c.wrapping_add(base_char.wrapping_sub(10));
            } else {
                c += b'0';
            }
            buf[b] = c;
            if comma != 0 && num != 0 && b > 0 && last_comma - b == n_comma {
                b -= 1;
                buf[b] = comma;
                last_comma = b;
            }
        }
    }
    if let Some(prefix) = prefix {
        let pl = prefix.len();
        if b >= pl {
            b -= pl;
            buf[b..b + pl].copy_from_slice(prefix.as_bytes());
        }
    }
    if sign != 0 && b > 0 {
        b -= 1;
        buf[b] = sign;
    }
    let fmt_size = needed - b - 1;
    (buf[b..b + fmt_size].to_vec(), fmt_size)
}

/// `mp_print_mp_int` (small-int path complete; heap int via objint hook later).
pub fn print_mp_int(
    print: &Print,
    x: Obj,
    base: u32,
    base_char: u8,
    mut flags: u32,
    fill: u8,
    width: i32,
    prec: i32,
) -> i32 {
    assert!(base == 2 || base == 8 || base == 10 || base == 16);
    let x = if obj::is_small_int(x) {
        x
    } else if obj::is_bool(x) {
        obj::new_small_int(i32::from(obj::bool_value(x)) as Int)
    } else {
        obj::new_small_int(0)
    };

    if flags & (PF_FLAG_LEFT_ADJUST | PF_FLAG_CENTER_ADJUST) == 0 && fill == b'0' {
        let mut width = width;
        let mut prec = prec;
        if prec > width {
            width = prec;
        }
        prec = 0;
    }

    let mut prefix_buf = [0u8; 4];
    let mut prefix_len = 0usize;
    if obj::is_small_int(x) && obj::small_int_value(x) >= 0 {
        if flags & PF_FLAG_SHOW_SIGN != 0 {
            prefix_buf[prefix_len] = b'+';
            prefix_len += 1;
        } else if flags & PF_FLAG_SPACE_SIGN != 0 {
            prefix_buf[prefix_len] = b' ';
            prefix_len += 1;
        }
    }
    if flags & PF_FLAG_SHOW_PREFIX != 0 {
        if base == 2 {
            prefix_buf[prefix_len] = b'0';
            prefix_len += 1;
            prefix_buf[prefix_len] = base_char + (b'b' - b'a');
            prefix_len += 1;
        } else if base == 8 {
            prefix_buf[prefix_len] = b'0';
            prefix_len += 1;
            if flags & PF_FLAG_SHOW_OCTAL_LETTER != 0 {
                prefix_buf[prefix_len] = base_char + (b'o' - b'a');
                prefix_len += 1;
            }
        } else if base == 16 {
            prefix_buf[prefix_len] = b'0';
            prefix_len += 1;
            prefix_buf[prefix_len] = base_char + (b'x' - b'a');
            prefix_len += 1;
        }
    }
    let prefix = std::str::from_utf8(&prefix_buf[..prefix_len]).ok();
    let comma = (flags >> PF_FLAG_SEP_POS) as u8;

    let (mut str, mut fmt_size) = if prec > 1 {
        flags |= PF_FLAG_PAD_AFTER_SIGN;
        int_formatted_small(x, base as i32, None, base_char, comma)
    } else {
        int_formatted_small(x, base as i32, prefix, base_char, comma)
    };

    let mut sign = 0u8;
    if flags & PF_FLAG_PAD_AFTER_SIGN != 0 {
        if !str.is_empty() && str[0] == b'-' {
            sign = b'-';
            str.remove(0);
            fmt_size -= 1;
        }
    }

    let mut spaces_before = 0;
    let mut spaces_after = 0;
    let mut width = width;
    let mut fill = fill;
    if prec > 1 {
        let mut prec_width = fmt_size as i32;
        if prec_width < prec {
            prec_width = prec;
        }
        if flags & PF_FLAG_PAD_AFTER_SIGN != 0 {
            if sign != 0 {
                prec_width += 1;
            }
            prec_width += prefix_len as i32;
        }
        if prec_width < width {
            if flags & PF_FLAG_LEFT_ADJUST != 0 {
                spaces_after = width - prec_width;
            } else {
                spaces_before = width - prec_width;
            }
        }
        fill = b'0';
        flags &= !PF_FLAG_LEFT_ADJUST;
    }

    let mut len = 0;
    if spaces_before > 0 {
        len += print_strn(print, b"", 0, b' ', spaces_before);
    }
    if flags & PF_FLAG_PAD_AFTER_SIGN != 0 {
        if sign != 0 {
            len += print_strn(print, &[sign], 0, 0, 1);
            width -= 1;
        }
        if prefix_len > 0 {
            len += print_strn(print, &prefix_buf[..prefix_len], 0, 0, 1);
            width -= prefix_len as i32;
        }
    }
    if prec > 1 {
        width = prec;
    }
    len += print_strn(print, &str, flags, fill, width);
    if spaces_after > 0 {
        len += print_strn(print, b"", 0, b' ', spaces_after);
    }
    len
}

/// Variadic printf argument (`va_arg` replacement for host Rust).
#[derive(Clone, Copy)]
pub enum VaArg<'a> {
    Int(i32),
    UInt(u32),
    USize(usize),
    Long(i64),
    ULong(u64),
    Str(&'a str),
    Qstr(Qstr),
    Double(f64),
    Char(u8),
    Bool(bool),
}

/// `mp_vprintf`
pub fn vprintf<'a>(print: &Print, fmt: &str, args: impl IntoIterator<Item = VaArg<'a>>) -> i32 {
    let mut args = args.into_iter();
    let mut fmt = fmt;
    let mut chrs = 0;
    loop {
        if let Some(i) = fmt.find('%') {
            if i > 0 {
                emit_strn(print, &fmt.as_bytes()[..i]);
                chrs += i as i32;
            }
            fmt = &fmt[i..];
        } else {
            emit_strn(print, fmt.as_bytes());
            chrs += fmt.len() as i32;
            break;
        }

        if fmt.is_empty() {
            break;
        }
        fmt = &fmt[1..];

        let mut flags = 0u32;
        let mut fill = b' ';
        for c in fmt.chars() {
            match c {
                '-' => flags |= PF_FLAG_LEFT_ADJUST,
                '+' => flags |= PF_FLAG_SHOW_SIGN,
                ' ' => flags |= PF_FLAG_SPACE_SIGN,
                '0' => {
                    flags |= PF_FLAG_PAD_AFTER_SIGN;
                    fill = b'0';
                }
                _ => break,
            }
            fmt = &fmt[c.len_utf8()..];
        }

        let mut width = 0i32;
        while fmt.starts_with(|c: char| c.is_ascii_digit()) {
            width = width * 10 + (fmt.as_bytes()[0] - b'0') as i32;
            fmt = &fmt[1..];
        }

        let mut prec = -1i32;
        if fmt.starts_with('.') {
            fmt = &fmt[1..];
            if fmt.starts_with('*') {
                fmt = &fmt[1..];
                prec = args.next().map(|a| match a {
                    VaArg::Int(v) => v,
                    _ => 0,
                }).unwrap_or(0);
            } else {
                prec = 0;
                while fmt.starts_with(|c: char| c.is_ascii_digit()) {
                    prec = prec * 10 + (fmt.as_bytes()[0] - b'0') as i32;
                    fmt = &fmt[1..];
                }
            }
            if prec < 0 {
                prec = 0;
            }
        }

        if fmt.starts_with('l') {
            fmt = &fmt[1..];
            if fmt.starts_with('l') {
                fmt = &fmt[1..];
            }
        }

        if fmt.is_empty() {
            break;
        }

        let spec = fmt.as_bytes()[0];
        fmt = &fmt[1..];

        match spec {
            b'b' => {
                let v = args.next().and_then(|a| match a {
                    VaArg::Int(x) => Some(x != 0),
                    VaArg::Bool(b) => Some(b),
                    _ => None,
                }).unwrap_or(false);
                chrs += print_strn(
                    print,
                    if v { b"true" } else { b"false" },
                    flags,
                    fill,
                    width,
                );
            }
            b'c' => {
                let ch = args.next().and_then(|a| match a {
                    VaArg::Int(v) => Some(v as u8),
                    VaArg::Char(v) => Some(v),
                    _ => None,
                }).unwrap_or(0);
                chrs += print_strn(print, &[ch], flags, fill, width);
            }
            b'q' => {
                let q = args.next().and_then(|a| match a {
                    VaArg::Qstr(q) => Some(q),
                    _ => None,
                }).unwrap_or(0);
                let data = qstr::str_data(q).unwrap_or_default();
                let mut len = data.len();
                if prec >= 0 && (prec as usize) < len {
                    len = prec as usize;
                }
                chrs += print_strn(print, &data[..len], flags, fill, width);
            }
            b's' => {
                let s = args.next().and_then(|a| match a {
                    VaArg::Str(s) => Some(s),
                    _ => None,
                }).unwrap_or("");
                let mut len = s.len();
                if prec >= 0 && (prec as usize) < len {
                    len = prec as usize;
                }
                chrs += print_strn(print, &s.as_bytes()[..len], flags, fill, width);
            }
            b'd' | b'p' | b'P' | b'u' | b'x' | b'X' => {
                let val: Uint = match args.next() {
                    Some(VaArg::Int(v)) if spec == b'd' => v as Uint,
                    Some(VaArg::UInt(v)) => v as Uint,
                    Some(VaArg::USize(v)) => v,
                    Some(VaArg::Long(v)) if spec == b'd' => v as Uint,
                    Some(VaArg::ULong(v)) => v as Uint,
                    _ => 0,
                };
                let base = if spec == b'd' || spec == b'u' { 10 } else { 16 };
                let fmt_c = (spec & 0xf0) as u8 + b'A' - b'P';
                chrs += print_int(print, val, spec == b'd', base, fmt_c, flags, fill, width);
            }
            _ => {
                emit_strn(print, &[spec]);
                chrs += 1;
            }
        }
    }
    chrs
}

/// `mp_print_float`
pub fn print_float(
    print: &Print,
    f: crate::objfloat::MpFloat,
    fmt: u8,
    mut flags: u32,
    fill: u8,
    width: i32,
    prec: i32,
) -> i32 {
    if !crate::mpconfig::PY_BUILTINS_FLOAT {
        return 0;
    }
    let mut sign = 0u8;
    if flags & PF_FLAG_SHOW_SIGN != 0 {
        sign = b'+';
    } else if flags & PF_FLAG_SPACE_SIGN != 0 {
        sign = b' ';
    }
    let mut buf = [0u8; 36];
    let mut len = crate::formatfloat::format_float(f, &mut buf, fmt, prec, sign);
    let mut s = 0usize;
    if flags & PF_FLAG_ALWAYS_DECIMAL != 0
        && !buf[..len].contains(&b'.')
        && !buf[..len].contains(&b'e')
        && !buf[..len].contains(&b'n')
    {
        buf[len] = b'.';
        len += 1;
        buf[len] = b'0';
        len += 1;
    }
    if flags & PF_FLAG_ADD_PERCENT != 0 {
        buf[len] = b'%';
        len += 1;
    }
    let mut chrs = 0i32;
    let mut width = width;
    if flags & PF_FLAG_PAD_AFTER_SIGN != 0 && len > 0 && buf[0] < b'0' {
        s = 1;
        chrs += print_strn(print, &buf[..1], 0, 0, 1);
        width -= 1;
        len -= 1;
    }
    chrs += print_strn(print, &buf[s..s + len], flags, fill, width);
    chrs
}

/// `mp_printf`
pub fn printf<'a>(print: &Print, fmt: &str, args: impl IntoIterator<Item = VaArg<'a>>) -> i32 {
    vprintf(print, fmt, args)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_printf() {
        let mut out = Vec::new();
        let mut print = Print {
            data: &mut out as *mut Vec<u8> as *mut (),
            print_strn: Some(collect_print),
        };
        vprintf(
            &print,
            "x=%d",
            [VaArg::Int(42)],
        );
        assert_eq!(out, b"x=42");
    }

    extern "C" fn collect_print(data: *mut (), str: *const u8, len: usize) {
        let out = unsafe { &mut *(data as *mut Vec<u8>) };
        out.extend_from_slice(unsafe { std::slice::from_raw_parts(str, len) });
    }

    #[test]
    fn json_separators_from_print_ext() {
        let mut ext = PrintExt {
            base: Print {
                data: core::ptr::null_mut(),
                print_strn: None,
            },
            item_separator: b"|\0".as_ptr(),
            key_separator: b"=>\0".as_ptr(),
        };
        assert_eq!(json_item_separator(&ext.base), "|");
        assert_eq!(json_key_separator(&ext.base), "=>");
    }
}
