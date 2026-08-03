//! rewrite of py/objdict.c
// symmetry: done

use core::mem::size_of;

use crate::argcheck;
use crate::malloc;
use crate::map::{self, LookupKind, Map, MapElem};
use crate::mpconfig;
use crate::mpprint::{self, Print, PrintKind};
use crate::obj::{
    self, IterNextFn, Obj, ObjBase, ObjIterBuf, ObjType, OBJ_NULL, OBJ_SENTINEL,
    TYPE_FLAG_BINDS_SELF, TYPE_FLAG_BUILTIN_FUN, TYPE_FLAG_ITER_IS_ITERNEXT,
};
use crate::objexcept;
use crate::objlist;
use crate::objpolyiter;
use crate::objtuple;
use crate::qstr;
use crate::raise::{self, MpRaise};
use crate::runtime;
use crate::runtime0::{BinaryOp, UnaryOp};

#[repr(C)]
pub struct ObjDict {
    pub base: ObjBase,
    pub map: Map,
}

#[repr(u8)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum DictViewKind {
    Items = 0,
    Keys = 1,
    Values = 2,
}

const DICT_VIEW_NAMES: [&str; 3] = ["dict_items", "dict_keys", "dict_values"];

// --- builtin method wrappers --------------------------------------------------

type BuiltinFn1 = fn(Obj) -> Obj;
type BuiltinFnVar = fn(usize, &[Obj]) -> Obj;
type BuiltinFnKw = fn(usize, &[Obj], &Map) -> Obj;

#[repr(C)]
struct ObjFunBuiltin1 {
    base: ObjBase,
    fun: BuiltinFn1,
}

#[repr(C)]
struct ObjFunBuiltinVar {
    base: ObjBase,
    min_args: u8,
    max_args: u8,
    fun: BuiltinFnVar,
}

#[repr(C)]
struct ObjFunBuiltinKw {
    base: ObjBase,
    min_args: u8,
    fun: BuiltinFnKw,
}

static mut FUN_BUILTIN_1_SLOTS: [*const (); 1] = [fun_builtin_1_call as *const ()];
static mut FUN_BUILTIN_VAR_SLOTS: [*const (); 1] = [fun_builtin_var_call as *const ()];
static mut FUN_BUILTIN_KW_SLOTS: [*const (); 1] = [fun_builtin_kw_call as *const ()];

static TYPE_FUN_BUILTIN_1: ObjType = ObjType {
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
    slots: unsafe { FUN_BUILTIN_1_SLOTS.as_ptr() },
};

static TYPE_FUN_BUILTIN_VAR: ObjType = ObjType {
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
    slots: unsafe { FUN_BUILTIN_VAR_SLOTS.as_ptr() },
};

static TYPE_FUN_BUILTIN_KW: ObjType = ObjType {
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
    slots: unsafe { FUN_BUILTIN_KW_SLOTS.as_ptr() },
};

fn fun_builtin_1_call(self_in: Obj, n_args: usize, n_kw: usize, args: &[Obj]) -> Obj {
    argcheck::check_num(n_args, n_kw, 1, 1, false);
    let self_ = unsafe { &*(obj::as_ptr(self_in) as *const ObjFunBuiltin1) };
    (self_.fun)(args[0])
}

fn fun_builtin_var_call(self_in: Obj, n_args: usize, n_kw: usize, args: &[Obj]) -> Obj {
    let self_ = unsafe { &*(obj::as_ptr(self_in) as *const ObjFunBuiltinVar) };
    argcheck::check_num(
        n_args,
        n_kw,
        self_.min_args as usize,
        self_.max_args as usize,
        false,
    );
    (self_.fun)(n_args, args)
}

fn fun_builtin_kw_call(self_in: Obj, n_args: usize, n_kw: usize, args: &[Obj]) -> Obj {
    let self_ = unsafe { &*(obj::as_ptr(self_in) as *const ObjFunBuiltinKw) };
    if n_args < self_.min_args as usize {
        raise::raise(MpRaise::TypeError("argument num/types mismatch"));
    }
    let mut kw = Map::default();
    map::init(&mut kw, n_kw);
    for i in 0..n_kw {
        let key = args[n_args + i * 2];
        let val = args[n_args + i * 2 + 1];
        if let Some(slot) = map::lookup(&mut kw, key, LookupKind::AddIfNotFound) {
            slot.value = val;
        }
    }
    (self_.fun)(n_args, &args[..n_args], &kw)
}

