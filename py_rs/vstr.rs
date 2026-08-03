//! rewrite of py/vstr.c (+ vstr_t from py/misc.h)
// symmetry: done

use crate::malloc;
use crate::misc::{self, Byte};
use crate::mpconfig;
use crate::mpprint::{self, Print, VaArg};
use crate::raise::{self, MpRaise};

/// Variable-length string buffer (`vstr_t`).
#[repr(C)]
pub struct Vstr {
    pub alloc: usize,
    pub len: usize,
    pub buf: *mut u8,
    pub fixed_buf: bool,
}

#[inline]
const fn round_alloc(a: usize) -> usize {
    (a & !7) + 8
}

/// Init with allocation (`vstr_init`).
pub fn init(vstr: &mut Vstr, alloc: usize) {
    let alloc = if alloc < 1 { 1 } else { alloc };
    vstr.alloc = alloc;
    vstr.len = 0;
    vstr.buf = malloc::new::<u8>(alloc)
        .unwrap_or_else(|| raise::raise(MpRaise::RuntimeError("vstr init")));
    vstr.fixed_buf = false;
}

/// Init with length preset (`vstr_init_len`).
pub fn init_len(vstr: &mut Vstr, len: usize) {
    init(vstr, len + 1);
    vstr.len = len;
}

/// Init with external fixed buffer (`vstr_init_fixed_buf`).
pub fn init_fixed_buf(vstr: &mut Vstr, alloc: usize, buf: *mut u8) {
    vstr.alloc = alloc;
    vstr.len = 0;
    vstr.buf = buf;
    vstr.fixed_buf = true;
}

/// Bind vstr as print target (`vstr_init_print`).
pub fn init_print(vstr: &mut Vstr, alloc: usize, print: &mut Print) {
    init(vstr, alloc);
    print.data = vstr as *mut Vstr as *mut ();
    print.print_strn = Some(vstr_add_strn_print);
}

pub extern "C" fn vstr_add_strn_print(data: *mut (), str: *const u8, len: usize) {
    let vstr = unsafe { &mut *(data as *mut Vstr) };
    add_strn(vstr, unsafe { std::slice::from_raw_parts(str, len) });
}

/// Clear buffer (`vstr_clear`).
pub fn clear(vstr: &mut Vstr) {
    if !vstr.fixed_buf && !vstr.buf.is_null() {
        malloc::del(vstr.buf, vstr.alloc);
    }
    vstr.buf = std::ptr::null_mut();
}

/// Allocate new vstr (`vstr_new`).
pub fn new(alloc: usize) -> *mut Vstr {
    let vstr = malloc::new_obj::<Vstr>()
        .unwrap_or_else(|| raise::raise(MpRaise::RuntimeError("vstr new")));
    unsafe {
        init(&mut *vstr, alloc);
    }
    vstr
}

/// Free vstr (`vstr_free`).
pub fn free(vstr: *mut Vstr) {
    if !vstr.is_null() {
        unsafe {
            let v = &mut *vstr;
            if !v.fixed_buf && !v.buf.is_null() {
                malloc::del(v.buf, v.alloc);
            }
            malloc::del_obj(vstr);
        }
    }
}

/// Extend by exact size, return new chunk (`vstr_extend`).
pub fn extend(vstr: &mut Vstr, size: usize) -> *mut u8 {
    if vstr.fixed_buf {
        raise::raise(MpRaise::RuntimeError("vstr extend fixed"));
    }
    let new_buf = malloc::renew(vstr.buf, vstr.alloc, vstr.alloc + size)
        .unwrap_or_else(|| raise::raise(MpRaise::RuntimeError("vstr extend")));
    let p = unsafe { new_buf.add(vstr.alloc) };
    vstr.alloc += size;
    vstr.buf = new_buf;
    p
}

fn ensure_extra(vstr: &mut Vstr, size: usize) {
    if vstr.len + size > vstr.alloc {
        if vstr.fixed_buf {
            raise::raise(MpRaise::RuntimeError("vstr ensure fixed"));
        }
        let new_alloc = round_alloc(vstr.len + size + 16);
        vstr.buf = malloc::renew(vstr.buf, vstr.alloc, new_alloc)
            .unwrap_or_else(|| raise::raise(MpRaise::RuntimeError("vstr renew")));
        vstr.alloc = new_alloc;
    }
}

/// Pre-grow (`vstr_hint_size`).
pub fn hint_size(vstr: &mut Vstr, size: usize) {
    ensure_extra(vstr, size);
}

/// Add length, return new slice start (`vstr_add_len`).
pub fn add_len(vstr: &mut Vstr, len: usize) -> *mut u8 {
    ensure_extra(vstr, len);
    let buf = unsafe { vstr.buf.add(vstr.len) };
    vstr.len += len;
    buf
}

/// Ensure NUL terminator (`vstr_null_terminated_str`).
pub fn null_terminated_str(vstr: &mut Vstr) -> *mut u8 {
    if vstr.alloc == vstr.len {
        extend(vstr, 1);
    }
    unsafe {
        *vstr.buf.add(vstr.len) = 0;
    }
    vstr.buf
}

/// Append byte (`vstr_add_byte`).
pub fn add_byte(vstr: &mut Vstr, b: Byte) {
    let buf = add_len(vstr, 1);
    unsafe {
        *buf = b;
    }
}

