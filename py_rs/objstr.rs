//! rewrite of py/objstr.c + py/objstr.h
// symmetry: done
use core::mem::size_of;

use crate::argcheck;
use crate::cstack;
use crate::map::{self, LookupKind, Map, MapElem};
use crate::malloc;
use crate::mpconfig;
use crate::mpprint::{self, Print, PrintKind, PF_FLAG_ADD_PERCENT, PF_FLAG_CENTER_ADJUST, PF_FLAG_LEFT_ADJUST, PF_FLAG_PAD_AFTER_SIGN, PF_FLAG_SEP_POS, PF_FLAG_SHOW_OCTAL_LETTER, PF_FLAG_SHOW_PREFIX, PF_FLAG_SHOW_SIGN, PF_FLAG_SPACE_SIGN};
use crate::obj::{
    self, BufferInfo, Obj, ObjBase, ObjIterBuf, ObjType, OBJ_SENTINEL, TYPE_FLAG_BINDS_SELF,
    TYPE_FLAG_BUILTIN_FUN, TYPE_FLAG_EQ_NOT_REFLEXIVE,
};
use crate::objdict::{self, ObjDict};
use crate::objexcept;
use crate::objfloat::{self, MpFloat};
use crate::objlist;
use crate::objpolyiter;
use crate::objslice;
use crate::objtuple;
use crate::qstr::{self, Qstr};
use crate::raise::{self, MpRaise};
use crate::runtime;
use crate::runtime0::{BinaryOp, UnaryOp};
use crate::sequence;
use crate::unicode::{self, Encoding, utf8_charlen, utf8_next_char, utf8_ptr_to_index, unichar_isalpha, unichar_isdigit, unichar_islower, unichar_isupper, unichar_isspace, unichar_isxdigit, unichar_tolower, unichar_toupper, unichar_xdigit_value};
use crate::vstr::{self, Vstr};

#[repr(C)]
pub struct ObjStr {
    pub base: ObjBase,
    pub hash: usize,
    pub len: usize,
    pub data: *const u8,
}

// --- builtin wrappers ---------------------------------------------------------

type BuiltinFn1 = fn(Obj) -> Obj;
type BuiltinFn2 = fn(Obj, Obj) -> Obj;
type BuiltinFnVar = fn(usize, &[Obj]) -> Obj;
type BuiltinFnKw = fn(usize, &[Obj], &mut Map) -> Obj;

#[repr(C)]
struct ObjFunBuiltin1 { base: ObjBase, fun: BuiltinFn1 }
#[repr(C)]
struct ObjFunBuiltin2 { base: ObjBase, fun: BuiltinFn2 }
#[repr(C)]
struct ObjFunBuiltinVar { base: ObjBase, min_args: u8, max_args: u8, fun: BuiltinFnVar }
#[repr(C)]
struct ObjFunBuiltinKw { base: ObjBase, min_args: u8, fun: BuiltinFnKw }

static mut FUN_BUILTIN_1_SLOTS: [*const (); 1] = [fun_builtin_1_call as *const ()];
static mut FUN_BUILTIN_2_SLOTS: [*const (); 1] = [fun_builtin_2_call as *const ()];
static mut FUN_BUILTIN_VAR_SLOTS: [*const (); 1] = [fun_builtin_var_call as *const ()];
static mut FUN_BUILTIN_KW_SLOTS: [*const (); 1] = [fun_builtin_kw_call as *const ()];

static TYPE_FUN_BUILTIN_1: ObjType = ObjType {
    base: ObjBase { type_: core::ptr::null() }, flags: TYPE_FLAG_BINDS_SELF | TYPE_FLAG_BUILTIN_FUN,
    name: 0, slot_index_make_new: 0, slot_index_print: 0, slot_index_call: 1,
    slot_index_unary_op: 0, slot_index_binary_op: 0, slot_index_attr: 0, slot_index_subscr: 0,
    slot_index_iter: 0, slot_index_buffer: 0, slot_index_protocol: 0, slot_index_parent: 0,
    slot_index_locals_dict: 0, slots: unsafe { FUN_BUILTIN_1_SLOTS.as_ptr() },
};
static TYPE_FUN_BUILTIN_2: ObjType = ObjType {
    base: ObjBase { type_: core::ptr::null() }, flags: TYPE_FLAG_BINDS_SELF | TYPE_FLAG_BUILTIN_FUN,
    name: 0, slot_index_make_new: 0, slot_index_print: 0, slot_index_call: 1,
    slot_index_unary_op: 0, slot_index_binary_op: 0, slot_index_attr: 0, slot_index_subscr: 0,
    slot_index_iter: 0, slot_index_buffer: 0, slot_index_protocol: 0, slot_index_parent: 0,
    slot_index_locals_dict: 0, slots: unsafe { FUN_BUILTIN_2_SLOTS.as_ptr() },
};
static TYPE_FUN_BUILTIN_VAR: ObjType = ObjType {
    base: ObjBase { type_: core::ptr::null() }, flags: TYPE_FLAG_BINDS_SELF | TYPE_FLAG_BUILTIN_FUN,
    name: 0, slot_index_make_new: 0, slot_index_print: 0, slot_index_call: 1,
    slot_index_unary_op: 0, slot_index_binary_op: 0, slot_index_attr: 0, slot_index_subscr: 0,
    slot_index_iter: 0, slot_index_buffer: 0, slot_index_protocol: 0, slot_index_parent: 0,
    slot_index_locals_dict: 0, slots: unsafe { FUN_BUILTIN_VAR_SLOTS.as_ptr() },
};
static TYPE_FUN_BUILTIN_KW: ObjType = ObjType {
    base: ObjBase { type_: core::ptr::null() }, flags: TYPE_FLAG_BINDS_SELF | TYPE_FLAG_BUILTIN_FUN,
    name: 0, slot_index_make_new: 0, slot_index_print: 0, slot_index_call: 1,
    slot_index_unary_op: 0, slot_index_binary_op: 0, slot_index_attr: 0, slot_index_subscr: 0,
    slot_index_iter: 0, slot_index_buffer: 0, slot_index_protocol: 0, slot_index_parent: 0,
    slot_index_locals_dict: 0, slots: unsafe { FUN_BUILTIN_KW_SLOTS.as_ptr() },
};

fn fun_builtin_1_call(self_in: Obj, n_args: usize, n_kw: usize, args: &[Obj]) -> Obj {
    argcheck::check_num(n_args, n_kw, 1, 1, false);
    (unsafe { &*(obj::as_ptr(self_in) as *const ObjFunBuiltin1) }.fun)(args[0])
}
fn fun_builtin_2_call(self_in: Obj, n_args: usize, n_kw: usize, args: &[Obj]) -> Obj {
    argcheck::check_num(n_args, n_kw, 2, 2, false);
    (unsafe { &*(obj::as_ptr(self_in) as *const ObjFunBuiltin2) }.fun)(args[0], args[1])
}
fn fun_builtin_var_call(self_in: Obj, n_args: usize, n_kw: usize, args: &[Obj]) -> Obj {
    let self_ = unsafe { &*(obj::as_ptr(self_in) as *const ObjFunBuiltinVar) };
    argcheck::check_num(n_args, n_kw, self_.min_args as usize, self_.max_args as usize, false);
    (self_.fun)(n_args, args)
}
fn fun_builtin_kw_call(self_in: Obj, n_args: usize, n_kw: usize, args: &[Obj]) -> Obj {
    let self_ = unsafe { &*(obj::as_ptr(self_in) as *const ObjFunBuiltinKw) };
    argcheck::check_num(n_args, n_kw, self_.min_args as usize, usize::MAX, false);
    let mut kwargs = Map::default();
    if n_kw != 0 {
        for i in 0..n_kw {
            let key = args[n_args + 2 * i];
            let value = args[n_args + 2 * i + 1];
            if let Some(elem) = map::lookup(&mut kwargs, key, LookupKind::AddIfNotFound) {
                elem.value = value;
            }
        }
    }
    (self_.fun)(n_args, args, &mut kwargs)
}
fn new_fun_builtin_1(fun: BuiltinFn1) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltin1>().expect("fun1");
    unsafe { (*o).base.type_ = &TYPE_FUN_BUILTIN_1 as *const ObjType; (*o).fun = fun; obj::from_ptr(o as *const ObjFunBuiltin1 as *const ()) }
}
fn new_fun_builtin_2(fun: BuiltinFn2) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltin2>().expect("fun2");
    unsafe { (*o).base.type_ = &TYPE_FUN_BUILTIN_2 as *const ObjType; (*o).fun = fun; obj::from_ptr(o as *const ObjFunBuiltin2 as *const ()) }
}
fn new_fun_builtin_var(min: u8, max: u8, fun: BuiltinFnVar) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltinVar>().expect("funv");
    unsafe { (*o).base.type_ = &TYPE_FUN_BUILTIN_VAR as *const ObjType; (*o).min_args = min; (*o).max_args = max; (*o).fun = fun; obj::from_ptr(o as *const ObjFunBuiltinVar as *const ()) }
}
fn new_fun_builtin_kw(min: u8, fun: BuiltinFnKw) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltinKw>().expect("funkw");
    unsafe { (*o).base.type_ = &TYPE_FUN_BUILTIN_KW as *const ObjType; (*o).min_args = min; (*o).fun = fun; obj::from_ptr(o as *const ObjFunBuiltinKw as *const ()) }
}

// --- data access --------------------------------------------------------------

fn str_type_of(o: Obj) -> &'static ObjType {
    if mpconfig::PY_BUILTINS_STR_UNICODE && obj::is_str(o) && !obj::is_qstr(o) {
        crate::objstrunicode::type_str()
    } else if obj::is_exact_type(o, type_bytes()) {
        type_bytes()
    } else {
        type_str()
    }
}

