//! rewrite of extmod/modre.c
// symmetry: done

use crate::re15::{compilecode, dumpcode, recursiveloopprog, sizecode, ByteProg, Subject};
use py_rs::bc::ModuleContext;
use py_rs::malloc;
use py_rs::map::{self, MapElem};
use py_rs::mpconfig;
use py_rs::mpprint::{self, Print, PrintKind, VaArg};
use py_rs::obj::{self, Obj, ObjBase, ObjType, TYPE_FLAG_BINDS_SELF, TYPE_FLAG_BUILTIN_FUN};
use py_rs::objdict::{self, ObjDict};
use py_rs::objexcept;
use py_rs::objlist;
use py_rs::objmodule;
use py_rs::objstr;
use py_rs::objtuple;
use py_rs::qstr;
use py_rs::raise::{self, MpRaise};
use py_rs::runtime;
use py_rs::vstr::{self, Vstr};

const FLAG_DEBUG: i32 = 0x1000;

type BuiltinFn1 = fn(Obj) -> Obj;
type BuiltinFn2 = fn(Obj, Obj) -> Obj;
type BuiltinFnVar = fn(usize, &[Obj]) -> Obj;

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
struct ObjFunBuiltinVar {
    base: ObjBase,
    min_args: u8,
    max_args: u8,
    fun: BuiltinFnVar,
}

static mut F1: [*const (); 1] = [call1 as *const ()];
static mut F2: [*const (); 1] = [call2 as *const ()];
static mut FV: [*const (); 1] = [callv as *const ()];
static T1: ObjType = ObjType {
    base: ObjBase {
        type_: core::ptr::null(),
    },
    flags: TYPE_FLAG_BINDS_SELF | TYPE_FLAG_BUILTIN_FUN,
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
    flags: TYPE_FLAG_BINDS_SELF | TYPE_FLAG_BUILTIN_FUN,
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
static TV: ObjType = ObjType {
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
    slots: unsafe { FV.as_ptr() },
};

fn call1(s: Obj, n: usize, k: usize, a: &[Obj]) -> Obj {
    py_rs::argcheck::check_num(n, k, 1, 1, false);
    unsafe { ((*(obj::as_ptr(s) as *const ObjFunBuiltin1)).fun)(a[0]) }
}
fn call2(s: Obj, n: usize, k: usize, a: &[Obj]) -> Obj {
    py_rs::argcheck::check_num(n, k, 2, 2, false);
    unsafe { ((*(obj::as_ptr(s) as *const ObjFunBuiltin2)).fun)(a[0], a[1]) }
}
fn callv(s: Obj, n: usize, k: usize, a: &[Obj]) -> Obj {
    let self_ = unsafe { &*(obj::as_ptr(s) as *const ObjFunBuiltinVar) };
    py_rs::argcheck::check_num(
        n,
        k,
        self_.min_args as usize,
        self_.max_args as usize,
        false,
    );
    (self_.fun)(n, a)
}
fn mk1(f: BuiltinFn1) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltin1>().expect("re fn1");
    unsafe {
        (*o).base.type_ = &T1;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltin1 as *const ())
    }
}
fn mk2(f: BuiltinFn2) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltin2>().expect("re fn2");
    unsafe {
        (*o).base.type_ = &T2;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltin2 as *const ())
    }
}
fn mkv(min: u8, max: u8, f: BuiltinFnVar) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltinVar>().expect("re fnv");
    unsafe {
        (*o).base.type_ = &TV;
        (*o).min_args = min;
        (*o).max_args = max;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltinVar as *const ())
    }
}

#[repr(C)]
struct ObjRe {
    base: ObjBase,
    prog: ByteProg,
}

#[repr(C)]
struct ObjMatch {
    base: ObjBase,
    num_matches: i32,
    str: Obj,
}

fn re_ptr(o: Obj) -> *mut ObjRe {
    obj::as_ptr(o) as *mut ObjRe
}

fn match_ptr(o: Obj) -> *mut ObjMatch {
    obj::as_ptr(o) as *mut ObjMatch
}

fn match_caps(o: *mut ObjMatch, caps_num: usize) -> *mut *const u8 {
    unsafe { (o as *mut u8).add(core::mem::size_of::<ObjMatch>()) as *mut *const u8 }
}