fn new_fun_builtin_1(fun: BuiltinFn1) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltin1>().expect("fun_builtin_1 alloc");
    unsafe {
        (*o).base.type_ = &TYPE_FUN_BUILTIN_1 as *const ObjType;
        (*o).fun = fun;
        obj::from_ptr(o as *const ObjFunBuiltin1 as *const ())
    }
}

fn new_fun_builtin_var(min_args: u8, max_args: u8, fun: BuiltinFnVar) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltinVar>().expect("fun_builtin_var alloc");
    unsafe {
        (*o).base.type_ = &TYPE_FUN_BUILTIN_VAR as *const ObjType;
        (*o).min_args = min_args;
        (*o).max_args = max_args;
        (*o).fun = fun;
        obj::from_ptr(o as *const ObjFunBuiltinVar as *const ())
    }
}

fn new_fun_builtin_kw(min_args: u8, fun: BuiltinFnKw) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltinKw>().expect("fun_builtin_kw alloc");
    unsafe {
        (*o).base.type_ = &TYPE_FUN_BUILTIN_KW as *const ObjType;
        (*o).min_args = min_args;
        (*o).fun = fun;
        obj::from_ptr(o as *const ObjFunBuiltinKw as *const ())
    }
}

// --- dict view / iterator -----------------------------------------------------

#[repr(C)]
struct ObjDictView {
    base: ObjBase,
    dict: Obj,
    kind: DictViewKind,
}

#[repr(C)]
struct ObjDictViewIter {
    base: ObjBase,
    kind: DictViewKind,
    dict: Obj,
    cur: usize,
}

static mut DICT_VIEW_IT_SLOTS: [*const (); 1] = [dict_view_it_iternext as *const ()];

static TYPE_DICT_VIEW_IT: ObjType = ObjType {
    base: ObjBase {
        type_: core::ptr::null(),
    },
    flags: TYPE_FLAG_ITER_IS_ITERNEXT,
    name: 0,
    slot_index_make_new: 0,
    slot_index_print: 0,
    slot_index_call: 0,
    slot_index_unary_op: 0,
    slot_index_binary_op: 0,
    slot_index_attr: 0,
    slot_index_subscr: 0,
    slot_index_iter: 1,
    slot_index_buffer: 0,
    slot_index_protocol: 0,
    slot_index_parent: 0,
    slot_index_locals_dict: 0,
    slots: unsafe { DICT_VIEW_IT_SLOTS.as_ptr() },
};

static mut DICT_VIEW_SLOTS: [*const (); 4] = [
    dict_view_print as *const (),
    dict_view_unary_op as *const (),
    dict_view_binary_op as *const (),
    dict_view_getiter as *const (),
];

static TYPE_DICT_VIEW: ObjType = ObjType {
    base: ObjBase {
        type_: core::ptr::null(),
    },
    flags: obj::TYPE_FLAG_NONE,
    name: 0,
    slot_index_make_new: 0,
    slot_index_print: 1,
    slot_index_call: 0,
    slot_index_unary_op: 2,
    slot_index_binary_op: 3,
    slot_index_attr: 0,
    slot_index_subscr: 0,
    slot_index_iter: 4,
    slot_index_buffer: 0,
    slot_index_protocol: 0,
    slot_index_parent: 0,
    slot_index_locals_dict: 0,
    slots: unsafe { DICT_VIEW_SLOTS.as_ptr() },
};

// --- dict type slots ----------------------------------------------------------

static mut DICT_SLOTS: [*const (); 7] = [
    dict_make_new as *const (),
    dict_print as *const (),
    dict_unary_op as *const (),
    dict_binary_op as *const (),
    dict_subscr as *const (),
    dict_getiter as *const (),
    core::ptr::null(),
];

static mut TYPE: ObjType = ObjType {
    base: ObjBase {
        type_: core::ptr::null(),
    },
    flags: obj::TYPE_FLAG_NONE,
    name: 0,
    slot_index_make_new: 1,
    slot_index_print: 2,
    slot_index_unary_op: 3,
    slot_index_binary_op: 4,
    slot_index_subscr: 5,
    slot_index_call: 0,
    slot_index_attr: 0,
    slot_index_iter: 6,
    slot_index_buffer: 0,
    slot_index_protocol: 0,
    slot_index_parent: 0,
    slot_index_locals_dict: 7,
    slots: unsafe { DICT_SLOTS.as_ptr() },
};