fn heap_str(o: Obj) -> Option<&'static ObjStr> {
    if obj::is_qstr(o) { return None; }
    if obj::is_exact_type(o, type_bytes()) || obj::is_exact_type(o, type_str()) {
        Some(unsafe { &*(obj::as_ptr(o) as *const ObjStr) })
    } else { None }
}

pub fn get_str_data_len(o: Obj) -> (Vec<u8>, usize) {
    if obj::is_qstr(o) {
        let q = obj::qstr_value(o);
        let len = qstr::qstr_len(q).unwrap_or(0);
        let mut data = qstr::str_data(q).unwrap_or_default();
        data.truncate(len);
        (data, len)
    } else if let Some(s) = heap_str(o) {
        let data = unsafe { std::slice::from_raw_parts(s.data, s.len) }.to_vec();
        (data, s.len)
    } else {
        bad_implicit_conversion(o);
    }
}

/// Borrow bytes from a str/bytes object when the backing storage outlives the call.
pub fn with_str_bytes<R>(o: Obj, f: impl FnOnce(*const u8, usize) -> R) -> R {
    if obj::is_qstr(o) {
        let q = obj::qstr_value(o);
        let len = qstr::qstr_len(q).unwrap_or(0);
        if let Some((data, _)) = qstr::qstr_data(q) {
            return f(data.as_ptr(), len);
        }
        bad_implicit_conversion(o);
    } else if let Some(s) = heap_str(o) {
        f(s.data, s.len)
    } else {
        bad_implicit_conversion(o);
    }
}

fn get_str_hash(o: Obj) -> usize {
    if obj::is_qstr(o) { qstr::qstr_hash(obj::qstr_value(o)).unwrap_or(0) }
    else if let Some(s) = heap_str(o) { s.hash }
    else { 0 }
}

fn bad_implicit_conversion(_: Obj) -> ! { raise::raise(MpRaise::TypeError("can't convert to str implicitly")); }

fn check_is_str_or_bytes(o: Obj) {
    if !obj::is_str_or_bytes(o) { raise::raise(MpRaise::TypeError("str/bytes method on wrong type")); }
}

fn str_check_arg_type(self_type: &ObjType, arg: Obj) {
    if obj::get_type(arg) as *const ObjType != self_type as *const ObjType {
        bad_implicit_conversion(arg);
    }
}

fn make_empty_str_of_type(type_: &ObjType) -> Obj {
    if core::ptr::eq(type_, type_bytes()) { const_empty_bytes() } else { obj::new_qstr(qstr::QSTR_EMPTY) }
}

fn index_to_ptr(type_: &ObjType, data: &[u8], len: usize, index: Obj, is_slice: bool) -> usize {
    if mpconfig::PY_BUILTINS_STR_UNICODE {
        let p = crate::objstrunicode::str_index_to_ptr(type_, data, len, index, is_slice);
        p as usize - data.as_ptr() as usize
    } else {
        obj::get_index(type_, len, index, is_slice)
    }
}

pub fn find_subbytes(haystack: &[u8], needle: &[u8], direction: i32) -> Option<usize> {
    if haystack.len() < needle.len() { return None; }
    let (mut i, end) = if direction > 0 { (0usize, haystack.len() - needle.len()) } else { (haystack.len() - needle.len(), 0) };
    loop {
        if haystack[i..i + needle.len()] == *needle { return Some(i); }
        if i == end { break; }
        i = if direction > 0 { i + 1 } else { i - 1 };
    }
    None
}

fn get_substring_data(obj_in: Obj, n_args: usize, args: &[Obj]) -> (Vec<u8>, usize) {
    let (mut data, mut len) = get_str_data_len(obj_in);
    let type_ = obj::get_type(obj_in);
    if n_args > 0 {
        let mut end = len;
        if n_args > 1 && args[1] != obj::CONST_NONE {
            end = index_to_ptr(type_, &data, len, args[1], true);
        }
        let mut start = 0usize;
        if args[0] != obj::CONST_NONE {
            start = index_to_ptr(type_, &data, len, args[0], true);
        }
        len = end.saturating_sub(start);
        data = data[start..start + len].to_vec();
    }
    (data, len)
}

// --- print --------------------------------------------------------------------

pub fn str_print_quoted(print: &Print, str_data: &[u8], is_bytes: bool) {
    let mut sq = false;
    let mut dq = false;
    for &b in str_data {
        if b == b'\'' { sq = true; } else if b == b'"' { dq = true; }
    }
    let qc = if sq && !dq { b'"' } else { b'\'' };
    let _ = mpprint::printf(print, "%c", [mpprint::VaArg::Char(qc)]);
    for &b in str_data {
        if b == qc { let _ = mpprint::printf(print, "\\%c", [mpprint::VaArg::Char(qc)]); }
        else if b == b'\\' { mpprint::print_str(print, "\\\\"); }
        else if b >= 0x20 && b != 0x7f && (!is_bytes || b < 0x80) { let _ = mpprint::printf(print, "%c", [mpprint::VaArg::Char(b)]); }
        else if b == b'\n' { mpprint::print_str(print, "\\n"); }
        else if b == b'\r' { mpprint::print_str(print, "\\r"); }
        else if b == b'\t' { mpprint::print_str(print, "\\t"); }
        else { let _ = mpprint::printf(print, "\\x%02x", [mpprint::VaArg::UInt(b as u32)]); }
    }
    let _ = mpprint::printf(print, "%c", [mpprint::VaArg::Char(qc)]);
}

pub fn str_print_json(print: &Print, str_data: &[u8]) {
    mpprint::print_str(print, "\"");
    for &b in str_data {
        if b == b'"' || b == b'\\' { let _ = mpprint::printf(print, "\\%c", [mpprint::VaArg::Char(b)]); }
        else if b >= 32 { let _ = mpprint::printf(print, "%c", [mpprint::VaArg::Char(b)]); }
        else if b == b'\n' { mpprint::print_str(print, "\\n"); }
        else if b == b'\r' { mpprint::print_str(print, "\\r"); }
        else if b == b'\t' { mpprint::print_str(print, "\\t"); }
        else { let _ = mpprint::printf(print, "\\u%04x", [mpprint::VaArg::UInt(b as u32)]); }
    }
    mpprint::print_str(print, "\"");
}

fn str_print(print: &Print, self_in: Obj, kind: PrintKind) {
    let (data, len) = get_str_data_len(self_in);
    if mpconfig::PY_JSON && kind == PrintKind::Json {
        str_print_json(print, &data[..len]);
        return;
    }
    let is_bytes = if mpconfig::PY_BUILTINS_STR_UNICODE { true } else { obj::is_exact_type(self_in, type_bytes()) };
    if kind == PrintKind::Raw || (!mpconfig::PY_BUILTINS_STR_UNICODE && kind == PrintKind::Str && !is_bytes) {
        if let Some(f) = print.print_strn { f(print.data, data.as_ptr(), len); }
    } else {
        if is_bytes { if let Some(f) = print.print_strn { f(print.data, b"b".as_ptr(), 1); } }
        str_print_quoted(print, &data[..len], is_bytes);
    }
}

// --- construction -------------------------------------------------------------

pub fn new_str_type_from_vstr(type_: &ObjType, vstr: &mut Vstr) -> Obj {
    if mpconfig::PY_BUILTINS_STR_UNICODE && core::ptr::eq(type_, type_str()) {
        let q = qstr::find_strn(unsafe { std::slice::from_raw_parts(vstr.buf, vstr.len) });
        if q != qstr::QSTR_NULL {
            vstr::clear(vstr);
            vstr.alloc = 0;
            return obj::new_qstr(q);
        }
    }
    let data = if vstr.len + 1 == vstr.alloc {
        vstr.buf
    } else {
        malloc::renew(vstr.buf, vstr.alloc, vstr.len + 1).expect("renew str")
    };
    unsafe { *data.add(vstr.len) = 0; }
    vstr.buf = core::ptr::null_mut();
    vstr.alloc = 0;
    let o = malloc::new_obj::<ObjStr>().expect("str obj");
    unsafe {
        (*o).base.type_ = type_ as *const ObjType;
        (*o).len = vstr.len;
        (*o).hash = qstr::compute_hash(std::slice::from_raw_parts(data, vstr.len));
        (*o).data = data;
        vstr::clear(vstr);
        obj::from_ptr(o as *const ObjStr as *const ())
    }
}

pub fn new_str_copy(type_: &ObjType, data: Option<&[u8]>, len: usize) -> Obj {
    let o = malloc::new_obj::<ObjStr>().expect("str copy");
    unsafe {
        (*o).base.type_ = type_ as *const ObjType;
        (*o).len = len;
        if let Some(data) = data {
            (*o).hash = qstr::compute_hash(data);
            let p = malloc::new::<u8>(len + 1).expect("str data");
            std::ptr::copy_nonoverlapping(data.as_ptr(), p, len);
            *p.add(len) = 0;
            (*o).data = p;
        } else {
            (*o).hash = 0;
            (*o).data = core::ptr::null();
        }
        obj::from_ptr(o as *const ObjStr as *const ())
    }
}

pub fn new_str_of_type(type_: &ObjType, data: &[u8]) -> Obj {
    if mpconfig::PY_BUILTINS_STR_UNICODE && core::ptr::eq(type_, type_str()) {
        return new_str(data);
    }
    new_bytes(data)
}

pub fn new_str_via_qstr(data: &[u8]) -> Obj { obj::new_qstr(qstr::from_strn(data)) }

