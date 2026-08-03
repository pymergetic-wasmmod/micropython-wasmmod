//! rewrite of extmod/modjson.c
// symmetry: done

use py_rs::argcheck::{self, Arg, ArgFlag, ArgVal};
use py_rs::bc::ModuleContext;
use py_rs::malloc;
use py_rs::map::{self, LookupKind, Map, MapElem};
use py_rs::misc;
use py_rs::mpconfig;
use py_rs::mpprint::{self, Print, PrintExt, PrintKind};
use py_rs::obj::{self, Obj, ObjBase, ObjType, TYPE_FLAG_BUILTIN_FUN};
use py_rs::objdict;
use py_rs::objlist;
use py_rs::objmodule;
use py_rs::objstr;
use py_rs::objstringio;
use py_rs::parsenum;
use py_rs::qstr;
use py_rs::raise::{self, MpRaise};
use py_rs::stream::{self, StreamIoFn, STREAM_OP_READ, STREAM_OP_WRITE};
use py_rs::vstr::{self, Vstr};

type BuiltinFn1 = fn(Obj) -> Obj;
type BuiltinFn2 = fn(Obj, Obj) -> Obj;
type BuiltinFnKw = fn(usize, &[Obj], &Map) -> Obj;

#[repr(C)]
struct ObjFunBuiltin1 {
    base: ObjBase,
    fun: BuiltinFn1,
}

#[repr(C)]
struct ObjFunBuiltin2 {
    base: ObjBase,
    fun: BuiltinFn2,
}

#[repr(C)]
struct ObjFunBuiltinKw {
    base: ObjBase,
    min_args: u8,
    fun: BuiltinFnKw,
}

static mut F1: [*const (); 1] = [call1 as *const ()];
static mut F2: [*const (); 1] = [call2 as *const ()];
static mut FK: [*const (); 1] = [call_kw as *const ()];

static T1: ObjType = ObjType {
    base: ObjBase {
        type_: core::ptr::null(),
    },
    flags: TYPE_FLAG_BUILTIN_FUN,
    name: 0,
    slot_index_make_new: 0,
    slot_index_print: 0,
    slot_index_call: 1,
    slot_index_unary_op: 0,
    slot_index_binary_op: 0,
    slot_index_attr: 0,
    slot_index_subscr: 0,
    slot_index_iter: 0,
    slot_index_buffer: 0,
    slot_index_protocol: 0,
    slot_index_parent: 0,
    slot_index_locals_dict: 0,
    slots: unsafe { F1.as_ptr() },
};

static T2: ObjType = ObjType {
    base: ObjBase {
        type_: core::ptr::null(),
    },
    flags: TYPE_FLAG_BUILTIN_FUN,
    name: 0,
    slot_index_make_new: 0,
    slot_index_print: 0,
    slot_index_call: 1,
    slot_index_unary_op: 0,
    slot_index_binary_op: 0,
    slot_index_attr: 0,
    slot_index_subscr: 0,
    slot_index_iter: 0,
    slot_index_buffer: 0,
    slot_index_protocol: 0,
    slot_index_parent: 0,
    slot_index_locals_dict: 0,
    slots: unsafe { F2.as_ptr() },
};

static TK: ObjType = ObjType {
    base: ObjBase {
        type_: core::ptr::null(),
    },
    flags: TYPE_FLAG_BUILTIN_FUN,
    name: 0,
    slot_index_make_new: 0,
    slot_index_print: 0,
    slot_index_call: 1,
    slot_index_unary_op: 0,
    slot_index_binary_op: 0,
    slot_index_attr: 0,
    slot_index_subscr: 0,
    slot_index_iter: 0,
    slot_index_buffer: 0,
    slot_index_protocol: 0,
    slot_index_parent: 0,
    slot_index_locals_dict: 0,
    slots: unsafe { FK.as_ptr() },
};

fn call1(s: Obj, n: usize, k: usize, a: &[Obj]) -> Obj {
    argcheck::check_num(n, k, 1, 1, false);
    unsafe { ((*(obj::as_ptr(s) as *const ObjFunBuiltin1)).fun)(a[0]) }
}

