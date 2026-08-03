//! rewrite of py/objstrunicode.c (unicode `str` type)
// symmetry: done

use core::mem::size_of;

use crate::mpconfig;
use crate::mpprint::{self, Print, PrintKind};
use crate::obj::{self, IterNextFn, Obj, ObjBase, ObjIterBuf, ObjType, OBJ_SENTINEL};
use crate::objpolyiter;
use crate::objslice;
use crate::objstr::{
    get_buffer, get_str_data_len, new_str, new_str_of_type, new_str_via_qstr, str_binary_op,
    str_make_new, str_print_json,
};
use crate::qstr;
use crate::raise::{self, MpRaise};
use crate::runtime0::UnaryOp;
use crate::unicode::{
    utf8_charlen, utf8_get_char, utf8_is_cont, utf8_is_nonascii, utf8_next_char, utf8_ptr_to_index,
};

// --- unicode str iterator -----------------------------------------------------

#[repr(C)]
struct ObjStrIter {
    base: ObjBase,
    iternext: IterNextFn,
    str: Obj,
    cur: usize,
}

fn str_it_iternext(self_in: Obj) -> Obj {
    let self_ = unsafe { &mut *(obj::as_ptr(self_in) as *mut ObjStrIter) };
    let (str_data, len) = get_str_data_len(self_.str);
    if self_.cur < len {
        let cur = &str_data[self_.cur..];
        let next = utf8_next_char(cur);
        // C: `end - cur` byte length of the current character (not `next.len()`).
        let char_len = cur.len() - next.len();
        let o_out = new_str_via_qstr(&cur[..char_len]);
        self_.cur += char_len;
        o_out
    } else {
        obj::OBJ_STOP_ITERATION
    }
}

fn new_str_iterator(str: Obj, iter_buf: *mut ObjIterBuf) -> Obj {
    debug_assert!(size_of::<ObjStrIter>() <= size_of::<ObjIterBuf>());
    let o = unsafe { &mut *(iter_buf as *mut ObjStrIter) };
    o.base.type_ = objpolyiter::type_polymorph_iter() as *const ObjType;
    o.iternext = str_it_iternext;
    o.str = str;
    o.cur = 0;
    obj::from_ptr(o as *const ObjStrIter as *const ())
}

// --- unicode print ------------------------------------------------------------

fn uni_print_quoted(print: &Print, str_data: &[u8]) {
    let mut has_single_quote = false;
    let mut has_double_quote = false;
    for &b in str_data {
        if b == b'\'' {
            has_single_quote = true;
        } else if b == b'"' {
            has_double_quote = true;
        }
    }
    let mut quote_char = b'\'';
    if has_single_quote && !has_double_quote {
        quote_char = b'"';
    }
    let _ = mpprint::printf(print, "%c", [mpprint::VaArg::Char(quote_char)]);
    let mut s = str_data;
    while !s.is_empty() {
        let seq_start = s;
        let ch = utf8_get_char(s);
        s = utf8_next_char(s);
        let seq_len = s.as_ptr() as usize - seq_start.as_ptr() as usize;
        if ch == quote_char as u32 {
            let _ = mpprint::printf(print, "\\%c", [mpprint::VaArg::Char(quote_char)]);
        } else if ch == b'\\' as u32 {
            mpprint::print_str(print, "\\\\");
        } else if (32..=126).contains(&ch) {
            let _ = mpprint::printf(print, "%c", [mpprint::VaArg::Char(ch as u8)]);
        } else if ch == b'\n' as u32 {
            mpprint::print_str(print, "\\n");
        } else if ch == b'\r' as u32 {
            mpprint::print_str(print, "\\r");
        } else if ch == b'\t' as u32 {
            mpprint::print_str(print, "\\t");
        } else if ch <= 127 {
            let _ = mpprint::printf(print, "\\x%02x", [mpprint::VaArg::UInt(ch as u32)]);
        } else if ch < 0xD800 {
            if let Some(f) = print.print_strn {
                f(print.data, seq_start.as_ptr(), seq_len);
            }
        } else if ch >= 0xE000 && ch < 0x110000 {
            if let Some(f) = print.print_strn {
                f(print.data, seq_start.as_ptr(), seq_len);
            }
        } else if ch < 0x10000 {
            let _ = mpprint::printf(print, "\\u%04x", [mpprint::VaArg::UInt(ch)]);
        } else {
            let _ = mpprint::printf(print, "\\U%08x", [mpprint::VaArg::UInt(ch)]);
        }
    }
    let _ = mpprint::printf(print, "%c", [mpprint::VaArg::Char(quote_char)]);
}