pub fn new_str(data: &[u8]) -> Obj {
    if mpconfig::PY_BUILTINS_STR_UNICODE && mpconfig::PY_BUILTINS_STR_UNICODE_CHECK {
        if !unicode::unicode_encoding_check(Encoding::Utf8, data) {
            raise::raise(MpRaise::RuntimeError("UnicodeError"));
        }
    }
    let q = qstr::find_strn(data);
    if q != qstr::QSTR_NULL {
        return obj::new_qstr(q);
    }
    new_str_copy(type_str(), Some(data), data.len())
}

pub fn new_str_from_vstr(vstr: &mut Vstr) -> Obj {
    if mpconfig::PY_BUILTINS_STR_UNICODE && mpconfig::PY_BUILTINS_STR_UNICODE_CHECK {
        let data = unsafe { std::slice::from_raw_parts(vstr.buf, vstr.len) };
        if !unicode::unicode_encoding_check(Encoding::Utf8, data) {
            raise::raise(MpRaise::RuntimeError("UnicodeError"));
        }
    }
    new_str_type_from_vstr(type_str(), vstr)
}

pub fn new_bytes_from_vstr(vstr: &mut Vstr) -> Obj { new_str_type_from_vstr(type_bytes(), vstr) }

pub fn new_bytes(data: &[u8]) -> Obj { new_str_copy(type_bytes(), Some(data), data.len()) }

/// Create a str object referencing ROM storage (persistent / VFS_ROM load).
pub fn new_str_from_rom(data: *const u8, len: usize) -> Obj {
    let slice = unsafe { std::slice::from_raw_parts(data, len) };
    let q = qstr::find_strn(slice);
    if q != qstr::QSTR_NULL {
        return obj::new_qstr(q);
    }
    debug_assert_eq!(unsafe { *data.add(len) }, 0);
    let o = malloc::new_obj::<ObjStr>().expect("rom str");
    unsafe {
        (*o).base.type_ = type_str() as *const ObjType;
        (*o).len = len;
        (*o).hash = qstr::compute_hash(slice);
        (*o).data = data;
        obj::from_ptr(o as *const ObjStr as *const ())
    }
}

/// Create a bytes object referencing ROM storage (persistent / VFS_ROM load).
pub fn new_bytes_from_rom(data: *const u8, len: usize) -> Obj {
    let o = malloc::new_obj::<ObjStr>().expect("rom bytes");
    unsafe {
        (*o).base.type_ = type_bytes() as *const ObjType;
        (*o).len = len;
        (*o).hash = qstr::compute_hash(std::slice::from_raw_parts(data, len));
        (*o).data = data;
        obj::from_ptr(o as *const ObjStr as *const ())
    }
}

pub fn str_set_data(str: &mut ObjStr, data: *const u8, len: usize) {
    str.data = data;
    str.len = len;
    str.hash = qstr::compute_hash(unsafe { std::slice::from_raw_parts(data, len) });
}

// --- equality / accessors -----------------------------------------------------

pub fn str_equal(a: Obj, b: Obj) -> bool {
    if obj::is_qstr(a) && obj::is_qstr(b) { return a == b; }
    let h1 = get_str_hash(a);
    let h2 = get_str_hash(b);
    if h1 != 0 && h2 != 0 && h1 != h2 { return false; }
    let (d1, l1) = get_str_data_len(a);
    let (d2, l2) = get_str_data_len(b);
    l1 == l2 && d1[..l1] == d2[..l2]
}

pub fn str_len(o: Obj) -> usize {
    if obj::is_qstr(o) || obj::is_exact_type(o, type_str()) {
        let (data, len) = get_str_data_len(o);
        if mpconfig::PY_BUILTINS_STR_UNICODE { utf8_charlen(&data[..len], len) } else { len }
    } else if obj::is_exact_type(o, type_bytes()) {
        get_str_data_len(o).1
    } else { 0 }
}

pub fn str_get_qstr(o: Obj) -> Qstr {
    if obj::is_qstr(o) { obj::qstr_value(o) }
    else if obj::is_exact_type(o, type_str()) {
        let (data, len) = get_str_data_len(o);
        qstr::from_strn(&data[..len])
    } else { bad_implicit_conversion(o); }
}

pub fn str_get_str(o: Obj) -> String {
    let (data, len) = get_str_data_len(o);
    String::from_utf8_lossy(&data[..len]).into_owned()
}

pub fn str_get_data(o: Obj) -> (Vec<u8>, usize) {
    get_str_data_len(o)
}

pub fn type_str() -> &'static ObjType {
    if mpconfig::PY_BUILTINS_STR_UNICODE {
        crate::objstrunicode::type_str()
    } else {
        init_str_nounicode();
        unsafe { &*core::ptr::addr_of!(TYPE_STR_NOUNICODE) }
    }
}

static mut TYPE_STR_NOUNICODE: ObjType = ObjType {
    base: ObjBase { type_: core::ptr::null() },
    flags: TYPE_FLAG_EQ_NOT_REFLEXIVE,
    name: 0, slot_index_make_new: 0, slot_index_print: 0, slot_index_call: 0,
    slot_index_unary_op: 0, slot_index_binary_op: 0, slot_index_attr: 0, slot_index_subscr: 0,
    slot_index_iter: 0, slot_index_buffer: 0, slot_index_protocol: 0, slot_index_parent: 0,
    slot_index_locals_dict: 0, slots: core::ptr::null(),
};

fn init_str_nounicode() {}

static mut BYTES_SLOTS: [*const (); 7] = [
    bytes_make_new as *const (),
    str_print as *const (),
    str_binary_op as *const (),
    bytes_subscr as *const (),
    new_bytes_iterator as *const (),
    get_buffer as *const (),
    core::ptr::null(),
];

static mut TYPE_BYTES: ObjType = ObjType {
    base: ObjBase { type_: core::ptr::null() },
    flags: TYPE_FLAG_EQ_NOT_REFLEXIVE,
    name: 0,
    slot_index_make_new: 1,
    slot_index_print: 2,
    slot_index_call: 0,
    slot_index_unary_op: 0,
    slot_index_binary_op: 3,
    slot_index_attr: 0,
    slot_index_subscr: 4,
    slot_index_iter: 5,
    slot_index_buffer: 6,
    slot_index_protocol: 0,
    slot_index_parent: 0,
    slot_index_locals_dict: 7,
    slots: unsafe { BYTES_SLOTS.as_ptr() },
};

static BYTES_INIT: std::sync::OnceLock<()> = std::sync::OnceLock::new();
static EMPTY_BYTES: std::sync::OnceLock<Obj> = std::sync::OnceLock::new();
static STR_LOCALS: std::sync::OnceLock<Obj> = std::sync::OnceLock::new();

pub fn str_locals_dict_obj() -> Obj {
    init_bytes_type();
    *STR_LOCALS.get().expect("str locals")
}

pub fn type_bytes() -> &'static ObjType {
    init_bytes_type();
    unsafe { &*core::ptr::addr_of!(TYPE_BYTES) }
}

fn const_empty_bytes() -> Obj {
    init_bytes_type();
    *EMPTY_BYTES.get().expect("empty bytes")
}

fn init_bytes_type() {
    BYTES_INIT.get_or_init(|| {
        unsafe {
            (*(core::ptr::addr_of_mut!(TYPE_BYTES) as *mut ObjType)).name = qstr::from_str("bytes");
        }
        let mut table: Vec<MapElem> = vec![
            me("find", new_fun_builtin_var(2, 4, str_find)),
            me("rfind", new_fun_builtin_var(2, 4, str_rfind)),
            me("index", new_fun_builtin_var(2, 4, str_index)),
            me("rindex", new_fun_builtin_var(2, 4, str_rindex)),
            me("join", new_fun_builtin_2(str_join)),
            me("split", new_fun_builtin_var(1, 3, str_split_method)),
            me("rsplit", new_fun_builtin_var(1, 3, str_rsplit)),
            me("startswith", new_fun_builtin_var(2, 4, str_startswith)),
            me("endswith", new_fun_builtin_var(2, 4, str_endswith)),
            me("strip", new_fun_builtin_var(1, 2, str_strip)),
            me("lstrip", new_fun_builtin_var(1, 2, str_lstrip)),
            me("rstrip", new_fun_builtin_var(1, 2, str_rstrip)),
            me("format", new_fun_builtin_kw(1, str_format_kw)),
            me("replace", new_fun_builtin_var(3, 4, str_replace)),
            me("lower", new_fun_builtin_1(str_lower)),
            me("upper", new_fun_builtin_1(str_upper)),
            me("isspace", new_fun_builtin_1(str_isspace)),
            me("isalpha", new_fun_builtin_1(str_isalpha)),
            me("isdigit", new_fun_builtin_1(str_isdigit)),
            me("isupper", new_fun_builtin_1(str_isupper)),
            me("islower", new_fun_builtin_1(str_islower)),
        ];
        if mpconfig::PY_BUILTINS_STR_SPLITLINES {
            table.push(me("splitlines", new_fun_builtin_var(1, 1, str_splitlines)));
        }
        if mpconfig::PY_BUILTINS_STR_COUNT {
            table.push(me("count", new_fun_builtin_var(2, 4, str_count)));
        }
        if mpconfig::PY_BUILTINS_STR_PARTITION {
            table.push(me("partition", new_fun_builtin_2(str_partition)));
            table.push(me("rpartition", new_fun_builtin_2(str_rpartition)));
        }
        if mpconfig::PY_BUILTINS_STR_CENTER {
            table.push(me("center", new_fun_builtin_2(str_center)));
        }
        if mpconfig::PY_BUILTINS_BYTES_HEX {
            table.push(me("hex", new_fun_builtin_var(1, 2, bytes_hex_method)));
            table.push(me("fromhex", new_fun_builtin_var(1, 1, bytes_fromhex_method)));
        }
        if mpconfig::CPYTHON_COMPAT {
            table.push(me("decode", new_fun_builtin_var(1, 3, bytes_decode)));
            table.push(me("encode", new_fun_builtin_var(1, 3, str_encode)));
        }
        let ptr = obj::malloc_helper(size_of::<ObjDict>(), objdict::type_dict()) as *mut ObjDict;
        unsafe {
            map::init_fixed_table(&mut (*ptr).map, table);
            BYTES_SLOTS[6] = obj::from_ptr(ptr as *const ObjDict as *const ()).0 as *const ();
            let _ = STR_LOCALS.set(obj::from_ptr(ptr as *const ObjDict as *const ()));
            let o = malloc::new_obj::<ObjStr>().expect("empty bytes");
            unsafe {
                (*o).base.type_ = &TYPE_BYTES as *const ObjType;
                (*o).len = 0;
                (*o).hash = qstr::compute_hash(b"");
                let p = malloc::new::<u8>(1).expect("empty bytes data");
                *p = 0;
                (*o).data = p;
                let _ = EMPTY_BYTES.set(obj::from_ptr(o as *const ObjStr as *const ()));
            }
        }
    });
}