fn is_re_obj(o: Obj) -> bool {
    obj::is_obj(o) && core::ptr::eq(obj::get_type(o), init_re_type())
}

fn pattern_str(o: Obj) -> String {
    objstr::str_get_str(o)
}

fn build_subject(str_obj: Obj, startpos: Option<i32>, endpos: Option<i32>) -> Subject {
    objstr::with_str_bytes(str_obj, |ptr, len| {
        let mut start = startpos.unwrap_or(0);
        if start > len as i32 {
            start = len as i32;
        } else if start < 0 {
            start = 0;
        }
        let mut end = endpos.unwrap_or(len as i32);
        if end > len as i32 {
            end = len as i32;
        } else if end < start {
            end = start;
        }
        Subject {
            begin_line: ptr,
            begin: unsafe { ptr.add(start as usize) },
            end: unsafe { ptr.add(end as usize) },
        }
    })
}

fn match_print(print: &Print, self_in: Obj, _kind: PrintKind) {
    let self_ = unsafe { &*match_ptr(self_in) };
    let _ = mpprint::printf(print, "<match num=%d>", [VaArg::Int(self_.num_matches)]);
}

fn match_group(self_in: Obj, no_in: Obj) -> Obj {
    let self_ = unsafe { &*match_ptr(self_in) };
    let no = obj::get_int(no_in);
    if no < 0 || no >= self_.num_matches as isize {
        raise::raise_obj(objexcept::new_exception_args(
            objexcept::type_index_error(),
            1,
            &[no_in],
        ));
    }
    let caps_num = self_.num_matches as usize * 2;
    let caps = match_caps(match_ptr(self_in), caps_num);
    let start = unsafe { *caps.add(no as usize * 2) };
    if start.is_null() {
        return obj::CONST_NONE;
    }
    let end = unsafe { *caps.add(no as usize * 2 + 1) };
    let len = unsafe { end.offset_from(start) } as usize;
    let str_type = obj::get_type(self_.str);
    objstr::new_str_of_type(str_type, unsafe { std::slice::from_raw_parts(start, len) })
}

fn match_span_helper(n_args: usize, args: &[Obj], span: &mut [Obj; 2]) {
    let self_ = unsafe { &*match_ptr(args[0]) };
    let mut no = 0isize;
    if n_args == 2 {
        no = obj::get_int(args[1]);
        if no < 0 || no >= self_.num_matches as isize {
            raise::raise_obj(objexcept::new_exception_args(
                objexcept::type_index_error(),
                1,
                &[args[1]],
            ));
        }
    }
    let mut s = -1isize;
    let mut e = -1isize;
    let caps_num = self_.num_matches as usize * 2;
    let caps = match_caps(match_ptr(args[0]), caps_num);
    let start = unsafe { *caps.add(no as usize * 2) };
    if !start.is_null() {
        objstr::with_str_bytes(self_.str, |begin, _| {
            s = unsafe { start.offset_from(begin) } as isize;
            let end = unsafe { *caps.add(no as usize * 2 + 1) };
            e = unsafe { end.offset_from(begin) } as isize;
        });
    }
    span[0] = obj::new_int(s);
    span[1] = obj::new_int(e);
}

fn match_span(n_args: usize, args: &[Obj]) -> Obj {
    let mut span = [obj::OBJ_NULL, obj::OBJ_NULL];
    match_span_helper(n_args, args, &mut span);
    objtuple::new_tuple(2, Some(&span))
}

fn match_start(n_args: usize, args: &[Obj]) -> Obj {
    let mut span = [obj::OBJ_NULL, obj::OBJ_NULL];
    match_span_helper(n_args, args, &mut span);
    span[0]
}

fn match_end(n_args: usize, args: &[Obj]) -> Obj {
    let mut span = [obj::OBJ_NULL, obj::OBJ_NULL];
    match_span_helper(n_args, args, &mut span);
    span[1]
}

fn re_print(print: &Print, self_in: Obj, _kind: PrintKind) {
    let _ = mpprint::printf(print, "<re %x>", [VaArg::USize(re_ptr(self_in) as usize)]);
}