// OrderedDict shares dict methods; parent = dict (C `mp_type_ordereddict`).
static mut ORDEREDDICT_SLOTS: [*const (); 8] = [
    dict_make_new as *const (),
    dict_print as *const (),
    dict_unary_op as *const (),
    dict_binary_op as *const (),
    dict_subscr as *const (),
    dict_getiter as *const (),
    core::ptr::null(), // locals
    core::ptr::null(), // parent
];

static mut TYPE_ORDEREDDICT: ObjType = ObjType {
    base: ObjBase {
        type_: core::ptr::null(),
    },
    flags: obj::TYPE_FLAG_NONE,
    name: 0,
    slot_index_make_new: 1,
    slot_index_print: 2,
    slot_index_unary_op: 3,
    slot_index_binary_op: 4,
    slot_index_subscr: 5,
    slot_index_call: 0,
    slot_index_attr: 0,
    slot_index_iter: 6,
    slot_index_buffer: 0,
    slot_index_protocol: 0,
    slot_index_parent: 8,
    slot_index_locals_dict: 7,
    slots: unsafe { ORDEREDDICT_SLOTS.as_ptr() },
};

static DICT_INIT: std::sync::OnceLock<()> = std::sync::OnceLock::new();
static EMPTY_DICT: std::sync::OnceLock<Obj> = std::sync::OnceLock::new();

fn init_dict_type() {
    DICT_INIT.get_or_init(|| {
        unsafe {
            TYPE.name = qstr::from_str("dict");
        }
        let mut table = vec![
            MapElem {
                key: obj::new_qstr(qstr::from_str("clear")),
                value: new_fun_builtin_1(dict_clear),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("copy")),
                value: new_fun_builtin_1(dict_copy),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("get")),
                value: new_fun_builtin_var(2, 3, dict_get_method),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("items")),
                value: new_fun_builtin_1(dict_items),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("keys")),
                value: new_fun_builtin_1(dict_keys),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("pop")),
                value: new_fun_builtin_var(2, 3, dict_pop),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("popitem")),
                value: new_fun_builtin_1(dict_popitem),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("setdefault")),
                value: new_fun_builtin_var(2, 3, dict_setdefault),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("update")),
                value: new_fun_builtin_kw(1, dict_update),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("values")),
                value: new_fun_builtin_1(dict_values),
            },
        ];
        if mpconfig::PY_BUILTINS_DICT_FROMKEYS {
            table.insert(
                2,
                MapElem {
                    key: obj::new_qstr(qstr::from_str("fromkeys")),
                    value: crate::objtype::new_classmethod(new_fun_builtin_var(
                        2,
                        3,
                        dict_fromkeys,
                    )),
                },
            );
        }
        let ptr = obj::malloc_helper(size_of::<ObjDict>(), unsafe { &TYPE }) as *mut ObjDict;
        unsafe {
            map::init_fixed_table(&mut (*ptr).map, table);
            let locals = obj::from_ptr(ptr as *const ObjDict as *const ()).0 as *const ();
            DICT_SLOTS[6] = locals;
            if mpconfig::PY_COLLECTIONS_ORDEREDDICT {
                TYPE_ORDEREDDICT.name = qstr::from_str("OrderedDict");
                ORDEREDDICT_SLOTS[6] = locals;
                ORDEREDDICT_SLOTS[7] = &raw const TYPE as *const ObjType as *const ();
            }
            crate::gc::add_root(ptr as *mut u8);
            for elem in &(*ptr).map.table {
                if elem.key != obj::OBJ_NULL
                    && elem.key != obj::OBJ_SENTINEL
                    && obj::is_obj(elem.value)
                {
                    crate::gc::add_root(obj::to_ptr(elem.value) as *mut u8);
                }
            }
        }
        // empty fixed dict singleton
        let empty = malloc::new_obj::<ObjDict>().expect("empty dict");
        unsafe {
            (*empty).base.type_ = &raw const TYPE as *const ObjType;
            (*empty).map = Map {
                all_keys_are_qstrs: false,
                is_fixed: true,
                is_ordered: true,
                used: 0,
                alloc: 0,
                table: Vec::new(),
            };
        }
        let empty_obj = obj::from_ptr(empty as *const ObjDict as *const ());
        crate::gc::add_root(empty as *mut u8);
        let _ = EMPTY_DICT.set(empty_obj);
    });
}