fn me(name: &str, value: Obj) -> MapElem {
    MapElem { key: obj::new_qstr(qstr::from_str(name)), value }
}

pub fn str_make_new(type_in: &ObjType, n_args: usize, n_kw: usize, args: &[Obj]) -> Obj {
    argcheck::check_num(n_args, n_kw, 0, 3, false);
    match n_args {
        0 => obj::new_qstr(qstr::QSTR_EMPTY),
        1 => {
            let mut v = Vstr { alloc: 0, len: 0, buf: core::ptr::null_mut(), fixed_buf: false };
            let mut print = Print { data: core::ptr::null_mut(), print_strn: None };
            vstr::init_print(&mut v, 16, &mut print);
            obj::print_helper(&print, args[0], PrintKind::Str);
            new_str_type_from_vstr(type_in, &mut v)
        }
        _ => {
            let (str_data, str_len) = if obj::is_exact_type(args[0], type_bytes()) {
                get_str_data_len(args[0])
            } else {
                let mut buf = BufferInfo::default();
                obj::get_buffer_raise(args[0], &mut buf, obj::BUFFER_READ);
                (unsafe { std::slice::from_raw_parts(buf.buf as *const u8, buf.len) }.to_vec(), buf.len)
            };
            if mpconfig::PY_BUILTINS_STR_UNICODE_CHECK {
                let enc = parse_encoding(str_get_qstr(args[1]));
                if !unicode::unicode_encoding_check(enc, &str_data[..str_len]) {
                    raise::raise(MpRaise::RuntimeError("UnicodeError"));
                }
            }
            let q = qstr::find_strn(&str_data[..str_len]);
            if q != qstr::QSTR_NULL {
                return obj::new_qstr(q);
            }
            if !obj::is_exact_type(args[0], type_bytes()) {
                return new_str_copy(type_in, Some(&str_data[..str_len]), str_len);
            }
            let mut hash = get_str_hash(args[0]);
            if hash == 0 {
                hash = qstr::compute_hash(&str_data[..str_len]);
            }
            return with_str_bytes(args[0], |data, len| {
                let o = new_str_copy(type_in, Some(unsafe { std::slice::from_raw_parts(data, len) }), len);
                unsafe {
                    let s = &mut *(obj::as_ptr(o) as *mut ObjStr);
                    s.data = data;
                    s.hash = hash;
                }
                o
            });
        }
    }
}

fn parse_encoding(q: Qstr) -> Encoding {
    let s = qstr::str_from_qstr(q).unwrap_or_default();
    if s == "utf-8" || s == "utf8" { Encoding::Utf8 }
    else if s == "ascii" { Encoding::Ascii }
    else { raise::raise(MpRaise::RuntimeError("LookupError")); }
}

pub fn bytes_make_new(_type_in: &ObjType, n_args: usize, n_kw: usize, args: &[Obj]) -> Obj {
    if mpconfig::CPYTHON_COMPAT && n_kw != 0 {
        raise::raise(MpRaise::TypeError("keyword args"));
    }
    if n_args == 0 { return const_empty_bytes(); }
    if obj::is_exact_type(args[0], type_bytes()) { return args[0]; }
    if obj::is_str(args[0]) {
        if n_args < 2 || n_args > 3 {
            raise::raise(MpRaise::TypeError("string argument without an encoding"));
        }
        let (data, len) = get_str_data_len(args[0]);
        let mut hash = get_str_hash(args[0]);
        if hash == 0 {
            hash = qstr::compute_hash(&data[..len]);
        }
        return with_str_bytes(args[0], |ptr, blen| {
            let o = new_str_copy(type_bytes(), Some(unsafe { std::slice::from_raw_parts(ptr, blen) }), blen);
            unsafe {
                let s = &mut *(obj::as_ptr(o) as *mut ObjStr);
                s.data = ptr;
                s.hash = hash;
            }
            o
        });
    }
    if n_args > 1 { raise::raise(MpRaise::TypeError("wrong number of arguments")); }
    if obj::is_small_int(args[0]) {
        let len = obj::small_int_value(args[0]);
        if len < 0 { raise::raise(MpRaise::ValueError("negative length")); }
        let mut v = Vstr { alloc: 0, len: 0, buf: core::ptr::null_mut(), fixed_buf: false };
        vstr::init_len(&mut v, len as usize);
        if !mpconfig::GC_CONSERVATIVE_CLEAR {
            unsafe { std::ptr::write_bytes(v.buf, 0, len as usize); }
        }
        return new_bytes_from_vstr(&mut v);
    }
    let mut buf = BufferInfo::default();
    if obj::get_buffer(args[0], &mut buf, obj::BUFFER_READ) {
        return new_bytes(unsafe { std::slice::from_raw_parts(buf.buf as *const u8, buf.len) });
    }
    let mut v = Vstr { alloc: 0, len: 0, buf: core::ptr::null_mut(), fixed_buf: false };
    if let Some(l) = obj::len_maybe(args[0]) {
        vstr::init(&mut v, obj::small_int_value(l) as usize);
    } else {
        vstr::init(&mut v, 16);
    }
    let iter = runtime::getiter(args[0], None);
    loop {
        let item = runtime::iternext(iter);
        if item == obj::OBJ_STOP_ITERATION { break; }
        let val = obj::get_int(item);
        if mpconfig::FULL_CHECKS && (val < 0 || val > 255) {
            raise::raise(MpRaise::ValueError("bytes value out of range"));
        }
        vstr::add_byte(&mut v, val as u8);
    }
    new_bytes_from_vstr(&mut v)
}

pub fn str_binary_op(op: BinaryOp, lhs: Obj, rhs: Obj) -> Obj {
    if op == BinaryOp::Modulo {
        if !mpconfig::PY_BUILTINS_STR_OP_MODULO { return obj::OBJ_NULL; }
        return str_modulo_format(lhs, rhs);
    }
    let lhs_type = obj::get_type(lhs);
    let (lhs_data, lhs_len) = get_str_data_len(lhs);
    if op == BinaryOp::Multiply {
        let mut n: obj::Int = 0;
        if !obj::get_int_maybe(rhs, &mut n) { return obj::OBJ_NULL; }
        if n <= 0 { return make_empty_str_of_type(lhs_type); }
        let mut v = Vstr { alloc: 0, len: 0, buf: core::ptr::null_mut(), fixed_buf: false };
        vstr::init_len(&mut v, lhs_len * n as usize);
        sequence::multiply(&lhs_data[..lhs_len], 1, lhs_len, n as usize, unsafe { std::slice::from_raw_parts_mut(v.buf, lhs_len * n as usize) });
        return new_str_type_from_vstr(lhs_type, &mut v);
    }
    let (rhs_data, rhs_len) = if obj::get_type(rhs) as *const ObjType == lhs_type as *const ObjType {
        get_str_data_len(rhs)
    } else if core::ptr::eq(lhs_type, type_bytes()) {
        let mut buf = BufferInfo::default();
        if !obj::get_buffer(rhs, &mut buf, obj::BUFFER_READ) { return obj::OBJ_NULL; }
        (unsafe { std::slice::from_raw_parts(buf.buf as *const u8, buf.len) }.to_vec(), buf.len)
    } else {
        if op == BinaryOp::Contains { bad_implicit_conversion(rhs); }
        return obj::OBJ_NULL;
    };
    match op {
        BinaryOp::Add | BinaryOp::InplaceAdd => {
            if lhs_len == 0 && obj::get_type(rhs) as *const ObjType == lhs_type as *const ObjType { return rhs; }
            if rhs_len == 0 { return lhs; }
            let mut v = Vstr { alloc: 0, len: 0, buf: core::ptr::null_mut(), fixed_buf: false };
            vstr::init_len(&mut v, lhs_len + rhs_len);
            unsafe {
                std::ptr::copy_nonoverlapping(lhs_data.as_ptr(), v.buf, lhs_len);
                std::ptr::copy_nonoverlapping(rhs_data.as_ptr(), v.buf.add(lhs_len), rhs_len);
            }
            new_str_type_from_vstr(lhs_type, &mut v)
        }
        BinaryOp::Contains => obj::new_bool(find_subbytes(&lhs_data[..lhs_len], &rhs_data[..rhs_len], 1).is_some()),
        BinaryOp::Equal | BinaryOp::Less | BinaryOp::LessEqual | BinaryOp::More | BinaryOp::MoreEqual =>
            obj::new_bool(sequence::cmp_bytes(op, &lhs_data[..lhs_len], &rhs_data[..rhs_len])),
        _ => obj::OBJ_NULL,
    }
}