fn call2(s: Obj, n: usize, k: usize, a: &[Obj]) -> Obj {
    argcheck::check_num(n, k, 2, 2, false);
    unsafe { ((*(obj::as_ptr(s) as *const ObjFunBuiltin2)).fun)(a[0], a[1]) }
}

fn call_kw(s: Obj, n: usize, k: usize, a: &[Obj]) -> Obj {
    let self_ = unsafe { &*(obj::as_ptr(s) as *const ObjFunBuiltinKw) };
    if n < self_.min_args as usize {
        raise::raise(MpRaise::TypeError("argument num/types mismatch"));
    }
    let mut kw = Map::default();
    map::init(&mut kw, k);
    for i in 0..k {
        let key = a[n + i * 2];
        let val = a[n + i * 2 + 1];
        if let Some(slot) = map::lookup(&mut kw, key, LookupKind::AddIfNotFound) {
            slot.value = val;
        }
    }
    (self_.fun)(n, &a[..n], &kw)
}

fn mk1(f: BuiltinFn1) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltin1>().expect("json fn1");
    unsafe {
        (*o).base.type_ = &T1;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltin1 as *const ())
    }
}

fn mk2(f: BuiltinFn2) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltin2>().expect("json fn2");
    unsafe {
        (*o).base.type_ = &T2;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltin2 as *const ())
    }
}

fn mk_kw(min: u8, f: BuiltinFnKw) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltinKw>().expect("json fnkw");
    unsafe {
        (*o).base.type_ = &TK;
        (*o).min_args = min;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltinKw as *const ())
    }
}

const DUMP_MODE_TO_STRING: usize = 1;
const DUMP_MODE_TO_STREAM: usize = 2;

const S_EOF: u8 = 0;

extern "C" fn stream_print_strn(data: *mut (), str: *const u8, len: usize) {
    let buf = unsafe { std::slice::from_raw_parts(str, len) };
    stream::stream_write_adaptor(data, buf);
}

fn null_terminate(mut data: Vec<u8>) -> Vec<u8> {
    if data.last() != Some(&0) {
        data.push(0);
    }
    data
}

fn set_default_separators(print_ext: &mut PrintExt) {
    print_ext.item_separator = b", \0".as_ptr();
    print_ext.key_separator = b": \0".as_ptr();
}

fn set_separators_from_obj(
    print_ext: &mut PrintExt,
    sep_obj: Obj,
    item_buf: &mut Vec<u8>,
    key_buf: &mut Vec<u8>,
) {
    let (len, items) = obj::get_array(sep_obj);
    if len != 2 {
        raise::raise(MpRaise::TypeError("argument num/types mismatch"));
    }
    *item_buf = null_terminate(objstr::get_str_data_len(items[0]).0);
    *key_buf = null_terminate(objstr::get_str_data_len(items[1]).0);
    print_ext.item_separator = item_buf.as_ptr();
    print_ext.key_separator = key_buf.as_ptr();
}

fn dump_helper_separators(n_args: usize, pos_args: &[Obj], kw_args: &Map, mode: usize) -> Obj {
    let allowed = [Arg {
        qst: qstr::from_str("separators"),
        flags: ArgFlag::KwOnly as u16 | ArgFlag::Obj as u16,
        defval: ArgVal::Obj(obj::CONST_NONE),
    }];
    let mut vals = [ArgVal::Obj(obj::CONST_NONE)];
    let mut kw = kw_args.clone();
    argcheck::parse_all(
        n_args - mode,
        &pos_args[mode..],
        &mut kw,
        allowed.len(),
        &allowed,
        &mut vals,
    );
    let mut item_buf = Vec::new();
    let mut key_buf = Vec::new();
    let mut print_ext = PrintExt {
        base: Print {
            data: core::ptr::null_mut(),
            print_strn: None,
        },
        item_separator: core::ptr::null(),
        key_separator: core::ptr::null(),
    };
    if let ArgVal::Obj(sep) = vals[0] {
        if sep == obj::CONST_NONE {
            set_default_separators(&mut print_ext);
        } else {
            set_separators_from_obj(&mut print_ext, sep, &mut item_buf, &mut key_buf);
        }
    }
    if mode == DUMP_MODE_TO_STRING {
        let mut vstr = Vstr {
            alloc: 0,
            len: 0,
            buf: core::ptr::null_mut(),
            fixed_buf: false,
        };
        vstr::init_print(&mut vstr, 8, &mut print_ext.base);
        obj::print_helper(&print_ext.base, pos_args[0], PrintKind::Json);
        objstr::new_str_from_vstr(&mut vstr)
    } else {
        print_ext.base.data = obj::as_ptr(pos_args[1]) as *mut ();
        print_ext.base.print_strn = Some(stream_print_strn);
        stream::get_stream_raise(pos_args[1], STREAM_OP_WRITE);
        obj::print_helper(&print_ext.base, pos_args[0], PrintKind::Json);
        obj::CONST_NONE
    }
}