fn uni_print(print: &Print, self_in: Obj, kind: PrintKind) {
    let (str_data, str_len) = get_str_data_len(self_in);
    let str_data = &str_data[..str_len];
    if mpconfig::PY_JSON && kind == PrintKind::Json {
        str_print_json(print, str_data);
        return;
    }
    if kind == PrintKind::Str {
        if let Some(f) = print.print_strn {
            f(print.data, str_data.as_ptr(), str_len);
        }
    } else {
        uni_print_quoted(print, str_data);
    }
}

fn uni_unary_op(op: UnaryOp, self_in: Obj) -> Obj {
    let (str_data, str_len) = get_str_data_len(self_in);
    let str_data = &str_data[..str_len];
    match op {
        UnaryOp::Bool => obj::new_bool(str_len != 0),
        UnaryOp::Len => obj::new_small_int(utf8_charlen(str_data, str_len) as obj::Int),
        _ => obj::OBJ_NULL,
    }
}

/// Unicode-aware index → byte pointer (`str_index_to_ptr` from objstrunicode.c).
pub fn str_index_to_ptr(
    type_: &ObjType,
    self_data: &[u8],
    self_len: usize,
    index: Obj,
    is_slice: bool,
) -> *const u8 {
    if core::ptr::eq(type_, crate::objstr::type_bytes()) {
        let index_val = obj::get_index(type_, self_len, index, is_slice);
        return self_data.as_ptr().wrapping_add(index_val);
    }

    let i = if obj::is_small_int(index) {
        obj::small_int_value(index)
    } else {
        let mut tmp = 0;
        if !obj::get_int_maybe(index, &mut tmp) {
            raise::raise(MpRaise::TypeError("string indices must be integers"));
        }
        tmp
    };

    let self_data = &self_data[..self_len];
    if i < 0 {
        let mut s_idx = self_len;
        let mut rem = i;
        while rem != 0 {
            if s_idx == 0 {
                if is_slice {
                    return self_data.as_ptr();
                }
                raise::raise(MpRaise::TypeError("string index out of range"));
            }
            s_idx -= 1;
            if !utf8_is_cont(self_data[s_idx]) {
                rem += 1;
            }
        }
        return self_data.as_ptr().wrapping_add(s_idx);
    }

    let mut s_idx = 0usize;
    let mut rem = i;
    loop {
        if s_idx >= self_len {
            if is_slice {
                return self_data.as_ptr().wrapping_add(self_len);
            }
            raise::raise(MpRaise::TypeError("string index out of range"));
        }
        if rem == 0 {
            break;
        }
        rem -= 1;
        s_idx += 1;
        while s_idx < self_len && utf8_is_cont(self_data[s_idx]) {
            s_idx += 1;
        }
    }
    self_data.as_ptr().wrapping_add(s_idx)
}