pub fn bytes_subscr(self_in: Obj, index: Obj, value: Obj) -> Obj {
    let type_ = obj::get_type(self_in);
    let (data, len) = get_str_data_len(self_in);
    if value == OBJ_SENTINEL {
        if mpconfig::PY_BUILTINS_SLICE && obj::is_exact_type(index, crate::objslice::type_slice()) {
            let mut bounds = objslice::BoundSlice { start: 0, stop: 0, step: 1 };
            if !sequence::get_fast_slice_indexes(len, index, &mut bounds) {
                raise::raise(MpRaise::RuntimeError("only slices with step=1 supported"));
            }
            return new_str_of_type(type_, &data[bounds.start as usize..bounds.stop as usize]);
        }
        let idx = index_to_ptr(type_, &data, len, index, false);
        if mpconfig::PY_BUILTINS_STR_UNICODE || core::ptr::eq(type_, type_bytes()) {
            obj::new_small_int(data[idx] as obj::Int)
        } else {
            new_str_via_qstr(&data[idx..idx + 1])
        }
    } else { obj::OBJ_NULL }
}

pub fn get_buffer(self_in: Obj, bufinfo: &mut BufferInfo, flags: u32) -> obj::Int {
    if flags == obj::BUFFER_READ {
        with_str_bytes(self_in, |data, len| {
            bufinfo.buf = data as *mut _;
            bufinfo.len = len;
            bufinfo.typecode = b'B' as i32;
        });
        0
    } else { 1 }
}

#[repr(C)]
struct ObjStr8Iter { base: ObjBase, iternext: crate::obj::IterNextFn, str: Obj, cur: usize }

fn bytes_it_iternext(self_in: Obj) -> Obj {
    let self_ = unsafe { &mut *(obj::as_ptr(self_in) as *mut ObjStr8Iter) };
    let (data, len) = get_str_data_len(self_.str);
    if self_.cur < len {
        let v = data[self_.cur];
        self_.cur += 1;
        obj::new_small_int(v as obj::Int)
    } else { obj::OBJ_STOP_ITERATION }
}

fn new_bytes_iterator(str: Obj, iter_buf: *mut ObjIterBuf) -> Obj {
    let o = unsafe { &mut *(iter_buf as *mut ObjStr8Iter) };
    o.base.type_ = objpolyiter::type_polymorph_iter() as *const ObjType;
    o.iternext = bytes_it_iternext;
    o.str = str;
    o.cur = 0;
    obj::from_ptr(o as *const ObjStr8Iter as *const ())
}

// --- string methods -----------------------------------------------------------

fn str_join(self_in: Obj, arg: Obj) -> Obj {
    check_is_str_or_bytes(self_in);
    let self_type = obj::get_type(self_in);
    let (sep, sep_len) = get_str_data_len(self_in);
    let mut arg = arg;
    if !obj::is_exact_type(arg, objlist::type_list()) && !obj::is_exact_type(arg, objtuple::type_tuple()) {
        arg = objlist::list_make_new(objlist::type_list(), 1, 0, &[arg]);
    }
    let (seq_len, seq_items) = obj::get_array(arg);
    let mut required = 0usize;
    for (i, &item) in seq_items.iter().enumerate() {
        if obj::get_type(item) as *const ObjType != self_type as *const ObjType {
            raise::raise(MpRaise::TypeError("join expects consistent types"));
        }
        if i > 0 { required += sep_len; }
        required += get_str_data_len(item).1;
    }
    let mut v = Vstr { alloc: 0, len: 0, buf: core::ptr::null_mut(), fixed_buf: false };
    vstr::init_len(&mut v, required);
    let mut off = 0usize;
    for (i, &item) in seq_items.iter().enumerate() {
        if i > 0 {
            unsafe { std::ptr::copy_nonoverlapping(sep.as_ptr(), v.buf.add(off), sep_len); }
            off += sep_len;
        }
        let (s, l) = get_str_data_len(item);
        unsafe { std::ptr::copy_nonoverlapping(s.as_ptr(), v.buf.add(off), l); }
        off += l;
    }
    new_str_type_from_vstr(self_type, &mut v)
}

pub fn str_format(n_args: usize, args: &[Obj], mut kwargs: Option<&mut Map>) -> Obj {
    check_is_str_or_bytes(args[0]);
    let (pat, plen) = get_str_data_len(args[0]);
    let mut arg_i = 0i32;
    let mut empty_kwargs = Map::default();
    let kwargs_map = kwargs.as_deref_mut().unwrap_or(&mut empty_kwargs);
    let mut v = str_format_helper(&pat[..plen], &mut arg_i, n_args, args, kwargs_map);
    new_str_type_from_vstr(obj::get_type(args[0]), &mut v)
}

fn terse_str_format_value_error() -> ! {
    raise::raise(MpRaise::ValueError("bad format string"));
}

fn str_to_int(str: &[u8], mut pos: usize, top: usize, num: &mut i32) -> usize {
    if pos < top && str[pos].is_ascii_digit() {
        *num = 0;
        while pos < top && str[pos].is_ascii_digit() {
            *num = *num * 10 + (str[pos] - b'0') as i32;
            pos += 1;
        }
    }
    pos
}

fn is_alignment(ch: u8) -> bool {
    matches!(ch, b'<' | b'>' | b'=' | b'^')
}

fn is_format_type(ch: u8) -> bool {
    matches!(ch, b'b' | b'c' | b'd' | b'e' | b'E' | b'f' | b'F' | b'g' | b'G' | b'n' | b'o' | b's' | b'x' | b'X' | b'%')
}

fn arg_looks_integer(arg: Obj) -> bool {
    obj::is_bool(arg) || obj::is_int(arg)
}

fn arg_looks_numeric(arg: Obj) -> bool {
    arg_looks_integer(arg) || (mpconfig::PY_BUILTINS_FLOAT && objfloat::is_float(arg))
}

fn print_format_char(print: &Print, arg: Obj, flags: u32, fill: u8, width: i32) {
    let ch = obj::get_int(arg) as u8;
    mpprint::print_strn(print, &[ch], flags, fill, width);
}