/// Append Unicode codepoint as UTF-8 (`vstr_add_char`).
pub fn add_char(vstr: &mut Vstr, c: misc::Unichar) {
    if mpconfig::PY_BUILTINS_STR_UNICODE {
        if c < 0x80 {
            add_byte(vstr, c as u8);
        } else if c < 0x800 {
            let buf = add_len(vstr, 2);
            unsafe {
                *buf = ((c >> 6) | 0xC0) as u8;
                *buf.add(1) = ((c & 0x3F) | 0x80) as u8;
            }
        } else if c < 0x10000 {
            let buf = add_len(vstr, 3);
            unsafe {
                *buf = ((c >> 12) | 0xE0) as u8;
                *buf.add(1) = (((c >> 6) & 0x3F) | 0x80) as u8;
                *buf.add(2) = ((c & 0x3F) | 0x80) as u8;
            }
        } else {
            debug_assert!(c < 0x110000);
            let buf = add_len(vstr, 4);
            unsafe {
                *buf = ((c >> 18) | 0xF0) as u8;
                *buf.add(1) = (((c >> 12) & 0x3F) | 0x80) as u8;
                *buf.add(2) = (((c >> 6) & 0x3F) | 0x80) as u8;
                *buf.add(3) = ((c & 0x3F) | 0x80) as u8;
            }
        }
    } else {
        add_byte(vstr, c as u8);
    }
}

/// Append C string (`vstr_add_str`).
pub fn add_str(vstr: &mut Vstr, str: &str) {
    add_strn(vstr, str.as_bytes());
}

/// Append bytes (`vstr_add_strn`).
pub fn add_strn(vstr: &mut Vstr, str: &[u8]) {
    ensure_extra(vstr, str.len());
    unsafe {
        std::ptr::copy(str.as_ptr(), vstr.buf.add(vstr.len), str.len());
    }
    vstr.len += str.len();
}

/// Insert blank region (`vstr_ins_blank_bytes`).
pub fn ins_blank_bytes(vstr: &mut Vstr, byte_pos: usize, byte_len: usize) -> *mut u8 {
    let l = vstr.len;
    let byte_pos = if byte_pos > l { l } else { byte_pos };
    ensure_extra(vstr, byte_len);
    unsafe {
        std::ptr::copy(
            vstr.buf.add(byte_pos),
            vstr.buf.add(byte_pos + byte_len),
            l - byte_pos,
        );
    }
    vstr.len += byte_len;
    unsafe { vstr.buf.add(byte_pos) }
}

pub fn ins_byte(vstr: &mut Vstr, byte_pos: usize, b: Byte) {
    let s = ins_blank_bytes(vstr, byte_pos, 1);
    unsafe {
        *s = b;
    }
}

pub fn ins_char(vstr: &mut Vstr, char_pos: usize, chr: misc::Unichar) {
    let s = ins_blank_bytes(vstr, char_pos, 1);
    unsafe {
        *s = chr as u8;
    }
}

pub fn ins_strn(vstr: &mut Vstr, byte_pos: usize, str: &[u8]) {
    let s = ins_blank_bytes(vstr, byte_pos, str.len());
    unsafe {
        std::ptr::copy_nonoverlapping(str.as_ptr(), s, str.len());
    }
}

pub fn cut_head_bytes(vstr: &mut Vstr, bytes_to_cut: usize) {
    cut_out_bytes(vstr, 0, bytes_to_cut);
}

pub fn cut_tail_bytes(vstr: &mut Vstr, len: usize) {
    if len > vstr.len {
        vstr.len = 0;
    } else {
        vstr.len -= len;
    }
}

pub fn cut_out_bytes(vstr: &mut Vstr, byte_pos: usize, bytes_to_cut: usize) {
    if byte_pos >= vstr.len {
        return;
    }
    if byte_pos + bytes_to_cut >= vstr.len {
        vstr.len = byte_pos;
    } else {
        unsafe {
            std::ptr::copy(
                vstr.buf.add(byte_pos + bytes_to_cut),
                vstr.buf.add(byte_pos),
                vstr.len - byte_pos - bytes_to_cut,
            );
        }
        vstr.len -= bytes_to_cut;
    }
}

pub fn reset(vstr: &mut Vstr) {
    vstr.len = 0;
}

pub fn str_ptr(vstr: &Vstr) -> *mut u8 {
    vstr.buf
}

pub fn len(vstr: &Vstr) -> usize {
    vstr.len
}

/// Formatted append (`vstr_vprintf` / `vstr_printf`).
pub fn vprintf<'a>(vstr: &mut Vstr, fmt: &str, args: impl IntoIterator<Item = VaArg<'a>>) {
    let print = Print {
        data: vstr as *mut Vstr as *mut (),
        print_strn: Some(vstr_add_strn_print),
    };
    mpprint::vprintf(&print, fmt, args);
}

pub fn printf<'a>(vstr: &mut Vstr, fmt: &str, args: impl IntoIterator<Item = VaArg<'a>>) {
    vprintf(vstr, fmt, args);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn append_and_cut() {
        crate::gc::init();
        let mut v = Vstr {
            alloc: 0,
            len: 0,
            buf: std::ptr::null_mut(),
            fixed_buf: false,
        };
        init(&mut v, 4);
        add_str(&mut v, "hi");
        assert_eq!(v.len, 2);
        cut_tail_bytes(&mut v, 1);
        assert_eq!(v.len, 1);
        clear(&mut v);
    }
}