fn dump_helper_plain(pos_args: &[Obj], mode: usize) -> Obj {
    if mode == DUMP_MODE_TO_STRING {
        let mut vstr = Vstr {
            alloc: 0,
            len: 0,
            buf: core::ptr::null_mut(),
            fixed_buf: false,
        };
        let mut print = Print {
            data: core::ptr::null_mut(),
            print_strn: None,
        };
        vstr::init_print(&mut vstr, 8, &mut print);
        obj::print_helper(&print, pos_args[0], PrintKind::Json);
        objstr::new_str_from_vstr(&mut vstr)
    } else {
        stream::get_stream_raise(pos_args[1], STREAM_OP_WRITE);
        let mut print = Print {
            data: obj::as_ptr(pos_args[1]) as *mut (),
            print_strn: Some(stream_print_strn),
        };
        obj::print_helper(&print, pos_args[0], PrintKind::Json);
        obj::CONST_NONE
    }
}

fn dump_helper(n_args: usize, pos_args: &[Obj], kw_args: &Map, mode: usize) -> Obj {
    if mpconfig::PY_JSON_SEPARATORS {
        dump_helper_separators(n_args, pos_args, kw_args, mode)
    } else {
        let _ = (n_args, kw_args);
        dump_helper_plain(pos_args, mode)
    }
}

fn json_dump(obj_in: Obj, stream: Obj) -> Obj {
    dump_helper(2, &[obj_in, stream], &Map::default(), DUMP_MODE_TO_STREAM)
}

fn json_dumps(obj_in: Obj) -> Obj {
    dump_helper(1, &[obj_in], &Map::default(), DUMP_MODE_TO_STRING)
}

fn json_dump_kw(n_args: usize, pos_args: &[Obj], kw_args: &Map) -> Obj {
    dump_helper(n_args, pos_args, kw_args, DUMP_MODE_TO_STREAM)
}

fn json_dumps_kw(n_args: usize, pos_args: &[Obj], kw_args: &Map) -> Obj {
    dump_helper(n_args, pos_args, kw_args, DUMP_MODE_TO_STRING)
}

struct JsonStream {
    stream_obj: Obj,
    read: StreamIoFn,
    errcode: i32,
    cur: u8,
}

fn json_stream_next(s: &mut JsonStream) -> u8 {
    let mut errcode = 0;
    let ret = (s.read)(s.stream_obj, &mut s.cur, 1, &mut errcode);
    if errcode != 0 {
        raise::raise(MpRaise::OSError(errcode));
    }
    if ret == 0 {
        s.cur = S_EOF;
    }
    s.cur
}

fn json_syntax_error() -> ! {
    raise::raise(MpRaise::ValueError("syntax error in JSON"));
}