pub fn type_dict() -> &'static ObjType {
    init_dict_type();
    unsafe { &TYPE }
}

pub fn type_ordereddict() -> &'static ObjType {
    init_dict_type();
    unsafe { &TYPE_ORDEREDDICT }
}

pub fn const_empty_dict() -> Obj {
    init_dict_type();
    *EMPTY_DICT.get().expect("empty dict")
}

pub fn dict_ptr(o: Obj) -> *mut ObjDict {
    obj::as_ptr(o) as *mut ObjDict
}

fn check_self(o: Obj) {
    if !is_dict_or_ordereddict(o) {
        raise::raise(MpRaise::TypeError("dict method on non-dict"));
    }
}

fn ensure_not_fixed(dict: &ObjDict) {
    if dict.map.is_fixed {
        raise::raise(MpRaise::TypeError("dict is read-only"));
    }
}

/// `mp_obj_is_dict_or_ordereddict`
pub fn is_dict_or_ordereddict(o: Obj) -> bool {
    if !obj::is_obj(o) {
        return false;
    }
    obj::type_get_make_new(obj::get_type(o)) == Some(dict_make_new)
}

fn dict_iter_next<'a>(dict: &'a ObjDict, cur: &mut usize) -> Option<&'a MapElem> {
    let max = dict.map.alloc;
    let i = *cur;
    for pos in i..max {
        if map::slot_is_filled(&dict.map, pos) {
            *cur = pos + 1;
            return Some(&dict.map.table[pos]);
        }
    }
    None
}

// --- public C API -------------------------------------------------------------

pub fn dict_init(dict: *mut ObjDict, n: usize) {
    unsafe {
        (*dict).base.type_ = type_dict() as *const ObjType;
        map::init(&mut (*dict).map, n);
    }
}

pub fn new_dict(n: usize) -> Obj {
    let o = malloc::new_obj::<ObjDict>().expect("dict alloc");
    dict_init(o, n);
    obj::from_ptr(o as *const ObjDict as *const ())
}

pub fn empty_dict() -> Obj {
    new_dict(0)
}

pub fn dict_len(o: Obj) -> usize {
    unsafe { (*dict_ptr(o)).map.used }
}

pub fn dict_get(o: Obj, key: Obj) -> Obj {
    let dict = unsafe { &mut *dict_ptr(o) };
    match map::lookup(&mut dict.map, key, LookupKind::Lookup) {
        Some(elem) => elem.value,
        None => raise::raise_obj(objexcept::new_exception_args(
            objexcept::type_key_error(),
            1,
            &[key],
        )),
    }
}

pub fn dict_store(o: Obj, key: Obj, value: Obj) -> Obj {
    check_self(o);
    let dict = unsafe { &mut *dict_ptr(o) };
    ensure_not_fixed(dict);
    if let Some(elem) = map::lookup(&mut dict.map, key, LookupKind::AddIfNotFound) {
        elem.value = value;
    }
    o
}

pub fn dict_delete(o: Obj, key: Obj) -> Obj {
    let args = [o, key];
    dict_get_helper(2, &args, LookupKind::RemoveIfFound);
    o
}

/// `mp_obj_dict_copy`
pub fn dict_copy(o: Obj) -> Obj {
    check_self(o);
    let src = unsafe { &*dict_ptr(o) };
    let other = malloc::new_obj::<ObjDict>().expect("dict copy");
    unsafe {
        (*other).base.type_ = src.base.type_;
        (*other).map = src.map.clone();
        (*other).map.is_fixed = false;
    }
    obj::from_ptr(other as *const ObjDict as *const ())
}

// --- type methods -------------------------------------------------------------