fn str_subscr(self_in: Obj, index: Obj, value: Obj) -> Obj {
    let type_ = obj::get_type(self_in);
    debug_assert!(core::ptr::eq(type_, type_str()));
    let (self_data, self_len) = get_str_data_len(self_in);
    let self_data = &self_data[..self_len];
    if value == OBJ_SENTINEL {
        if mpconfig::PY_BUILTINS_SLICE && obj::is_exact_type(index, objslice::type_slice()) {
            let slice = unsafe { &*(obj::as_ptr(index) as *const objslice::ObjSlice) };
            if slice.step != obj::CONST_NONE && slice.step != obj::new_small_int(1) {
                raise::raise(MpRaise::RuntimeError("only slices with step=1 supported"));
            }
            let pstart = if slice.start != obj::CONST_NONE {
                str_index_to_ptr(type_, self_data, self_len, slice.start, true)
            } else {
                self_data.as_ptr()
            };
            let pstop = if slice.stop != obj::CONST_NONE {
                str_index_to_ptr(type_, self_data, self_len, slice.stop, true)
            } else {
                self_data.as_ptr().wrapping_add(self_len)
            };
            if pstop < pstart {
                return obj::new_qstr(qstr::QSTR_EMPTY);
            }
            let len = unsafe { pstop.offset_from(pstart) } as usize;
            return new_str_of_type(type_, unsafe { std::slice::from_raw_parts(pstart, len) });
        }
        let s = str_index_to_ptr(type_, self_data, self_len, index, false);
        let mut len = 1usize;
        if utf8_is_nonascii(unsafe { *s }) {
            let mut mask = 0x40i8;
            while unsafe { *s } & mask as u8 != 0 {
                len += 1;
                mask >>= 1;
            }
        }
        new_str_via_qstr(unsafe { std::slice::from_raw_parts(s, len) })
    } else {
        obj::OBJ_NULL
    }
}

// --- type vtable --------------------------------------------------------------

static mut STR_SLOTS: [*const (); 8] = [
    str_make_new as *const (),
    uni_print as *const (),
    uni_unary_op as *const (),
    str_binary_op as *const (),
    str_subscr as *const (),
    new_str_iterator as *const (),
    get_buffer as *const (),
    core::ptr::null(),
];

static mut TYPE: ObjType = ObjType {
    base: ObjBase {
        type_: core::ptr::null(),
    },
    flags: obj::TYPE_FLAG_EQ_NOT_REFLEXIVE,
    name: 0,
    slot_index_make_new: 1,
    slot_index_print: 2,
    slot_index_call: 0,
    slot_index_unary_op: 3,
    slot_index_binary_op: 4,
    slot_index_attr: 0,
    slot_index_subscr: 5,
    slot_index_iter: 6,
    slot_index_buffer: 7,
    slot_index_protocol: 0,
    slot_index_parent: 0,
    slot_index_locals_dict: 8,
    slots: unsafe { STR_SLOTS.as_ptr() },
};

static STR_INIT: std::sync::OnceLock<()> = std::sync::OnceLock::new();

fn init_str_type() {
    STR_INIT.get_or_init(|| unsafe {
        (*(core::ptr::addr_of_mut!(TYPE) as *mut ObjType)).name = qstr::from_str("str");
        STR_SLOTS[7] = crate::objstr::str_locals_dict_obj().0 as *const ();
    });
}

pub fn type_str() -> &'static ObjType {
    init_str_type();
    unsafe { &*core::ptr::addr_of!(TYPE) }
}

pub fn utf8_index_to_byte_offset(haystack: &[u8], ptr: *const u8) -> Obj {
    obj::new_small_int(
        utf8_ptr_to_index(haystack, ptr as usize - haystack.as_ptr() as usize) as obj::Int,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gc;
    use crate::qstr;

    fn setup() {
        let _ = gc::init();
        qstr::init();
    }

    #[test]
    fn str_type_has_slots() {
        setup();
        let t = type_str();
        assert!(obj::type_get_make_new(t).is_some());
        assert!(obj::type_get_print(t).is_some());
        assert!(obj::type_get_binary_op(t).is_some());
    }

    #[test]
    fn str_len_unicode() {
        setup();
        let s = new_str("aβ".as_bytes());
        let len = uni_unary_op(UnaryOp::Len, s);
        assert_eq!(obj::small_int_value(len), 2);
    }
}