fn json_load(stream_obj: Obj) -> Obj {
    let stream_p = stream::get_stream_raise(stream_obj, STREAM_OP_READ);
    let read = stream_p.read.expect("stream read");
    let mut s = JsonStream {
        stream_obj,
        read,
        errcode: 0,
        cur: 0,
    };
    let mut vstr = Vstr {
        alloc: 0,
        len: 0,
        buf: core::ptr::null_mut(),
        fixed_buf: false,
    };
    vstr::init(&mut vstr, 8);
    let mut stack: Vec<Obj> = Vec::new();
    let mut stack_top = obj::OBJ_NULL;
    let mut stack_top_is_list = false;
    let mut stack_key = obj::OBJ_NULL;
    json_stream_next(&mut s);
    loop {
        if s.cur == S_EOF {
            json_syntax_error();
        }
        let mut next = obj::OBJ_NULL;
        let mut enter = false;
        let cur = s.cur;
        s.cur = json_stream_next(&mut s);
        match cur {
            b',' | b':' | b' ' | b'\t' | b'\n' | b'\r' => continue,
            b'n' => {
                if s.cur == b'u'
                    && json_stream_next(&mut s) == b'l'
                    && json_stream_next(&mut s) == b'l'
                {
                    json_stream_next(&mut s);
                    next = obj::CONST_NONE;
                } else {
                    json_syntax_error();
                }
            }
            b'f' => {
                if s.cur == b'a'
                    && json_stream_next(&mut s) == b'l'
                    && json_stream_next(&mut s) == b's'
                    && json_stream_next(&mut s) == b'e'
                {
                    json_stream_next(&mut s);
                    next = obj::CONST_FALSE;
                } else {
                    json_syntax_error();
                }
            }
            b't' => {
                if s.cur == b'r'
                    && json_stream_next(&mut s) == b'u'
                    && json_stream_next(&mut s) == b'e'
                {
                    json_stream_next(&mut s);
                    next = obj::CONST_TRUE;
                } else {
                    json_syntax_error();
                }
            }
            b'"' => {
                vstr::reset(&mut vstr);
                while s.cur != S_EOF && s.cur != b'"' {
                    let mut c = s.cur;
                    if c == b'\\' {
                        c = json_stream_next(&mut s);
                        match c {
                            b'b' => c = 0x08,
                            b'f' => c = 0x0c,
                            b'n' => c = 0x0a,
                            b'r' => c = 0x0d,
                            b't' => c = 0x09,
                            b'u' => {
                                let mut num = 0u32;
                                for _ in 0..4 {
                                    c = json_stream_next(&mut s) | 0x20;
                                    let mut digit = c.wrapping_sub(b'0');
                                    if digit > 9 {
                                        digit = digit.wrapping_sub(b'a' - (b'9' + 1));
                                    }
                                    num = (num << 4) | digit as u32;
                                }
                                vstr::add_char(&mut vstr, num);
                                s.cur = json_stream_next(&mut s);
                                continue;
                            }
                            _ => {}
                        }
                    }
                    vstr::add_byte(&mut vstr, c);
                    s.cur = json_stream_next(&mut s);
                }
                if s.cur == S_EOF {
                    json_syntax_error();
                }
                s.cur = json_stream_next(&mut s);
                let data = unsafe { std::slice::from_raw_parts(vstr.buf, vstr.len) };
                next = objstr::new_str(data);
            }
            b'-' | b'0'..=b'9' => {
                let mut flt = false;
                let mut cur = cur;
                vstr::reset(&mut vstr);
                loop {
                    vstr::add_byte(&mut vstr, cur);
                    cur = s.cur;
                    if cur == b'.' || cur == b'E' || cur == b'e' {
                        flt = true;
                    } else if cur == b'+' || cur == b'-' || misc::unichar_isdigit(cur as u32) {
                        // pass
                    } else {
                        break;
                    }
                    s.cur = json_stream_next(&mut s);
                }
                let data = unsafe { std::slice::from_raw_parts(vstr.buf, vstr.len) };
                next = if flt {
                    parsenum::parse_num_float(data, false, None)
                } else {
                    parsenum::parse_num_integer(data, 10, None)
                };
            }
            b'[' => {
                next = objlist::new_list(0, None);
                enter = true;
            }
            b'{' => {
                next = objdict::new_dict(0);
                enter = true;
            }
            b'}' | b']' => {
                if stack_top == obj::OBJ_NULL {
                    json_syntax_error();
                }
                if stack.is_empty() {
                    break;
                }
                stack_top = stack.pop().unwrap();
                stack_top_is_list = obj::is_exact_type(stack_top, objlist::type_list());
                continue;
            }
            _ => json_syntax_error(),
        }
        if stack_top == obj::OBJ_NULL {
            stack_top = next;
            stack_top_is_list = obj::is_exact_type(stack_top, objlist::type_list());
            if !enter {
                break;
            }
        } else {
            if stack_top_is_list {
                objlist::list_append(stack_top, next);
            } else if stack_key == obj::OBJ_NULL {
                stack_key = next;
                if enter {
                    json_syntax_error();
                }
            } else {
                objdict::dict_store(stack_top, stack_key, next);
                stack_key = obj::OBJ_NULL;
            }
            if enter {
                stack.push(stack_top);
                stack_top = next;
                stack_top_is_list = obj::is_exact_type(stack_top, objlist::type_list());
            }
        }
    }
    while misc::unichar_isspace(s.cur as u32) {
        s.cur = json_stream_next(&mut s);
    }
    if s.cur != S_EOF {
        json_syntax_error();
    }
    if stack_top == obj::OBJ_NULL || !stack.is_empty() {
        json_syntax_error();
    }
    vstr::clear(&mut vstr);
    stack_top
}