pub fn dict_print(print: &Print, self_in: Obj, kind: PrintKind) {
    let self_ = unsafe { &*dict_ptr(self_in) };
    let mut first = true;
    let mut kind = kind;
    let (item_separator, key_separator) = if mpconfig::PY_JSON && kind == PrintKind::Json {
        (
            mpprint::json_item_separator(print),
            mpprint::json_key_separator(print),
        )
    } else {
        (", ", ": ")
    };
    if !(mpconfig::PY_JSON && kind == PrintKind::Json) {
        kind = PrintKind::Repr;
    }
    if mpconfig::PY_COLLECTIONS_ORDEREDDICT
        && self_in != const_empty_dict()
        && !obj::is_exact_type(self_in, type_dict())
        && kind != PrintKind::Json
    {
        let name = obj::get_type(self_in).name;
        let _ = mpprint::printf(print, "%q(", std::iter::once(mpprint::VaArg::Qstr(name)));
    }
    mpprint::print_str(print, "{");
    let mut cur = 0;
    while let Some(next) = dict_iter_next(self_, &mut cur) {
        if !first {
            mpprint::print_str(print, item_separator);
        }
        first = false;
        let add_quote =
            mpconfig::PY_JSON && kind == PrintKind::Json && !obj::is_str_or_bytes(next.key);
        if add_quote {
            mpprint::print_str(print, "\"");
        }
        obj::print_helper(print, next.key, kind);
        if add_quote {
            mpprint::print_str(print, "\"");
        }
        mpprint::print_str(print, key_separator);
        obj::print_helper(print, next.value, kind);
    }
    mpprint::print_str(print, "}");
    if mpconfig::PY_COLLECTIONS_ORDEREDDICT
        && self_in != const_empty_dict()
        && !obj::is_exact_type(self_in, type_dict())
        && kind != PrintKind::Json
    {
        mpprint::print_str(print, ")");
    }
}

pub fn dict_make_new(type_in: &ObjType, n_args: usize, n_kw: usize, args: &[Obj]) -> Obj {
    let dict_out = new_dict(0);
    unsafe {
        (*(dict_ptr(dict_out))).base.type_ = type_in as *const ObjType;
        if mpconfig::PY_COLLECTIONS_ORDEREDDICT
            && type_in as *const ObjType != type_dict() as *const ObjType
        {
            (*(dict_ptr(dict_out))).map.is_ordered = true;
        }
    }
    if n_args > 0 || n_kw > 0 {
        let args2 = [dict_out, if n_args > 0 { args[0] } else { obj::CONST_NONE }];
        let mut kwargs = Map::default();
        map::init(&mut kwargs, n_kw);
        for i in 0..n_kw {
            let key = args[n_args + i * 2];
            let val = args[n_args + i * 2 + 1];
            if let Some(slot) = map::lookup(&mut kwargs, key, LookupKind::AddIfNotFound) {
                slot.value = val;
            }
        }
        dict_update(n_args + 1, &args2[..n_args + 1], &kwargs);
    }
    dict_out
}

pub fn dict_unary_op(op: UnaryOp, self_in: Obj) -> Obj {
    let self_ = unsafe { &*dict_ptr(self_in) };
    match op {
        UnaryOp::Bool => obj::new_bool(self_.map.used != 0),
        UnaryOp::Len => obj::new_small_int(self_.map.used as obj::Int),
        UnaryOp::Sizeof if mpconfig::PY_SYS_GETSIZEOF => {
            let sz = size_of::<ObjDict>() + size_of::<MapElem>() * self_.map.alloc;
            obj::new_small_int(sz as obj::Int)
        }
        _ => obj::OBJ_NULL,
    }
}

pub fn dict_binary_op(op: BinaryOp, lhs_in: Obj, rhs_in: Obj) -> Obj {
    let o = unsafe { &*dict_ptr(lhs_in) };
    match op {
        BinaryOp::Contains => {
            let mut map_copy = o.map.clone();
            let elem = map::lookup(&mut map_copy, rhs_in, LookupKind::Lookup);
            obj::new_bool(elem.is_some())
        }
        BinaryOp::Equal => {
            if mpconfig::PY_COLLECTIONS_ORDEREDDICT
                && obj::is_exact_type(lhs_in, type_dict())
                && obj::is_exact_type(rhs_in, type_dict())
            {
                let rhs = unsafe { &*dict_ptr(rhs_in) };
                if o.map.is_ordered && rhs.map.is_ordered {
                    let mut c1 = 0usize;
                    let mut c2 = 0usize;
                    loop {
                        let e1 = dict_iter_next(o, &mut c1);
                        let e2 = dict_iter_next(rhs, &mut c2);
                        match (e1, e2) {
                            (Some(a), Some(b)) => {
                                if !obj::equal(a.key, b.key) || !obj::equal(a.value, b.value) {
                                    return obj::CONST_FALSE;
                                }
                            }
                            (None, None) => return obj::CONST_TRUE,
                            _ => return obj::CONST_FALSE,
                        }
                    }
                }
            }
            if is_dict_or_ordereddict(rhs_in) {
                let rhs = unsafe { &*dict_ptr(rhs_in) };
                if o.map.used != rhs.map.used {
                    return obj::CONST_FALSE;
                }
                let mut cur = 0;
                while let Some(next) = dict_iter_next(o, &mut cur) {
                    let mut rhs_map = rhs.map.clone();
                    let elem = map::lookup(&mut rhs_map, next.key, LookupKind::Lookup);
                    if elem.is_none() || !obj::equal(next.value, elem.unwrap().value) {
                        return obj::CONST_FALSE;
                    }
                }
                obj::CONST_TRUE
            } else {
                obj::CONST_FALSE
            }
        }
        BinaryOp::Or | BinaryOp::InplaceOr if mpconfig::CPYTHON_COMPAT => {
            let lhs = if op == BinaryOp::Or {
                dict_copy(lhs_in)
            } else {
                lhs_in
            };
            let dicts = [lhs, rhs_in];
            dict_update(2, &dicts, &Map::default());
            lhs
        }
        _ => obj::OBJ_NULL,
    }
}