fn mod_re_compile(n_args: usize, args: &[Obj]) -> Obj {
    let re_str = pattern_str(args[0]);
    let sz = sizecode(&re_str);
    if sz == -1 {
        if mpconfig::ERROR_REPORTING >= mpconfig::ERROR_REPORTING_NORMAL {
            raise::raise(MpRaise::ValueError("regex too complex"));
        }
        raise::raise(MpRaise::ValueError("error in regex"));
    }
    let o = malloc::new_obj::<ObjRe>().expect("re object");
    unsafe {
        (*o).base.type_ = init_re_type();
        (*o).prog = ByteProg::default();
        if compilecode(&mut (*o).prog, &re_str) != 0 {
            raise::raise(MpRaise::ValueError("error in regex"));
        }
        if mpconfig::PY_RE_DEBUG {
            let flags = if n_args > 1 {
                obj::get_int(args[1]) as i32
            } else {
                0
            };
            if flags & FLAG_DEBUG != 0 {
                dumpcode(&(*o).prog);
            }
        }
        obj::from_ptr(o as *const ObjRe as *const ())
    }
}

fn re_exec_helper(is_anchored: bool, n_args: usize, args: &[Obj]) -> Obj {
    let (self_, was_compiled) = if is_re_obj(args[0]) {
        (args[0], true)
    } else {
        (mod_re_compile(1, &args[..1]), false)
    };
    let prog = unsafe { &(*re_ptr(self_)).prog };
    let mut startpos = None;
    let mut endpos = None;
    if was_compiled && n_args > 2 {
        startpos = Some(obj::get_int(args[2]) as i32);
        if n_args > 3 {
            endpos = Some(obj::get_int(args[3]) as i32);
        }
    }
    let subj = build_subject(args[1], startpos, endpos);
    let caps_num = ((prog.sub + 1) * 2) as usize;
    let match_obj = obj::malloc_var::<ObjMatch>(
        caps_num * core::mem::size_of::<*const u8>(),
        init_match_type(),
    );
    unsafe {
        let caps = match_caps(match_obj, caps_num);
        core::ptr::write_bytes(caps, 0, caps_num);
        let mut cap_slice = std::slice::from_raw_parts_mut(caps, caps_num);
        let res = recursiveloopprog(prog, &subj, &mut cap_slice, caps_num as i32, is_anchored);
        if res == 0 {
            malloc::del_obj(match_obj);
            return obj::CONST_NONE;
        }
        (*match_obj).base.type_ = init_match_type();
        (*match_obj).num_matches = (caps_num / 2) as i32;
        (*match_obj).str = args[1];
        obj::from_ptr(match_obj as *const ObjMatch as *const ())
    }
}

fn re_match(n_args: usize, args: &[Obj]) -> Obj {
    re_exec_helper(true, n_args, args)
}

fn re_search(n_args: usize, args: &[Obj]) -> Obj {
    re_exec_helper(false, n_args, args)
}

fn re_split(n_args: usize, args: &[Obj]) -> Obj {
    let self_ = args[0];
    let prog = unsafe { &(*re_ptr(self_)).prog };
    let str_type = obj::get_type(args[1]);
    let subj = build_subject(args[1], None, None);
    let caps_num = ((prog.sub + 1) * 2) as usize;
    let mut maxsplit = if n_args > 2 {
        obj::get_int(args[2]) as i32
    } else {
        0
    };
    let retval = objlist::new_list(0, None);
    let mut caps = vec![std::ptr::null(); caps_num];
    let mut cur = subj;
    loop {
        caps.fill(std::ptr::null());
        let res = recursiveloopprog(prog, &cur, &mut caps, caps_num as i32, false);
        if res == 0 || caps[0] == caps[1] {
            break;
        }
        let pre_len = unsafe { caps[0].offset_from(cur.begin) } as usize;
        let s = objstr::new_str_of_type(str_type, unsafe {
            std::slice::from_raw_parts(cur.begin, pre_len)
        });
        objlist::list_append(retval, s);
        if prog.sub > 0 {
            raise::raise_obj(objexcept::new_exception_args(
                objexcept::type_not_implemented_error(),
                1,
                &[objstr::new_str(b"splitting with sub-captures")],
            ));
        }
        cur.begin = caps[1];
        if maxsplit > 0 {
            maxsplit -= 1;
            if maxsplit == 0 {
                break;
            }
        }
    }
    let tail_len = unsafe { cur.end.offset_from(cur.begin) } as usize;
    let s = objstr::new_str_of_type(str_type, unsafe {
        std::slice::from_raw_parts(cur.begin, tail_len)
    });
    objlist::list_append(retval, s);
    retval
}