fn str_format_helper(
    top: &[u8],
    arg_i: &mut i32,
    n_args: usize,
    args: &[Obj],
    kwargs: &mut Map,
) -> Vstr {
    let mut vstr = Vstr { alloc: 0, len: 0, buf: core::ptr::null_mut(), fixed_buf: false };
    let mut print = Print { data: core::ptr::null_mut(), print_strn: None };
    vstr::init_print(&mut vstr, 16, &mut print);

    let mut pos = 0usize;
    while pos < top.len() {
        let ch = top[pos];
        if ch == b'}' {
            pos += 1;
            if pos < top.len() && top[pos] == b'}' {
                vstr::add_byte(&mut vstr, b'}');
                pos += 1;
                continue;
            }
            terse_str_format_value_error();
        }
        if ch != b'{' {
            vstr::add_byte(&mut vstr, ch);
            pos += 1;
            continue;
        }

        pos += 1;
        if pos < top.len() && top[pos] == b'{' {
            vstr::add_byte(&mut vstr, b'{');
            pos += 1;
            continue;
        }

        let field_start = pos;
        while pos < top.len() && top[pos] != b'}' && top[pos] != b'!' && top[pos] != b':' {
            pos += 1;
        }
        let field_name = if pos > field_start {
            Some((field_start, pos))
        } else {
            None
        };

        let mut conversion = 0u8;
        if pos < top.len() && top[pos] == b'!' {
            pos += 1;
            if pos < top.len() && (top[pos] == b'r' || top[pos] == b's') {
                conversion = top[pos];
                pos += 1;
            } else {
                terse_str_format_value_error();
            }
        }

        let mut format_spec: Option<(usize, usize)> = None;
        if pos < top.len() && top[pos] == b':' {
            pos += 1;
            if pos < top.len() && top[pos] != b'}' {
                let spec_start = pos;
                let mut nest = 1i32;
                while pos < top.len() {
                    if top[pos] == b'{' {
                        nest += 1;
                    } else if top[pos] == b'}' {
                        nest -= 1;
                        if nest == 0 {
                            break;
                        }
                    }
                    pos += 1;
                }
                format_spec = Some((spec_start, pos));
            }
        }

        if pos >= top.len() || top[pos] != b'}' {
            terse_str_format_value_error();
        }
        pos += 1;

        let mut arg = obj::CONST_NONE;
        if let Some((fn_start, fn_end)) = field_name {
            if top[fn_start].is_ascii_digit() {
                if *arg_i > 0 {
                    terse_str_format_value_error();
                }
                let mut index = 0i32;
                let _ = str_to_int(top, fn_start, fn_end, &mut index);
                if (index as usize) >= n_args.saturating_sub(1) {
                    raise::raise_obj(objexcept::new_exception_args(
                        objexcept::type_index_error(),
                        1,
                        &[new_str(b"tuple index out of range")],
                    ));
                }
                arg = args[(index as usize) + 1];
                *arg_i = -1;
            } else {
                let mut lookup = fn_start;
                while lookup < fn_end && top[lookup] != b'.' && top[lookup] != b'[' {
                    lookup += 1;
                }
                let field_q = new_str_via_qstr(&top[fn_start..lookup]);
                let Some(key_elem) = map::lookup(kwargs, field_q, LookupKind::Lookup) else {
                    raise::raise_obj(objexcept::new_exception_args(
                        objexcept::type_key_error(),
                        1,
                        &[field_q],
                    ));
                };
                arg = key_elem.value;
            }
            if top[fn_start..fn_end].contains(&b'.') || top[fn_start..fn_end].contains(&b'[') {
                raise::raise_obj(objexcept::new_exception_args(
                    objexcept::type_not_implemented_error(),
                    1,
                    &[new_str(b"attributes not supported")],
                ));
            }
        } else {
            if *arg_i < 0 {
                terse_str_format_value_error();
            }
            if (*arg_i as usize) >= n_args.saturating_sub(1) {
                raise::raise_obj(objexcept::new_exception_args(
                    objexcept::type_index_error(),
                    1,
                    &[new_str(b"tuple index out of range")],
                ));
            }
            arg = args[(*arg_i as usize) + 1];
            *arg_i += 1;
        }

        if format_spec.is_none() && conversion == 0 {
            conversion = b's';
        }
        if conversion != 0 {
            let print_kind = if conversion == b's' { PrintKind::Str } else { PrintKind::Repr };
            let mut arg_vstr = Vstr { alloc: 0, len: 0, buf: core::ptr::null_mut(), fixed_buf: false };
            let mut arg_print = Print { data: core::ptr::null_mut(), print_strn: None };
            vstr::init_print(&mut arg_vstr, 16, &mut arg_print);
            obj::print_helper(&arg_print, arg, print_kind);
            arg = new_str_type_from_vstr(type_str(), &mut arg_vstr);
        }

        let mut fill = 0u8;
        let mut align = 0u8;
        let mut width = -1i32;
        let mut precision = -1i32;
        let mut ftype = 0u8;
        let mut flags = 0u32;

        if let Some((spec_start, spec_end)) = format_spec {
            cstack::check();
            let spec_slice = &top[spec_start..spec_end];
            let mut spec_vstr = str_format_helper(spec_slice, arg_i, n_args, args, kwargs);
            let spec_bytes = unsafe { std::slice::from_raw_parts(spec_vstr.buf, spec_vstr.len) };
            let mut s = 0usize;
            let stop = spec_bytes.len();
            if s < stop && is_alignment(spec_bytes[s]) {
                align = spec_bytes[s];
                s += 1;
            } else if s + 1 < stop && is_alignment(spec_bytes[s + 1]) {
                fill = spec_bytes[s];
                s += 1;
                align = spec_bytes[s];
                s += 1;
            }
            if s < stop && matches!(spec_bytes[s], b'+' | b'-' | b' ') {
                if spec_bytes[s] == b'+' {
                    flags |= PF_FLAG_SHOW_SIGN;
                } else if spec_bytes[s] == b' ' {
                    flags |= PF_FLAG_SPACE_SIGN;
                }
                s += 1;
            }
            if s < stop && spec_bytes[s] == b'#' {
                flags |= PF_FLAG_SHOW_PREFIX;
                s += 1;
            }
            if s < stop && spec_bytes[s] == b'0' {
                if align == 0 && arg_looks_numeric(arg) {
                    align = b'=';
                }
                if fill == 0 {
                    fill = b'0';
                }
            }
            s = str_to_int(spec_bytes, s, stop, &mut width);
            if s < stop && (spec_bytes[s] == b',' || spec_bytes[s] == b'_') {
                flags |= (spec_bytes[s] as u32) << PF_FLAG_SEP_POS;
                s += 1;
            }
            if s < stop && spec_bytes[s] == b'.' {
                s += 1;
                s = str_to_int(spec_bytes, s, stop, &mut precision);
            }
            if s < stop && is_format_type(spec_bytes[s]) {
                ftype = spec_bytes[s];
                s += 1;
            }
            if s < stop {
                vstr::clear(&mut spec_vstr);
                terse_str_format_value_error();
            }
            vstr::clear(&mut spec_vstr);
        }

        if align == 0 {
            align = if arg_looks_numeric(arg) { b'>' } else { b'<' };
        }
        if fill == 0 {
            fill = b' ';
        }

        if flags & (PF_FLAG_SHOW_SIGN | PF_FLAG_SPACE_SIGN) != 0 && (ftype == b's' || ftype == b'c') {
            terse_str_format_value_error();
        }

        match align {
            b'<' => flags |= PF_FLAG_LEFT_ADJUST,
            b'=' => flags |= PF_FLAG_PAD_AFTER_SIGN,
            b'^' => flags |= PF_FLAG_CENTER_ADJUST,
            _ => {}
        }

        if arg_looks_integer(arg) {
            match ftype {
                b'b' => {
                    mpprint::print_mp_int(&print, arg, 2, b'a', flags, fill, width, 0);
                    continue;
                }
                b'c' => {
                    print_format_char(&print, arg, flags, fill, width);
                    continue;
                }
                0 | b'n' | b'd' => {
                    mpprint::print_mp_int(&print, arg, 10, b'a', flags, fill, width, 0);
                    continue;
                }
                b'o' => {
                    if flags & PF_FLAG_SHOW_PREFIX != 0 {
                        flags |= PF_FLAG_SHOW_OCTAL_LETTER;
                    }
                    mpprint::print_mp_int(&print, arg, 8, b'a', flags, fill, width, 0);
                    continue;
                }
                b'X' | b'x' => {
                    mpprint::print_mp_int(&print, arg, 16, ftype - (b'X' - b'A'), flags, fill, width, 0);
                    continue;
                }
                b'e' | b'E' | b'f' | b'F' | b'g' | b'G' | b'%' => {}
                _ => terse_str_format_value_error(),
            }
        }

        if arg_looks_numeric(arg) {
            let mut ftype = ftype;
            if ftype == 0 {
                ftype = b'g';
            }
            if ftype == b'n' {
                ftype = b'g';
            }
            if mpconfig::PY_BUILTINS_FLOAT {
                match ftype {
                    b'e' | b'E' | b'f' | b'F' | b'g' | b'G' => {
                        mpprint::print_float(
                            &print,
                            objfloat::float_get(arg),
                            ftype,
                            flags,
                            fill,
                            width,
                            precision,
                        );
                    }
                    b'%' => {
                        flags |= PF_FLAG_ADD_PERCENT;
                        mpprint::print_float(
                            &print,
                            objfloat::float_get(arg) * 100.0,
                            b'f',
                            flags,
                            fill,
                            width,
                            precision,
                        );
                    }
                    _ => terse_str_format_value_error(),
                }
            }
        } else {
            if align == b'=' {
                terse_str_format_value_error();
            }
            match ftype {
                0 | b's' => {
                    let (s, slen) = get_str_data_len(arg);
                    let mut slen = slen;
                    if precision >= 0 && (precision as usize) < slen {
                        slen = precision as usize;
                    }
                    mpprint::print_strn(&print, &s[..slen], flags, fill, width);
                }
                _ => terse_str_format_value_error(),
            }
        }
    }

    vstr
}

fn str_format_kw(n_args: usize, args: &[Obj], kwargs: &mut Map) -> Obj {
    str_format(n_args, args, Some(kwargs))
}

pub fn str_split(n_args: usize, args: &[Obj]) -> Obj {
    let self_type = obj::get_type(args[0]);
    let mut splits: i32 = -1;
    let mut sep = obj::CONST_NONE;
    if n_args > 1 { sep = args[1]; if n_args > 2 { splits = obj::get_int(args[2]) as i32; } }
    let res = objlist::new_list(0, None);
    let (s, len) = get_str_data_len(args[0]);
    let data = s.clone();
    let top = len;
    let mut pos = 0usize;
    if sep == obj::CONST_NONE {
        while pos < top && unichar_isspace(data[pos] as u32) { pos += 1; }
        while pos < top && splits != 0 {
            let start = pos;
            while pos < top && !unichar_isspace(data[pos] as u32) { pos += 1; }
            objlist::list_append(res, new_str_of_type(self_type, &data[start..pos]));
            if pos >= top { break; }
            while pos < top && unichar_isspace(data[pos] as u32) { pos += 1; }
            if splits > 0 { splits -= 1; }
        }
        if pos < top { objlist::list_append(res, new_str_of_type(self_type, &data[pos..top])); }
    } else {
        str_check_arg_type(self_type, sep);
        let (sep_data, sep_len) = get_str_data_len(sep);
        if sep_len == 0 { raise::raise(MpRaise::ValueError("empty separator")); }
        loop {
            let start = pos;
            loop {
                if splits == 0 || pos + sep_len > top { pos = top; break; }
                if data[pos..pos + sep_len] == sep_data[..sep_len] { break; }
                pos += 1;
            }
            objlist::list_append(res, new_str_of_type(self_type, &data[start..pos]));
            if pos >= top { break; }
            pos += sep_len;
            if splits > 0 { splits -= 1; }
        }
    }
    res
}

fn str_split_method(n: usize, args: &[Obj]) -> Obj { str_split(n, args) }

fn str_rsplit(n_args: usize, args: &[Obj]) -> Obj {
    if n_args < 3 {
        return str_split(n_args, args);
    }
    let self_type = obj::get_type(args[0]);
    let sep = args[1];
    let (s, len) = get_str_data_len(args[0]);
    let data = s.clone();
    let mut splits = obj::get_int(args[2]) as i32;
    if splits < 0 {
        return str_split(n_args, args);
    }
    let org_splits = splits;
    let mut res_items: Vec<Obj> = vec![obj::OBJ_NULL; (splits + 1) as usize];
    let mut idx = splits;
    if sep == obj::CONST_NONE {
        raise::raise(MpRaise::RuntimeError("rsplit(None,n)"));
    }
    str_check_arg_type(self_type, sep);
    let (sep_data, sep_len) = get_str_data_len(sep);
    if sep_len == 0 {
        raise::raise(MpRaise::ValueError("empty separator"));
    }
    let mut beg = 0usize;
    let mut last = len;
    loop {
        let mut pos = if last >= sep_len { last - sep_len } else { 0 };
        loop {
            if splits == 0 || pos < beg {
                break;
            }
            if data[pos..pos + sep_len] == sep_data[..sep_len] {
                break;
            }
            if pos == 0 {
                break;
            }
            pos -= 1;
        }
        if pos < beg || splits == 0 {
            res_items[idx as usize] = new_str_of_type(self_type, &data[beg..last]);
            break;
        }
        res_items[idx as usize] = new_str_of_type(self_type, &data[pos + sep_len..last]);
        last = pos;
        splits -= 1;
        idx -= 1;
    }
    if idx != 0 {
        let used = (org_splits + 1 - idx) as usize;
        res_items = res_items[idx as usize..idx as usize + used].to_vec();
    }
    objlist::new_list(res_items.len(), Some(&res_items))
}