pub fn dict_subscr(self_in: Obj, index: Obj, value: Obj) -> Obj {
    if value == OBJ_NULL {
        dict_delete(self_in, index);
        obj::CONST_NONE
    } else if value == OBJ_SENTINEL {
        dict_get(self_in, index)
    } else {
        dict_store(self_in, index, value);
        obj::CONST_NONE
    }
}

fn dict_get_helper(n_args: usize, args: &[Obj], lookup_kind: LookupKind) -> Obj {
    check_self(args[0]);
    let self_ = unsafe { &mut *dict_ptr(args[0]) };
    if lookup_kind != LookupKind::Lookup {
        ensure_not_fixed(self_);
    }
    let elem = map::lookup(&mut self_.map, args[1], lookup_kind);
    if elem.is_none() || elem.as_ref().unwrap().value == OBJ_NULL {
        let value = if n_args == 2 {
            if lookup_kind == LookupKind::RemoveIfFound {
                raise::raise_obj(objexcept::new_exception_args(
                    objexcept::type_key_error(),
                    1,
                    &[args[1]],
                ));
            }
            obj::CONST_NONE
        } else {
            args[2]
        };
        if lookup_kind == LookupKind::AddIfNotFound {
            if let Some(slot) = map::lookup(&mut self_.map, args[1], LookupKind::AddIfNotFound) {
                slot.value = value;
            }
        }
        value
    } else if lookup_kind == LookupKind::RemoveIfFound {
        let v = elem.unwrap().value;
        if let Some(slot) = map::lookup(&mut self_.map, args[1], LookupKind::RemoveIfFound) {
            slot.value = OBJ_NULL;
        }
        v
    } else {
        elem.unwrap().value
    }
}

pub fn dict_get_method(n_args: usize, args: &[Obj]) -> Obj {
    dict_get_helper(n_args, args, LookupKind::Lookup)
}

pub fn dict_clear(self_in: Obj) -> Obj {
    check_self(self_in);
    let self_ = unsafe { &mut *dict_ptr(self_in) };
    ensure_not_fixed(self_);
    map::clear(&mut self_.map);
    obj::CONST_NONE
}

pub fn dict_pop(n_args: usize, args: &[Obj]) -> Obj {
    dict_get_helper(n_args, args, LookupKind::RemoveIfFound)
}

pub fn dict_setdefault(n_args: usize, args: &[Obj]) -> Obj {
    dict_get_helper(n_args, args, LookupKind::AddIfNotFound)
}

pub fn dict_popitem(self_in: Obj) -> Obj {
    check_self(self_in);
    let self_ = unsafe { &mut *dict_ptr(self_in) };
    ensure_not_fixed(self_);
    if self_.map.used == 0 {
        raise::raise(MpRaise::RuntimeError("popitem(): dictionary is empty"));
    }
    let mut cur = 0;
    if mpconfig::PY_COLLECTIONS_ORDEREDDICT && self_.map.is_ordered {
        cur = self_.map.used.saturating_sub(1);
    }
    let (key, value) = {
        let next = dict_iter_next(self_, &mut cur).expect("dict popitem elem");
        (next.key, next.value)
    };
    self_.map.used -= 1;
    if let Some(slot) = map::lookup(&mut self_.map, key, LookupKind::Lookup) {
        slot.key = OBJ_SENTINEL;
        slot.value = OBJ_NULL;
    }
    objtuple::new_tuple(2, Some(&[key, value]))
}