fn re_sub_helper(n_args: usize, args: &[Obj]) -> Obj {
    let self_ = if is_re_obj(args[0]) {
        args[0]
    } else {
        mod_re_compile(1, &args[..1])
    };
    let replace = args[1];
    let where_obj = args[2];
    let mut count = 0isize;
    if n_args > 3 {
        count = obj::get_int(args[3]);
    }
    let prog = unsafe { &(*re_ptr(self_)).prog };
    let mut subj = build_subject(where_obj, None, None);
    let caps_num = ((prog.sub + 1) * 2) as usize;
    let match_obj = obj::malloc_var::<ObjMatch>(
        caps_num * core::mem::size_of::<*const u8>(),
        init_match_type(),
    );
    unsafe {
        (*match_obj).base.type_ = init_match_type();
        (*match_obj).num_matches = (caps_num / 2) as i32;
        (*match_obj).str = where_obj;
    }
    let mut vstr_return = Vstr {
        alloc: 0,
        len: 0,
        buf: core::ptr::null_mut(),
        fixed_buf: false,
    };
    loop {
        let caps = match_caps(match_obj, caps_num);
        unsafe {
            core::ptr::write_bytes(caps, 0, caps_num);
        }
        let mut cap_slice = unsafe { std::slice::from_raw_parts_mut(caps, caps_num) };
        let res = recursiveloopprog(prog, &subj, &mut cap_slice, caps_num as i32, false);
        if res == 0 || cap_slice[0] == cap_slice[1] {
            break;
        }
        if vstr_return.buf.is_null() {
            let pre = unsafe { cap_slice[0].offset_from(subj.begin) } as usize;
            vstr::init(&mut vstr_return, pre);
        }
        let pre_len = unsafe { cap_slice[0].offset_from(subj.begin) } as usize;
        vstr::add_strn(&mut vstr_return, unsafe {
            std::slice::from_raw_parts(subj.begin, pre_len)
        });
        let repl_obj = if obj::is_callable(replace) {
            runtime::call_function_1(
                replace,
                obj::from_ptr(match_obj as *const ObjMatch as *const ()),
            )
        } else {
            replace
        };
        let repl = pattern_str(repl_obj);
        let mut repl = repl.as_bytes();
        while !repl.is_empty() {
            if repl[0] == b'\\' {
                repl = &repl[1..];
                if repl.is_empty() {
                    break;
                }
                let mut is_g_format = false;
                if repl[0] == b'g' && repl.len() > 1 && repl[1] == b'<' {
                    repl = &repl[2..];
                    is_g_format = true;
                }
                if repl[0] >= b'0' && repl[0] <= b'9' {
                    let mut match_no = 0u32;
                    while !repl.is_empty() && repl[0] >= b'0' && repl[0] <= b'9' {
                        match_no = match_no * 10 + (repl[0] - b'0') as u32;
                        repl = &repl[1..];
                    }
                    if is_g_format && !repl.is_empty() && repl[0] == b'>' {
                        repl = &repl[1..];
                    }
                    if match_no >= caps_num as u32 / 2 {
                        raise::raise_obj(objexcept::new_exception_args(
                            objexcept::type_index_error(),
                            1,
                            &[obj::new_small_int(match_no as isize)],
                        ));
                    }
                    let start_match = cap_slice[match_no as usize * 2];
                    if !start_match.is_null() {
                        let end_match = cap_slice[match_no as usize * 2 + 1];
                        let mlen = unsafe { end_match.offset_from(start_match) } as usize;
                        vstr::add_strn(&mut vstr_return, unsafe {
                            std::slice::from_raw_parts(start_match, mlen)
                        });
                    }
                } else if repl[0] == b'\\' {
                    vstr::add_byte(&mut vstr_return, repl[0]);
                    repl = &repl[1..];
                }
            } else {
                vstr::add_byte(&mut vstr_return, repl[0]);
                repl = &repl[1..];
            }
        }
        subj.begin = cap_slice[1];
        if count > 0 {
            count -= 1;
            if count == 0 {
                break;
            }
        }
    }
    malloc::del_obj(match_obj);
    if vstr_return.buf.is_null() {
        return where_obj;
    }
    let tail_len = unsafe { subj.end.offset_from(subj.begin) } as usize;
    vstr::add_strn(&mut vstr_return, unsafe {
        std::slice::from_raw_parts(subj.begin, tail_len)
    });
    if core::ptr::eq(obj::get_type(where_obj), objstr::type_str()) {
        objstr::new_str_from_vstr(&mut vstr_return)
    } else {
        objstr::new_bytes_from_vstr(&mut vstr_return)
    }
}