fn str_rsplit_method(n: usize, args: &[Obj]) -> Obj { str_rsplit(n, args) }

fn str_finder(n_args: usize, args: &[Obj], direction: i32, is_index: bool) -> Obj {
    let self_type = obj::get_type(args[0]);
    check_is_str_or_bytes(args[0]);
    let (haystack, haystack_len) = get_str_data_len(args[0]);
    let (needle, _needle_len) = if core::ptr::eq(self_type, type_bytes()) {
        let mut iv = 0isize;
        if obj::get_int_maybe(args[1], &mut iv) {
            (vec![iv as u8], 1)
        } else {
            str_check_arg_type(self_type, args[1]);
            get_str_data_len(args[1])
        }
    } else {
        str_check_arg_type(self_type, args[1]);
        get_str_data_len(args[1])
    };
    let mut start = 0usize;
    let mut end = haystack_len;
    if n_args >= 3 && args[2] != obj::CONST_NONE { start = index_to_ptr(self_type, &haystack, haystack_len, args[2], true); }
    if n_args >= 4 && args[3] != obj::CONST_NONE { end = index_to_ptr(self_type, &haystack, haystack_len, args[3], true); }
    if end < start {
        if is_index { raise::raise(MpRaise::ValueError("substring not found")); }
        return obj::new_small_int(-1);
    }
    if let Some(p) = find_subbytes(&haystack[start..end], &needle, direction) {
        let abs = start + p;
        if mpconfig::PY_BUILTINS_STR_UNICODE && core::ptr::eq(self_type, type_str()) {
            return obj::new_small_int(utf8_ptr_to_index(&haystack, abs) as obj::Int);
        }
        obj::new_small_int(abs as obj::Int)
    } else if is_index {
        raise::raise(MpRaise::ValueError("substring not found"));
    } else {
        obj::new_small_int(-1)
    }
}

fn str_find(n: usize, a: &[Obj]) -> Obj { str_finder(n, a, 1, false) }
fn str_rfind(n: usize, a: &[Obj]) -> Obj { str_finder(n, a, -1, false) }
fn str_index(n: usize, a: &[Obj]) -> Obj { str_finder(n, a, 1, true) }
fn str_rindex(n: usize, a: &[Obj]) -> Obj { str_finder(n, a, -1, true) }

fn str_startendswith(n_args: usize, args: &[Obj], ends: bool) -> Obj {
    let (str_data, str_len) = get_substring_data(args[0], n_args - 2, &args[2..n_args]);
    let mut prefixes = vec![args[1]];
    let mut n = 1usize;
    if obj::is_exact_type(args[1], objtuple::type_tuple()) {
        let (l, items) = objtuple::tuple_get(args[1]);
        prefixes = items;
        n = l;
    }
    for i in 0..n {
        let (pref, plen) = get_str_data_len(prefixes[i]);
        let s = if ends { str_len.saturating_sub(plen) } else { 0 };
        if plen <= str_len && str_data[s..s + plen] == pref[..plen] { return obj::CONST_TRUE; }
    }
    obj::CONST_FALSE
}

fn str_startswith(n: usize, a: &[Obj]) -> Obj { str_startendswith(n, a, false) }
fn str_endswith(n: usize, a: &[Obj]) -> Obj { str_startendswith(n, a, true) }

enum StripKind { L, R, Both }

fn str_uni_strip(kind: StripKind, n_args: usize, args: &[Obj]) -> Obj {
    check_is_str_or_bytes(args[0]);
    let self_type = obj::get_type(args[0]);
    let whitespace = b" \t\n\r\x0b\x0c";
    let (chars, clen) = if n_args == 1 {
        (whitespace.to_vec(), whitespace.len())
    } else {
        str_check_arg_type(self_type, args[1]);
        get_str_data_len(args[1])
    };
    let (orig, orig_len) = get_str_data_len(args[0]);
    let mut first = None::<usize>;
    let mut last = 0usize;
    let (mut i, delta, count) = match kind {
        StripKind::L => (0usize, 1isize, orig_len),
        StripKind::R => (orig_len.saturating_sub(1), -1, orig_len),
        StripKind::Both => (0, 1, orig_len),
    };
    let mut len_left = count;
    while len_left > 0 {
        let b = orig[i];
        if find_subbytes(&chars[..clen], &[b], 1).is_none() {
            if first.is_none() { first = Some(i); last = if matches!(kind, StripKind::L) { orig_len - 1 } else { i }; }
            else if matches!(kind, StripKind::Both) { last = i; }
            if matches!(kind, StripKind::R) { break; }
        }
        if delta > 0 { i += 1; } else { i = i.wrapping_sub(1); }
        len_left -= 1;
    }
    let Some(first) = first else { return make_empty_str_of_type(self_type); };
    let stripped_len = last - first + 1;
    if stripped_len == orig_len { return args[0]; }
    new_str_of_type(self_type, &orig[first..first + stripped_len])
}

fn str_strip(n: usize, a: &[Obj]) -> Obj { str_uni_strip(StripKind::Both, n, a) }
fn str_lstrip(n: usize, a: &[Obj]) -> Obj { str_uni_strip(StripKind::L, n, a) }
fn str_rstrip(n: usize, a: &[Obj]) -> Obj { str_uni_strip(StripKind::R, n, a) }

fn str_replace(n: usize, args: &[Obj]) -> Obj {
    check_is_str_or_bytes(args[0]);
    let self_type = obj::get_type(args[0]);
    str_check_arg_type(self_type, args[1]);
    str_check_arg_type(self_type, args[2]);
    let (str_data, str_len) = get_str_data_len(args[0]);
    let (old, old_len) = get_str_data_len(args[1]);
    let (new, new_len) = get_str_data_len(args[2]);
    if old_len > str_len { return args[0]; }
    let mut out = Vec::new();
    let mut pos = 0usize;
    while pos <= str_len.saturating_sub(old_len) {
        if let Some(p) = find_subbytes(&str_data[pos..], &old, 1) {
            out.extend_from_slice(&str_data[pos..pos + p]);
            out.extend_from_slice(&new);
            pos += p + old_len;
        } else { break; }
    }
    out.extend_from_slice(&str_data[pos..str_len]);
    if out.len() == str_len && out[..] == str_data[..str_len] { return args[0]; }
    new_str_of_type(self_type, &out)
}

fn str_caseconv(op: fn(u32) -> u32, self_in: Obj) -> Obj {
    let (data, len) = get_str_data_len(self_in);
    let mut out = data[..len].to_vec();
    for b in &mut out { *b = op(*b as u32) as u8; }
    new_str_type_from_vstr(obj::get_type(self_in), &mut { let mut v = Vstr { alloc: 0, len: 0, buf: core::ptr::null_mut(), fixed_buf: false }; vstr::init_len(&mut v, len); unsafe { std::ptr::copy_nonoverlapping(out.as_ptr(), v.buf, len); }; v })
}

fn str_lower(self_in: Obj) -> Obj { str_caseconv(unichar_tolower, self_in) }
fn str_upper(self_in: Obj) -> Obj { str_caseconv(unichar_toupper, self_in) }

fn str_uni_istype(f: fn(u32) -> bool, self_in: Obj) -> Obj {
    let (data, len) = get_str_data_len(self_in);
    if len == 0 { return obj::CONST_FALSE; }
    for &b in &data[..len] {
        if !f(b as u32) { return obj::CONST_FALSE; }
    }
    obj::CONST_TRUE
}

fn str_isspace(o: Obj) -> Obj { str_uni_istype(unichar_isspace, o) }
fn str_isalpha(o: Obj) -> Obj { str_uni_istype(unichar_isalpha, o) }
fn str_isdigit(o: Obj) -> Obj { str_uni_istype(unichar_isdigit, o) }
fn str_isupper(o: Obj) -> Obj { str_uni_istype(unichar_isupper, o) }
fn str_islower(o: Obj) -> Obj { str_uni_istype(unichar_islower, o) }

fn str_count(n: usize, args: &[Obj]) -> Obj {
    check_is_str_or_bytes(args[0]);
    let self_type = obj::get_type(args[0]);
    str_check_arg_type(self_type, args[1]);
    let (haystack, haystack_len) = get_str_data_len(args[0]);
    let (needle, needle_len) = get_str_data_len(args[1]);
    let mut start = 0usize;
    let mut end = haystack_len;
    if n >= 3 && args[2] != obj::CONST_NONE { start = index_to_ptr(self_type, &haystack, haystack_len, args[2], true); }
    if n >= 4 && args[3] != obj::CONST_NONE { end = index_to_ptr(self_type, &haystack, haystack_len, args[3], true); }
    if needle_len == 0 { return obj::new_small_int(utf8_charlen(&haystack[start..end], end - start) as obj::Int + 1); }
    let mut count = 0i32;
    let mut p = start;
    while p + needle_len <= end {
        if haystack[p..p + needle_len] == needle[..needle_len] { count += 1; p += needle_len; }
        else { p = if core::ptr::eq(self_type, type_str()) { p + utf8_next_char(&haystack[p..]).len() } else { p + 1 }; }
    }
    obj::new_small_int(count as obj::Int)
}