pub fn dict_update(n_args: usize, args: &[Obj], kwargs: &Map) -> Obj {
    check_self(args[0]);
    let self_ = unsafe { &mut *dict_ptr(args[0]) };
    ensure_not_fixed(self_);
    argcheck::check_num(n_args, kwargs.used, 1, 2, true);

    if n_args == 2 {
        if is_dict_or_ordereddict(args[1]) {
            if args[1] != args[0] {
                let other = unsafe { &*dict_ptr(args[1]) };
                let mut cur = 0;
                while let Some(elem) = dict_iter_next(other, &mut cur) {
                    if let Some(slot) =
                        map::lookup(&mut self_.map, elem.key, LookupKind::AddIfNotFound)
                    {
                        slot.value = elem.value;
                    }
                }
            }
        } else {
            let iter = runtime::getiter(args[1], None);
            loop {
                let next = runtime::iternext(iter);
                if next == obj::OBJ_STOP_ITERATION {
                    break;
                }
                let inner = runtime::getiter(next, None);
                let key = runtime::iternext(inner);
                let value = runtime::iternext(inner);
                let stop = runtime::iternext(inner);
                if key == obj::OBJ_STOP_ITERATION
                    || value == obj::OBJ_STOP_ITERATION
                    || stop != obj::OBJ_STOP_ITERATION
                {
                    raise::raise(MpRaise::ValueError("dict update sequence has wrong length"));
                }
                if let Some(slot) = map::lookup(&mut self_.map, key, LookupKind::AddIfNotFound) {
                    slot.value = value;
                }
            }
        }
    }

    for i in 0..kwargs.alloc {
        if map::slot_is_filled(kwargs, i) {
            let key = kwargs.table[i].key;
            let val = kwargs.table[i].value;
            if let Some(slot) = map::lookup(&mut self_.map, key, LookupKind::AddIfNotFound) {
                slot.value = val;
            }
        }
    }
    obj::CONST_NONE
}

pub fn dict_fromkeys(n_args: usize, args: &[Obj]) -> Obj {
    let iter = runtime::getiter(args[1], None);
    let mut value = obj::CONST_NONE;
    if n_args > 2 {
        value = args[2];
    }
    let self_out = if let Some(len) = obj::len_maybe(args[1]) {
        new_dict(obj::small_int_value(len) as usize)
    } else {
        new_dict(0)
    };
    let self_ = unsafe { &mut *dict_ptr(self_out) };
    loop {
        let next = runtime::iternext(iter);
        if next == obj::OBJ_STOP_ITERATION {
            break;
        }
        if let Some(slot) = map::lookup(&mut self_.map, next, LookupKind::AddIfNotFound) {
            slot.value = value;
        }
    }
    self_out
}

fn new_dict_view(dict: Obj, kind: DictViewKind) -> Obj {
    let o = malloc::new_obj::<ObjDictView>().expect("dict view");
    unsafe {
        (*o).base.type_ = &TYPE_DICT_VIEW as *const ObjType;
        (*o).dict = dict;
        (*o).kind = kind;
        obj::from_ptr(o as *const ObjDictView as *const ())
    }
}

pub fn dict_items(self_in: Obj) -> Obj {
    check_self(self_in);
    new_dict_view(self_in, DictViewKind::Items)
}

pub fn dict_keys(self_in: Obj) -> Obj {
    check_self(self_in);
    new_dict_view(self_in, DictViewKind::Keys)
}

pub fn dict_values(self_in: Obj) -> Obj {
    check_self(self_in);
    new_dict_view(self_in, DictViewKind::Values)
}

fn dict_view_it_iternext(self_in: Obj) -> Obj {
    let self_ = unsafe { &mut *(obj::as_ptr(self_in) as *mut ObjDictViewIter) };
    let dict = unsafe { &*dict_ptr(self_.dict) };
    if let Some(next) = dict_iter_next(dict, &mut self_.cur) {
        match self_.kind {
            DictViewKind::Items => {
                let items = [next.key, next.value];
                objtuple::new_tuple(2, Some(&items))
            }
            DictViewKind::Keys => next.key,
            DictViewKind::Values => next.value,
        }
    } else {
        obj::OBJ_STOP_ITERATION
    }
}