static mut MATCH_SLOTS: [*const (); 2] = [core::ptr::null(), core::ptr::null()];
static mut TYPE_MATCH: ObjType = ObjType {
    base: ObjBase {
        type_: core::ptr::null(),
    },
    flags: obj::TYPE_FLAG_NONE,
    name: 0,
    slot_index_make_new: 0,
    slot_index_print: 1,
    slot_index_call: 0,
    slot_index_unary_op: 0,
    slot_index_binary_op: 0,
    slot_index_attr: 0,
    slot_index_subscr: 0,
    slot_index_iter: 0,
    slot_index_buffer: 0,
    slot_index_protocol: 0,
    slot_index_parent: 0,
    slot_index_locals_dict: 2,
    slots: unsafe { MATCH_SLOTS.as_ptr() },
};

static MATCH_INIT: std::sync::OnceLock<()> = std::sync::OnceLock::new();

fn init_match_type() -> &'static ObjType {
    MATCH_INIT.get_or_init(|| {
        unsafe {
            MATCH_SLOTS[0] = match_print as *const ();
        }
        let mut table = vec![MapElem {
            key: obj::new_qstr(qstr::from_str("group")),
            value: mk2(match_group),
        }];
        if mpconfig::PY_RE_MATCH_GROUPS {
            table.push(MapElem {
                key: obj::new_qstr(qstr::from_str("groups")),
                value: mk1(match_groups_impl),
            });
        }
        if mpconfig::PY_RE_MATCH_SPAN_START_END {
            table.extend([
                MapElem {
                    key: obj::new_qstr(qstr::from_str("span")),
                    value: mkv(1, 2, match_span),
                },
                MapElem {
                    key: obj::new_qstr(qstr::from_str("start")),
                    value: mkv(1, 2, match_start),
                },
                MapElem {
                    key: obj::new_qstr(qstr::from_str("end")),
                    value: mkv(1, 2, match_end),
                },
            ]);
        }
        let ptr = obj::malloc_helper(core::mem::size_of::<ObjDict>(), objdict::type_dict())
            as *mut ObjDict;
        unsafe {
            map::init_fixed_table(&mut (*ptr).map, table);
            MATCH_SLOTS[1] = obj::from_ptr(ptr as *const ObjDict as *const ()).0 as *const ();
            TYPE_MATCH.name = qstr::from_str("match");
        }
    });
    unsafe { &TYPE_MATCH }
}

fn match_groups_impl(self_in: Obj) -> Obj {
    let self_ = unsafe { &*match_ptr(self_in) };
    if self_.num_matches <= 1 {
        return objtuple::new_tuple(0, None);
    }
    let n = (self_.num_matches - 1) as usize;
    let mut items = vec![obj::OBJ_NULL; n];
    for i in 0..n {
        items[i] = match_group(self_in, obj::new_small_int((i + 1) as isize));
    }
    objtuple::new_tuple(n, Some(&items))
}

static mut RE_SLOTS: [*const (); 2] = [core::ptr::null(), core::ptr::null()];
static mut TYPE_RE: ObjType = ObjType {
    base: ObjBase {
        type_: core::ptr::null(),
    },
    flags: obj::TYPE_FLAG_NONE,
    name: 0,
    slot_index_make_new: 0,
    slot_index_print: 1,
    slot_index_call: 0,
    slot_index_unary_op: 0,
    slot_index_binary_op: 0,
    slot_index_attr: 0,
    slot_index_subscr: 0,
    slot_index_iter: 0,
    slot_index_buffer: 0,
    slot_index_protocol: 0,
    slot_index_parent: 0,
    slot_index_locals_dict: 2,
    slots: unsafe { RE_SLOTS.as_ptr() },
};