fn str_partitioner(self_in: Obj, arg: Obj, direction: i32) -> Obj {
    check_is_str_or_bytes(self_in);
    let self_type = obj::get_type(self_in);
    str_check_arg_type(self_type, arg);
    let (str_data, str_len) = get_str_data_len(self_in);
    let (sep, sep_len) = get_str_data_len(arg);
    if sep_len == 0 { raise::raise(MpRaise::ValueError("empty separator")); }
    let mut result = [make_empty_str_of_type(self_type), make_empty_str_of_type(self_type), make_empty_str_of_type(self_type)];
    if direction > 0 { result[0] = self_in; } else { result[2] = self_in; }
    if let Some(p) = find_subbytes(&str_data, &sep, direction) {
        result[0] = new_str_of_type(self_type, &str_data[..p]);
        result[1] = arg;
        result[2] = new_str_of_type(self_type, &str_data[p + sep_len..str_len]);
    }
    objtuple::new_tuple(3, Some(&result))
}

fn str_partition(self_in: Obj, arg: Obj) -> Obj { str_partitioner(self_in, arg, 1) }
fn str_rpartition(self_in: Obj, arg: Obj) -> Obj { str_partitioner(self_in, arg, -1) }

fn str_center(self_in: Obj, width_in: Obj) -> Obj {
    let (data, str_len) = get_str_data_len(self_in);
    let width = obj::get_int(width_in) as usize;
    let char_len = if mpconfig::PY_BUILTINS_STR_UNICODE { utf8_charlen(&data, str_len) } else { str_len };
    if char_len >= width { return self_in; }
    let padding = width - char_len;
    let total = padding + str_len;
    let mut v = Vstr { alloc: 0, len: 0, buf: core::ptr::null_mut(), fixed_buf: false };
    vstr::init_len(&mut v, total);
    unsafe { std::ptr::write_bytes(v.buf, b' ', total); }
    let left = padding / 2;
    unsafe { std::ptr::copy_nonoverlapping(data.as_ptr(), v.buf.add(left), str_len); }
    new_str_type_from_vstr(obj::get_type(self_in), &mut v)
}

fn str_splitlines(n: usize, args: &[Obj]) -> Obj {
    let self_type = obj::get_type(args[0]);
    let keepends = n > 1 && args[1] == obj::CONST_TRUE;
    let (data, len) = get_str_data_len(args[0]);
    let res = objlist::new_list(0, None);
    let mut pos = 0usize;
    while pos < len {
        let start = pos;
        let mut m = 0usize;
        if data[pos] == b'\n' {
            m = 1;
        } else if data[pos] == b'\r' {
            m = if pos + 1 < len && data[pos + 1] == b'\n' { 2 } else { 1 };
        }
        if m == 0 {
            pos += 1;
            continue;
        }
        let end = if keepends { pos + m } else { pos };
        objlist::list_append(res, new_str_of_type(self_type, &data[start..end]));
        pos += m;
    }
    res
}

pub fn str_intern(o: Obj) -> Obj {
    let (data, len) = get_str_data_len(o);
    new_str_via_qstr(&data[..len])
}

pub fn str_intern_checked(o: Obj) -> Obj {
    let (data, len) = get_str_data_len(o);
    new_str_via_qstr(&data[..len])
}

fn str_modulo_format(pattern: Obj, rhs: Obj) -> Obj {
    check_is_str_or_bytes(pattern);
    let (pat, plen) = get_str_data_len(pattern);
    let is_bytes = obj::is_exact_type(pattern, type_bytes());
    let mut args_storage: Vec<Obj> = Vec::new();
    let mut args: &[Obj] = &[rhs];
    let mut n_args = 1usize;
    let mut dict = obj::OBJ_NULL;
    if objtuple::is_tuple_compatible(rhs) {
        let (n, items) = objtuple::tuple_get(rhs);
        args_storage = items;
        args = &args_storage;
        n_args = n;
    } else if obj::is_dict_or_ordereddict(rhs) {
        dict = rhs;
        n_args = 1;
    }
    let mut v = Vstr { alloc: 0, len: 0, buf: core::ptr::null_mut(), fixed_buf: false };
    let mut print = Print { data: core::ptr::null_mut(), print_strn: None };
    vstr::init_print(&mut v, 16, &mut print);
    let mut arg_i = 0usize;
    let mut i = 0usize;
    while i < plen {
        if pat[i] != b'%' { vstr::add_byte(&mut v, pat[i]); i += 1; continue; }
        i += 1;
        if i >= plen { raise::raise(MpRaise::ValueError("incomplete format")); }
        if pat[i] == b'%' { vstr::add_byte(&mut v, b'%'); i += 1; continue; }
        if arg_i >= n_args { raise::raise(MpRaise::TypeError("format string needs more arguments")); }
        let arg = if dict != obj::OBJ_NULL { objdict::dict_get(dict, args[arg_i]) } else { args[arg_i] };
        arg_i += 1;
        match pat[i] {
            b's' | b'r' => {
                let mut pv = Vstr { alloc: 0, len: 0, buf: core::ptr::null_mut(), fixed_buf: false };
                let mut pr = Print { data: core::ptr::null_mut(), print_strn: None };
                vstr::init_print(&mut pv, 16, &mut pr);
                let kind = if pat[i] == b'r' { PrintKind::Repr } else { PrintKind::Str };
                obj::print_helper(&pr, arg, kind);
                vstr::add_strn(&mut v, unsafe { std::slice::from_raw_parts(pv.buf, pv.len) });
                vstr::clear(&mut pv);
            }
            b'd' | b'i' | b'u' => {
                mpprint::print_mp_int(&print, arg, 10, b'a', 0, b' ', -1, -1);
            }
            b'x' | b'X' => {
                mpprint::print_mp_int(&print, arg, 16, pat[i] - (b'X' - b'A'), 0, b' ', -1, -1);
            }
            _ => raise::raise(MpRaise::ValueError("unsupported format character")),
        }
        i += 1;
    }
    let t = if is_bytes { type_bytes() } else { type_str() };
    new_str_type_from_vstr(t, &mut v)
}

fn bytes_hex(n: usize, args: &[Obj], type_: &ObjType) -> Obj {
    let mut buf = BufferInfo::default();
    obj::get_buffer_raise(args[0], &mut buf, obj::BUFFER_READ);
    if buf.len == 0 { return make_empty_str_of_type(type_); }
    let mut out_len = buf.len * 2;
    if n > 1 { out_len += buf.len - 1; }
    let mut v = Vstr { alloc: 0, len: 0, buf: core::ptr::null_mut(), fixed_buf: false };
    vstr::init_len(&mut v, out_len);
    let inb = unsafe { std::slice::from_raw_parts(buf.buf as *const u8, buf.len) };
    let mut o = 0usize;
    for (idx, &byte) in inb.iter().enumerate() {
        for shift in [4u8, 0] {
            let mut d = (byte >> shift) & 0xf;
            if d > 9 { d += b'a' - b'9' - 1; }
            unsafe { *v.buf.add(o) = d + b'0'; }
            o += 1;
        }
        if n > 1 && idx + 1 != inb.len() {
            let (sep, _) = get_str_data_len(args[1]);
            unsafe { *v.buf.add(o) = sep[0]; }
            o += 1;
        }
    }
    new_str_type_from_vstr(type_, &mut v)
}

fn bytes_hex_method(n: usize, args: &[Obj]) -> Obj { bytes_hex(n, args, type_bytes()) }

fn bytes_fromhex_method(_n: usize, args: &[Obj]) -> Obj {
    let mut buf = BufferInfo::default();
    obj::get_buffer_raise(args[0], &mut buf, obj::BUFFER_READ);
    let inb = unsafe { std::slice::from_raw_parts(buf.buf as *const u8, buf.len) };
    let mut out: Vec<u8> = Vec::with_capacity(inb.len() / 2);
    let mut i = 0usize;
    while i < inb.len() {
        while i < inb.len() && unichar_isspace(inb[i] as u32) {
            i += 1;
        }
        if i >= inb.len() {
            break;
        }
        if i + 1 >= inb.len() || !unichar_isxdigit(inb[i] as u32) || !unichar_isxdigit(inb[i + 1] as u32) {
            raise::raise(MpRaise::ValueError("non-hex digit"));
        }
        out.push(
            ((unichar_xdigit_value(inb[i] as u32) << 4) | unichar_xdigit_value(inb[i + 1] as u32)) as u8,
        );
        i += 2;
    }
    new_bytes(&out)
}

fn bytes_decode(n: usize, args: &[Obj]) -> Obj {
    let mut a = args.to_vec();
    if n == 1 {
        a.push(obj::new_qstr(qstr::from_str("utf-8")));
    }
    str_make_new(type_str(), a.len(), 0, &a)
}

fn str_encode(n: usize, args: &[Obj]) -> Obj {
    let mut a = args.to_vec();
    if n == 1 {
        a.push(obj::new_qstr(qstr::from_str("utf-8")));
    }
    bytes_make_new(type_bytes(), a.len(), 0, &a)
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
    fn str_equal_qstr() {
        setup();
        let a = obj::new_qstr(qstr::from_str("hi"));
        let b = obj::new_qstr(qstr::from_str("hi"));
        assert!(str_equal(a, b));
    }

    #[test]
    fn bytes_add() {
        setup();
        let a = new_bytes(b"ab");
        let b = new_bytes(b"cd");
        let r = str_binary_op(BinaryOp::Add, a, b);
        let (d, l) = get_str_data_len(r);
        assert_eq!(&d[..l], b"abcd");
    }

    #[test]
    fn split_whitespace() {
        setup();
        let s = new_str(b"a b c");
        let parts = str_split(1, &[s]);
        let (_, items) = objlist::list_get(parts);
        assert_eq!(items.len(), 3);
    }
}