fn dict_view_getiter(view_in: Obj, iter_buf: *mut ObjIterBuf) -> Obj {
    debug_assert!(size_of::<ObjDictViewIter>() <= size_of::<ObjIterBuf>());
    let view = unsafe { &*(obj::as_ptr(view_in) as *const ObjDictView) };
    let o = unsafe { &mut *(iter_buf as *mut ObjDictViewIter) };
    o.base.type_ = &TYPE_DICT_VIEW_IT as *const ObjType;
    o.kind = view.kind;
    o.dict = view.dict;
    o.cur = 0;
    obj::from_ptr(iter_buf as *const ObjDictViewIter as *const ())
}

fn dict_view_print(print: &Print, self_in: Obj, _kind: PrintKind) {
    let self_ = unsafe { &*(obj::as_ptr(self_in) as *const ObjDictView) };
    let mut first = true;
    mpprint::print_str(print, DICT_VIEW_NAMES[self_.kind as usize]);
    mpprint::print_str(print, "([");
    let mut iter_buf = obj::ObjIterBuf {
        base: ObjBase {
            type_: core::ptr::null(),
        },
        buf: [obj::OBJ_NULL; 3],
    };
    let self_iter = dict_view_getiter(self_in, &mut iter_buf as *mut ObjIterBuf);
    loop {
        let next = dict_view_it_iternext(self_iter);
        if next == obj::OBJ_STOP_ITERATION {
            break;
        }
        if !first {
            mpprint::print_str(print, ", ");
        }
        first = false;
        obj::print_helper(print, next, PrintKind::Repr);
    }
    mpprint::print_str(print, "])");
}

fn dict_view_unary_op(op: UnaryOp, o_in: Obj) -> Obj {
    let o = unsafe { &*(obj::as_ptr(o_in) as *const ObjDictView) };
    if op == UnaryOp::Hash && o.kind == DictViewKind::Values {
        return obj::new_small_int(o_in.0 as obj::Int);
    }
    dict_unary_op(op, o.dict)
}

fn dict_view_binary_op(op: BinaryOp, lhs_in: Obj, rhs_in: Obj) -> Obj {
    let o = unsafe { &*(obj::as_ptr(lhs_in) as *const ObjDictView) };
    if o.kind != DictViewKind::Keys || op != BinaryOp::Contains {
        return obj::OBJ_NULL;
    }
    dict_binary_op(op, o.dict, rhs_in)
}

pub fn dict_getiter(self_in: Obj, iter_buf: *mut ObjIterBuf) -> Obj {
    debug_assert!(size_of::<ObjDictViewIter>() <= size_of::<ObjIterBuf>());
    check_self(self_in);
    let o = unsafe { &mut *(iter_buf as *mut ObjDictViewIter) };
    o.base.type_ = &TYPE_DICT_VIEW_IT as *const ObjType;
    o.kind = DictViewKind::Keys;
    o.dict = self_in;
    o.cur = 0;
    obj::from_ptr(iter_buf as *const ObjDictViewIter as *const ())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gc;

    fn setup() {
        let _ = gc::init();
        qstr::init();
    }

    #[test]
    fn store_and_get() {
        setup();
        let d = new_dict(0);
        let k = obj::new_qstr(qstr::from_str("a"));
        dict_store(d, k, obj::new_small_int(42));
        let v = dict_get(d, k);
        assert_eq!(obj::small_int_value(v), 42);
    }

    #[test]
    fn copy_is_independent() {
        setup();
        let d = new_dict(0);
        let k = obj::new_qstr(qstr::from_str("x"));
        dict_store(d, k, obj::new_small_int(1));
        let c = dict_copy(d);
        dict_store(c, k, obj::new_small_int(2));
        assert_eq!(obj::small_int_value(dict_get(d, k)), 1);
        assert_eq!(obj::small_int_value(dict_get(c, k)), 2);
    }

    #[test]
    fn contains_and_len() {
        setup();
        let d = new_dict(0);
        let k = obj::new_qstr(qstr::from_str("k"));
        dict_store(d, k, obj::CONST_NONE);
        assert!(obj::is_true(dict_binary_op(BinaryOp::Contains, d, k)));
        assert_eq!(obj::small_int_value(dict_unary_op(UnaryOp::Len, d)), 1);
    }
}