static RE_INIT: std::sync::OnceLock<()> = std::sync::OnceLock::new();

fn init_re_type() -> &'static ObjType {
    RE_INIT.get_or_init(|| {
        unsafe {
            RE_SLOTS[0] = re_print as *const ();
        }
        let mut table = vec![
            MapElem {
                key: obj::new_qstr(qstr::from_str("match")),
                value: mkv(2, 4, re_match),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("search")),
                value: mkv(2, 4, re_search),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("split")),
                value: mkv(2, 3, re_split),
            },
        ];
        if mpconfig::PY_RE_SUB {
            table.push(MapElem {
                key: obj::new_qstr(qstr::from_str("sub")),
                value: mkv(3, 5, re_sub_helper),
            });
        }
        let ptr = obj::malloc_helper(core::mem::size_of::<ObjDict>(), objdict::type_dict())
            as *mut ObjDict;
        unsafe {
            map::init_fixed_table(&mut (*ptr).map, table);
            RE_SLOTS[1] = obj::from_ptr(ptr as *const ObjDict as *const ()).0 as *const ();
            TYPE_RE.name = qstr::from_str("re");
        }
    });
    unsafe { &TYPE_RE }
}

/// Register built-in `re` module (`MP_REGISTER_EXTENSIBLE_MODULE`).
pub fn init_module() -> Obj {
    if !mpconfig::PY_RE {
        return obj::OBJ_NULL;
    }
    init_re_type();
    init_match_type();
    let mut table = vec![
        MapElem {
            key: obj::new_qstr(qstr::from_str("__name__")),
            value: obj::new_qstr(qstr::from_str("re")),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("compile")),
            value: mkv(1, 2, mod_re_compile),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("match")),
            value: mkv(2, 4, re_match),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("search")),
            value: mkv(2, 4, re_search),
        },
    ];
    if mpconfig::PY_RE_SUB {
        table.push(MapElem {
            key: obj::new_qstr(qstr::from_str("sub")),
            value: mkv(3, 5, re_sub_helper),
        });
    }
    if mpconfig::PY_RE_DEBUG {
        table.push(MapElem {
            key: obj::new_qstr(qstr::from_str("DEBUG")),
            value: obj::new_int(FLAG_DEBUG as isize),
        });
    }
    let ctx = malloc::new_obj::<ModuleContext>().expect("re module");
    let dict = objdict::new_dict(table.len());
    unsafe {
        map::init_fixed_table(&mut (*objdict::dict_ptr(dict)).map, table);
        (*ctx).module.base.type_ = objmodule::type_module();
        (*ctx).module.globals = objdict::dict_ptr(dict);
        (*ctx).constants = Default::default();
    }
    let module = obj::from_ptr(ctx as *const ModuleContext as *const ());
    objmodule::register_builtin_module(qstr::from_str("re"), module);
    module
}

#[cfg(test)]
mod tests {
    use super::*;
    use py_rs::gc;
    use py_rs::mpstate;

    fn setup() {
        let _ = gc::init();
        qstr::init();
        mpstate::init();
    }

    #[test]
    fn compile_match_search_sub() {
        setup();
        let pat = mod_re_compile(1, &[objstr::new_str(b"a(b+)y")]);
        let hay = objstr::new_str(b"xxabbbyy");
        let m = re_search(2, &[pat, hay]);
        assert_ne!(m, obj::CONST_NONE);
        let anchored = mod_re_compile(1, &[objstr::new_str(b"^xx")]);
        let m2 = re_match(2, &[anchored, hay]);
        assert_ne!(m2, obj::CONST_NONE);
        let out = re_sub_helper(3, &[objstr::new_str(b"b+"), objstr::new_str(b"X"), hay]);
        assert_eq!(objstr::str_get_str(out), "xxaXyy");
    }

    #[test]
    fn module_registers_when_enabled() {
        setup();
        let m = init_module();
        assert_ne!(m, obj::OBJ_NULL);
    }
}