fn json_loads(obj_in: Obj) -> Obj {
    let mut bufinfo = obj::BufferInfo::default();
    obj::get_buffer_raise(obj_in, &mut bufinfo, obj::BUFFER_READ);
    let mut v = Vstr {
        alloc: bufinfo.len,
        len: bufinfo.len,
        buf: bufinfo.buf as *mut u8,
        fixed_buf: true,
    };
    let mut sio = objstringio::ObjStringio {
        base: ObjBase {
            type_: objstringio::type_stringio() as *const ObjType,
        },
        vstr: &mut v as *mut Vstr,
        pos: 0,
        ref_obj: obj::OBJ_NULL,
    };
    json_load(obj::from_ptr(
        &sio as *const objstringio::ObjStringio as *const (),
    ))
}

/// Register built-in `json` module (`MP_REGISTER_EXTENSIBLE_MODULE`).
pub fn init_module() -> Obj {
    if !mpconfig::PY_JSON {
        return obj::OBJ_NULL;
    }
    let dump_fn = if mpconfig::PY_JSON_SEPARATORS {
        mk_kw(2, json_dump_kw)
    } else {
        mk2(json_dump)
    };
    let dumps_fn = if mpconfig::PY_JSON_SEPARATORS {
        mk_kw(1, json_dumps_kw)
    } else {
        mk1(json_dumps)
    };
    let table = [
        MapElem {
            key: obj::new_qstr(qstr::from_str("__name__")),
            value: obj::new_qstr(qstr::from_str("json")),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("dump")),
            value: dump_fn,
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("dumps")),
            value: dumps_fn,
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("load")),
            value: mk1(json_load),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("loads")),
            value: mk1(json_loads),
        },
    ];
    let ctx = malloc::new_obj::<ModuleContext>().expect("json module");
    let dict = objdict::new_dict(table.len());
    unsafe {
        map::init_fixed_table(&mut (*objdict::dict_ptr(dict)).map, table.to_vec());
        (*ctx).module.base.type_ = objmodule::type_module();
        (*ctx).module.globals = objdict::dict_ptr(dict);
        (*ctx).constants = Default::default();
    }
    let module = obj::from_ptr(ctx as *const ModuleContext as *const ());
    objmodule::register_builtin_module(qstr::from_str("json"), module);
    module
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn null_terminate_appends_nul() {
        let buf = null_terminate(b"abc".to_vec());
        assert_eq!(buf, b"abc\0");
    }

    #[test]
    fn default_separators_are_c_strings() {
        let mut ext = PrintExt {
            base: Print {
                data: core::ptr::null_mut(),
                print_strn: None,
            },
            item_separator: core::ptr::null(),
            key_separator: core::ptr::null(),
        };
        set_default_separators(&mut ext);
        assert_eq!(mpprint::json_item_separator(&ext.base), ", ");
        assert_eq!(mpprint::json_key_separator(&ext.base), ": ");
    }
}
