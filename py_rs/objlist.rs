//! rewrite of py/objlist.c + py/objlist.h
// symmetry: done

use core::mem::size_of;

use crate::argcheck;
use crate::cstack;
use crate::map::{self, MapElem};
use crate::malloc;
use crate::mpconfig;
use crate::mpprint::{self, Print, PrintKind};
use crate::obj::{
    self, IterNextFn, Obj, ObjBase, ObjIterBuf, ObjType, OBJ_SENTINEL, TYPE_FLAG_BINDS_SELF,
    TYPE_FLAG_BUILTIN_FUN,
};
use crate::objdict::{self, ObjDict};
use crate::objpolyiter;
use crate::objslice;
use crate::qstr;
use crate::raise::{self, MpRaise};
use crate::runtime;
use crate::runtime0::{BinaryOp, UnaryOp};
use crate::sequence;

const LIST_MIN_ALLOC: usize = 4;

#[repr(C)]
pub struct ObjList {
    pub base: ObjBase,
    pub alloc: usize,
    pub len: usize,
    pub items: *mut Obj,
}

// --- minimal builtin method wrappers (MP_DEFINE_CONST_FUN_OBJ_*) ------------

type BuiltinFn1 = fn(Obj) -> Obj;
type BuiltinFn2 = fn(Obj, Obj) -> Obj;
type BuiltinFn3 = fn(Obj, Obj, Obj) -> Obj;
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
struct ObjFunBuiltin3 {
    base: ObjBase,
    fun: BuiltinFn3,
}

#[repr(C)]
struct ObjFunBuiltinVar {
    base: ObjBase,
    min_args: u8,
    max_args: u8,
    fun: BuiltinFnVar,
}

static mut FUN_BUILTIN_1_SLOTS: [*const (); 1] = [fun_builtin_1_call as *const ()];
static mut FUN_BUILTIN_2_SLOTS: [*const (); 1] = [fun_builtin_2_call as *const ()];
static mut FUN_BUILTIN_3_SLOTS: [*const (); 1] = [fun_builtin_3_call as *const ()];
static mut FUN_BUILTIN_VAR_SLOTS: [*const (); 1] = [fun_builtin_var_call as *const ()];

static TYPE_FUN_BUILTIN_1: ObjType = ObjType {
    base: ObjBase { type_: core::ptr::null() },
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

static TYPE_FUN_BUILTIN_2: ObjType = ObjType {
    base: ObjBase { type_: core::ptr::null() },
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
    slots: unsafe { FUN_BUILTIN_2_SLOTS.as_ptr() },
};

static TYPE_FUN_BUILTIN_3: ObjType = ObjType {
    base: ObjBase { type_: core::ptr::null() },
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
    slots: unsafe { FUN_BUILTIN_3_SLOTS.as_ptr() },
};

static TYPE_FUN_BUILTIN_VAR: ObjType = ObjType {
    base: ObjBase { type_: core::ptr::null() },
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

fn fun_builtin_1_call(self_in: Obj, n_args: usize, n_kw: usize, args: &[Obj]) -> Obj {
    argcheck::check_num(n_args, n_kw, 1, 1, false);
    let self_ = unsafe { &*(obj::as_ptr(self_in) as *const ObjFunBuiltin1) };
    (self_.fun)(args[0])
}

fn fun_builtin_2_call(self_in: Obj, n_args: usize, n_kw: usize, args: &[Obj]) -> Obj {
    argcheck::check_num(n_args, n_kw, 2, 2, false);
    let self_ = unsafe { &*(obj::as_ptr(self_in) as *const ObjFunBuiltin2) };
    (self_.fun)(args[0], args[1])
}

fn fun_builtin_3_call(self_in: Obj, n_args: usize, n_kw: usize, args: &[Obj]) -> Obj {
    argcheck::check_num(n_args, n_kw, 3, 3, false);
    let self_ = unsafe { &*(obj::as_ptr(self_in) as *const ObjFunBuiltin3) };
    (self_.fun)(args[0], args[1], args[2])
}

fn fun_builtin_var_call(self_in: Obj, n_args: usize, n_kw: usize, args: &[Obj]) -> Obj {
    let self_ = unsafe { &*(obj::as_ptr(self_in) as *const ObjFunBuiltinVar) };
    argcheck::check_num(n_args, n_kw, self_.min_args as usize, self_.max_args as usize, false);
    (self_.fun)(n_args, args)
}

fn new_fun_builtin_1(fun: BuiltinFn1) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltin1>().expect("fun_builtin_1 alloc");
    unsafe {
        (*o).base.type_ = &TYPE_FUN_BUILTIN_1 as *const ObjType;
        (*o).fun = fun;
        obj::from_ptr(o as *const ObjFunBuiltin1 as *const ())
    }
}

fn new_fun_builtin_2(fun: BuiltinFn2) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltin2>().expect("fun_builtin_2 alloc");
    unsafe {
        (*o).base.type_ = &TYPE_FUN_BUILTIN_2 as *const ObjType;
        (*o).fun = fun;
        obj::from_ptr(o as *const ObjFunBuiltin2 as *const ())
    }
}

fn new_fun_builtin_3(fun: BuiltinFn3) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltin3>().expect("fun_builtin_3 alloc");
    unsafe {
        (*o).base.type_ = &TYPE_FUN_BUILTIN_3 as *const ObjType;
        (*o).fun = fun;
        obj::from_ptr(o as *const ObjFunBuiltin3 as *const ())
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

// --- list iterator ------------------------------------------------------------

#[repr(C)]
struct ObjListIter {
    base: ObjBase,
    iternext: IterNextFn,
    list: Obj,
    cur: usize,
}

fn list_it_iternext(self_in: Obj) -> Obj {
    let self_ = unsafe { &mut *(obj::as_ptr(self_in) as *mut ObjListIter) };
    let list = unsafe { &*(obj::as_ptr(self_.list) as *const ObjList) };
    if self_.cur < list.len {
        let o_out = unsafe { *list.items.add(self_.cur) };
        self_.cur += 1;
        o_out
    } else {
        obj::OBJ_STOP_ITERATION
    }
}

// --- list type slots ----------------------------------------------------------

static mut LIST_SLOTS: [*const (); 7] = [
    list_make_new as *const (),
    list_print as *const (),
    list_unary_op as *const (),
    list_binary_op as *const (),
    list_subscr as *const (),
    list_getiter as *const (),
    core::ptr::null(),
];

static TYPE: ObjType = ObjType {
    base: ObjBase { type_: core::ptr::null() },
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
    slots: unsafe { LIST_SLOTS.as_ptr() },
};

static LIST_INIT: std::sync::OnceLock<()> = std::sync::OnceLock::new();

fn init_list_type() {
    LIST_INIT.get_or_init(|| {
        let table = vec![
            MapElem { key: obj::new_qstr(qstr::from_str("append")), value: new_fun_builtin_2(list_append) },
            MapElem { key: obj::new_qstr(qstr::from_str("clear")), value: new_fun_builtin_1(list_clear) },
            MapElem { key: obj::new_qstr(qstr::from_str("copy")), value: new_fun_builtin_1(list_copy) },
            MapElem { key: obj::new_qstr(qstr::from_str("count")), value: new_fun_builtin_2(list_count) },
            MapElem { key: obj::new_qstr(qstr::from_str("extend")), value: new_fun_builtin_2(list_extend) },
            MapElem { key: obj::new_qstr(qstr::from_str("index")), value: new_fun_builtin_var(2, 4, list_index) },
            MapElem { key: obj::new_qstr(qstr::from_str("insert")), value: new_fun_builtin_3(list_insert) },
            MapElem { key: obj::new_qstr(qstr::from_str("pop")), value: new_fun_builtin_var(1, 2, list_pop) },
            MapElem { key: obj::new_qstr(qstr::from_str("remove")), value: new_fun_builtin_2(list_remove) },
            MapElem { key: obj::new_qstr(qstr::from_str("reverse")), value: new_fun_builtin_1(list_reverse) },
            MapElem { key: obj::new_qstr(qstr::from_str("sort")), value: new_fun_builtin_var(1, 1, list_sort) },
        ];
        let ptr = obj::malloc_helper(size_of::<ObjDict>(), objdict::type_dict()) as *mut ObjDict;
        unsafe {
            map::init_fixed_table(&mut (*ptr).map, table);
            LIST_SLOTS[6] = obj::from_ptr(ptr as *const ObjDict as *const ()).0 as *const ();
        }
    });
}

pub fn type_list() -> &'static ObjType {
    init_list_type();
    &TYPE
}

// --- helpers ------------------------------------------------------------------

fn check_self(self_in: Obj) {
    if !obj::is_exact_type(self_in, type_list()) {
        raise::raise(MpRaise::TypeError("list method on non-list"));
    }
}

fn list_ptr(self_in: Obj) -> *mut ObjList {
    obj::as_ptr(self_in) as *mut ObjList
}

fn seq_clear(items: *mut Obj, len: usize, alloc: usize) {
    unsafe {
        for i in len..alloc {
            *items.add(i) = obj::OBJ_NULL;
        }
    }
}

fn seq_copy(dest: *mut Obj, src: *const Obj, len: usize) {
    unsafe {
        std::ptr::copy_nonoverlapping(src, dest, len);
    }
}

fn seq_cat(dest: *mut Obj, src1: *const Obj, len1: usize, src2: *const Obj, len2: usize) {
    unsafe {
        std::ptr::copy_nonoverlapping(src1, dest, len1);
        std::ptr::copy_nonoverlapping(src2, dest.add(len1), len2);
    }
}

fn seq_replace_no_grow(
    dest: *mut Obj,
    dest_len: usize,
    beg: usize,
    end: usize,
    slice: *const Obj,
    slice_len: usize,
) {
    unsafe {
        std::ptr::copy(slice, dest.add(beg), slice_len);
        std::ptr::copy(
            dest.add(end),
            dest.add(beg + slice_len),
            dest_len - end,
        );
    }
}

fn seq_replace_grow_inplace(
    dest: *mut Obj,
    dest_len: usize,
    beg: usize,
    end: usize,
    slice: *const Obj,
    slice_len: usize,
    len_adj: isize,
) {
    unsafe {
        std::ptr::copy(
            dest.add(end),
            dest.add(beg + slice_len),
            (dest_len as isize + len_adj - (beg as isize + slice_len as isize)) as usize,
        );
        std::ptr::copy(slice, dest.add(beg), slice_len);
    }
}

fn list_new(n: usize) -> *mut ObjList {
    let o = malloc::new_obj::<ObjList>().expect("objlist alloc");
    list_init(o, n);
    o
}

/// `mp_obj_list_init`
pub fn list_init(o: *mut ObjList, n: usize) {
    unsafe {
        (*o).base.type_ = type_list() as *const ObjType;
        (*o).alloc = if n < LIST_MIN_ALLOC { LIST_MIN_ALLOC } else { n };
        (*o).len = n;
        (*o).items = malloc::new::<Obj>((*o).alloc).expect("list items alloc");
        seq_clear((*o).items, n, (*o).alloc);
    }
}

/// `mp_obj_new_list`
pub fn new_list(n: usize, items: Option<&[Obj]>) -> Obj {
    let o = list_new(n);
    if let Some(items) = items {
        unsafe {
            seq_copy((*o).items, items.as_ptr(), n);
        }
    }
    obj::from_ptr(o as *const ObjList as *const ())
}

fn list_extend_from_iter(list: Obj, iterable: Obj) -> Obj {
    let iter = runtime::getiter(iterable, None);
    loop {
        let item = runtime::iternext(iter);
        if item == obj::OBJ_STOP_ITERATION {
            break;
        }
        list_append(list, item);
    }
    list
}

// --- type methods -------------------------------------------------------------

pub fn list_print(print: &Print, o_in: Obj, kind: PrintKind) {
    let o = unsafe { &*(obj::as_ptr(o_in) as *const ObjList) };
    let mut kind = kind;
    let item_separator = if mpconfig::PY_JSON && kind == PrintKind::Json {
        mpprint::json_item_separator(print)
    } else {
        ", "
    };
    if !(mpconfig::PY_JSON && kind == PrintKind::Json) {
        kind = PrintKind::Repr;
    }
    mpprint::print_str(print, "[");
    for i in 0..o.len {
        if i > 0 {
            mpprint::print_str(print, item_separator);
        }
        obj::print_helper(print, unsafe { *o.items.add(i) }, kind);
    }
    mpprint::print_str(print, "]");
}

pub fn list_make_new(_type_in: &ObjType, n_args: usize, n_kw: usize, args: &[Obj]) -> Obj {
    argcheck::check_num(n_args, n_kw, 0, 1, false);
    match n_args {
        0 => new_list(0, None),
        _ => {
            let list = new_list(0, None);
            list_extend_from_iter(list, args[0])
        }
    }
}

pub fn list_unary_op(op: UnaryOp, self_in: Obj) -> Obj {
    let self_ = unsafe { &*(list_ptr(self_in) as *const ObjList) };
    match op {
        UnaryOp::Bool => obj::new_bool(self_.len != 0),
        UnaryOp::Len => obj::new_small_int(self_.len as obj::Int),
        UnaryOp::Sizeof if mpconfig::PY_SYS_GETSIZEOF => {
            let sz = size_of::<ObjList>() + size_of::<Obj>() * self_.alloc;
            obj::new_small_int(sz as obj::Int)
        }
        _ => obj::OBJ_NULL,
    }
}

pub fn list_binary_op(op: BinaryOp, lhs: Obj, rhs: Obj) -> Obj {
    let o = unsafe { &*(list_ptr(lhs) as *const ObjList) };
    match op {
        BinaryOp::Add => {
            if !obj::is_exact_type(rhs, type_list()) {
                return obj::OBJ_NULL;
            }
            let p = unsafe { &*(list_ptr(rhs) as *const ObjList) };
            let s = list_new(o.len + p.len);
            unsafe {
                seq_cat((*s).items, o.items, o.len, p.items, p.len);
            }
            obj::from_ptr(s as *const ObjList as *const ())
        }
        BinaryOp::InplaceAdd => {
            list_extend(lhs, rhs);
            lhs
        }
        BinaryOp::Multiply => {
            let mut n = 0;
            if !obj::get_int_maybe(rhs, &mut n) {
                return obj::OBJ_NULL;
            }
            let n = if n < 0 { 0 } else { n as usize };
            let s = list_new(o.len * n);
            unsafe {
                sequence::multiply(
                    std::slice::from_raw_parts(o.items as *const u8, o.len * size_of::<Obj>()),
                    size_of::<Obj>(),
                    o.len,
                    n,
                    std::slice::from_raw_parts_mut((*s).items as *mut u8, (*s).len * size_of::<Obj>()),
                );
            }
            obj::from_ptr(s as *const ObjList as *const ())
        }
        BinaryOp::Equal
        | BinaryOp::Less
        | BinaryOp::LessEqual
        | BinaryOp::More
        | BinaryOp::MoreEqual => {
            if !obj::is_exact_type(rhs, type_list()) {
                if op == BinaryOp::Equal {
                    return obj::CONST_FALSE;
                }
                return obj::OBJ_NULL;
            }
            let another = unsafe { &*(list_ptr(rhs) as *const ObjList) };
            let items1 = unsafe { std::slice::from_raw_parts(o.items, o.len) };
            let items2 = unsafe { std::slice::from_raw_parts(another.items, another.len) };
            let res = sequence::cmp_objs(op, items1, items2);
            obj::new_bool(res)
        }
        _ => obj::OBJ_NULL,
    }
}

pub fn list_subscr(self_in: Obj, index: Obj, value: Obj) -> Obj {
    if mpconfig::PY_BUILTINS_SLICE && obj::is_exact_type(index, objslice::type_slice()) {
        let self_ = unsafe { &mut *list_ptr(self_in) };
        let mut slice = objslice::BoundSlice { start: 0, stop: 0, step: 1 };
        let fast = sequence::get_fast_slice_indexes(self_.len, index, &mut slice);
        if value == OBJ_SENTINEL {
            if !fast {
                let items = unsafe { std::slice::from_raw_parts(self_.items, self_.len) };
                return sequence::extract_slice(items, &slice);
            }
            let res = list_new((slice.stop - slice.start) as usize);
            unsafe {
                seq_copy(
                    (*res).items,
                    self_.items.add(slice.start as usize),
                    (slice.stop - slice.start) as usize,
                );
            }
            return obj::from_ptr(res as *const ObjList as *const ());
        }
        let mut value = value;
        if value == obj::OBJ_NULL {
            value = new_list(0, None);
        }
        if !fast {
            raise::raise(MpRaise::RuntimeError("list slice assign with step"));
        }
        let (value_len, value_items) = obj::get_array(value);
        let len_adj = value_len as isize - (slice.stop - slice.start) as isize;
        unsafe {
            if len_adj > 0 {
                if self_.len as isize + len_adj > self_.alloc as isize {
                    self_.items = malloc::renew(self_.items, self_.alloc, self_.len + len_adj as usize)
                        .expect("list grow");
                    self_.alloc = self_.len + len_adj as usize;
                }
                seq_replace_grow_inplace(
                    self_.items,
                    self_.len,
                    slice.start as usize,
                    slice.stop as usize,
                    value_items.as_ptr(),
                    value_len,
                    len_adj,
                );
            } else {
                seq_replace_no_grow(
                    self_.items,
                    self_.len,
                    slice.start as usize,
                    slice.stop as usize,
                    value_items.as_ptr(),
                    value_len,
                );
                seq_clear(
                    self_.items,
                    (self_.len as isize + len_adj) as usize,
                    self_.len,
                );
            }
            self_.len = (self_.len as isize + len_adj) as usize;
        }
        return obj::CONST_NONE;
    }
    if value == obj::OBJ_NULL {
        let args = [self_in, index];
        list_pop(2, &args);
        return obj::CONST_NONE;
    } else if value == OBJ_SENTINEL {
        let self_ = unsafe { &*list_ptr(self_in) };
        let index_val = obj::get_index(unsafe { &*self_.base.type_ }, self_.len, index, false);
        unsafe { *self_.items.add(index_val) }
    } else {
        list_store(self_in, index, value);
        obj::CONST_NONE
    }
}

pub fn list_getiter(o_in: Obj, iter_buf: *mut ObjIterBuf) -> Obj {
    debug_assert!(size_of::<ObjListIter>() <= size_of::<ObjIterBuf>());
    let o = unsafe { &mut *(iter_buf as *mut ObjListIter) };
    o.base.type_ = objpolyiter::type_polymorph_iter() as *const ObjType;
    o.iternext = list_it_iternext;
    o.list = o_in;
    o.cur = 0;
    obj::from_ptr(iter_buf as *const ObjListIter as *const ())
}

// --- list methods -------------------------------------------------------------

/// `mp_obj_list_append`
pub fn list_append(self_in: Obj, arg: Obj) -> Obj {
    check_self(self_in);
    let self_ = unsafe { &mut *list_ptr(self_in) };
    if self_.len >= self_.alloc {
        self_.items = malloc::renew(self_.items, self_.alloc, self_.alloc * 2)
            .expect("list grow");
        self_.alloc *= 2;
        seq_clear(self_.items, self_.len + 1, self_.alloc);
    }
    unsafe {
        *self_.items.add(self_.len) = arg;
    }
    self_.len += 1;
    obj::CONST_NONE
}

pub fn list_extend(self_in: Obj, arg_in: Obj) -> Obj {
    check_self(self_in);
    if obj::is_exact_type(arg_in, type_list()) {
        let self_ = unsafe { &mut *list_ptr(self_in) };
        let arg = unsafe { &*(list_ptr(arg_in) as *const ObjList) };
        if self_.len + arg.len > self_.alloc {
            self_.items = malloc::renew(self_.items, self_.alloc, self_.len + arg.len + 4)
                .expect("list grow");
            self_.alloc = self_.len + arg.len + 4;
            seq_clear(self_.items, self_.len + arg.len, self_.alloc);
        }
        unsafe {
            std::ptr::copy_nonoverlapping(arg.items, self_.items.add(self_.len), arg.len);
        }
        self_.len += arg.len;
    } else {
        list_extend_from_iter(self_in, arg_in);
    }
    obj::CONST_NONE
}

pub fn list_pop(n_args: usize, args: &[Obj]) -> Obj {
    check_self(args[0]);
    let self_ = unsafe { &mut *list_ptr(args[0]) };
    if self_.len == 0 {
        raise::raise(MpRaise::ValueError("pop from empty list"));
    }
    let index = if n_args == 1 {
        obj::get_index(unsafe { &*self_.base.type_ }, self_.len, obj::new_small_int(-1), false)
    } else {
        obj::get_index(unsafe { &*self_.base.type_ }, self_.len, args[1], false)
    };
    let ret = unsafe { *self_.items.add(index) };
    self_.len -= 1;
    unsafe {
        std::ptr::copy(
            self_.items.add(index + 1),
            self_.items.add(index),
            self_.len - index,
        );
        *self_.items.add(self_.len) = obj::OBJ_NULL;
    }
    if self_.alloc > LIST_MIN_ALLOC && self_.alloc > 2 * self_.len {
        self_.items = malloc::renew(self_.items, self_.alloc, self_.alloc / 2)
            .expect("list shrink");
        self_.alloc /= 2;
    }
    ret
}

fn quicksort(
    mut head: *mut Obj,
    mut tail: *mut Obj,
    key_fn: Obj,
    binop_less_result: Obj,
) {
    cstack::check();
    unsafe {
        while tail.offset_from(head) > 1 {
            let mut h = head;
            let mut t = tail;
            let v = if key_fn == obj::OBJ_NULL {
                *tail
            } else {
                runtime::call_function_1(key_fn, *tail)
            };
            loop {
                loop {
                    h = h.add(1);
                    if h >= t {
                        break;
                    }
                    let hv = if key_fn == obj::OBJ_NULL {
                        *h
                    } else {
                        runtime::call_function_1(key_fn, *h)
                    };
                    if runtime::binary_op_obj(BinaryOp::Less, hv, v) != binop_less_result {
                        break;
                    }
                }
                loop {
                    t = t.sub(1);
                    if h >= t {
                        break;
                    }
                    let tv = if key_fn == obj::OBJ_NULL {
                        *t
                    } else {
                        runtime::call_function_1(key_fn, *t)
                    };
                    if runtime::binary_op_obj(BinaryOp::Less, v, tv) != binop_less_result {
                        break;
                    }
                }
                if h >= t {
                    break;
                }
                std::ptr::swap(h, t);
            }
            let x = *h;
            *h = *tail;
            *tail = x;
            if t.offset_from(head) < tail.offset_from(h) {
                quicksort(head, t, key_fn, binop_less_result);
                head = h;
            } else {
                quicksort(h, tail, key_fn, binop_less_result);
                tail = t;
            }
        }
    }
}

pub fn list_sort(n_args: usize, args: &[Obj]) -> Obj {
    check_self(args[0]);
    let mut key = obj::CONST_NONE;
    let mut reverse = false;
    if n_args > 1 {
        // positional extras not supported; kw-only key/reverse handled below via trailing pairs
        for i in (1..n_args).step_by(2) {
            if i + 1 >= n_args {
                break;
            }
            let k = args[i];
            let v = args[i + 1];
            if obj::is_qstr(k) {
                let name = qstr::str_from_qstr(obj::qstr_value(k)).unwrap_or_default();
                if name == "key" {
                    key = v;
                } else if name == "reverse" {
                    reverse = obj::is_true(v);
                }
            }
        }
    }
    let self_ = unsafe { &mut *list_ptr(args[0]) };
    if self_.len > 1 {
        let key_fn = if key == obj::CONST_NONE { obj::OBJ_NULL } else { key };
        let less_result = if reverse { obj::CONST_FALSE } else { obj::CONST_TRUE };
        unsafe {
            quicksort(
                self_.items.offset(-1),
                self_.items.add(self_.len - 1),
                key_fn,
                less_result,
            );
        }
    }
    obj::CONST_NONE
}

pub fn list_clear(self_in: Obj) -> Obj {
    check_self(self_in);
    let self_ = unsafe { &mut *list_ptr(self_in) };
    self_.len = 0;
    self_.items = malloc::renew(self_.items, self_.alloc, LIST_MIN_ALLOC).expect("list shrink");
    self_.alloc = LIST_MIN_ALLOC;
    seq_clear(self_.items, 0, self_.alloc);
    obj::CONST_NONE
}

pub fn list_copy(self_in: Obj) -> Obj {
    check_self(self_in);
    let self_ = unsafe { &*list_ptr(self_in) };
    let items = unsafe { std::slice::from_raw_parts(self_.items, self_.len) };
    new_list(self_.len, Some(items))
}

pub fn list_count(self_in: Obj, value: Obj) -> Obj {
    check_self(self_in);
    let self_ = unsafe { &*list_ptr(self_in) };
    let items = unsafe { std::slice::from_raw_parts(self_.items, self_.len) };
    sequence::count_obj(items, self_.len, value)
}

pub fn list_index(n_args: usize, args: &[Obj]) -> Obj {
    check_self(args[0]);
    let self_ = unsafe { &*list_ptr(args[0]) };
    let items = unsafe { std::slice::from_raw_parts(self_.items, self_.len) };
    sequence::index_obj(items, self_.len, n_args, args)
}

pub fn list_insert(self_in: Obj, idx: Obj, val: Obj) -> Obj {
    check_self(self_in);
    let self_ = unsafe { &mut *list_ptr(self_in) };
    let mut index = obj::get_int(idx);
    if index < 0 {
        index += self_.len as obj::Int;
    }
    if index < 0 {
        index = 0;
    }
    if index as usize > self_.len {
        index = self_.len as obj::Int;
    }
    list_append(self_in, obj::CONST_NONE);
    unsafe {
        for i in (index as usize + 1..=self_.len - 1).rev() {
            *self_.items.add(i) = *self_.items.add(i - 1);
        }
        *self_.items.add(index as usize) = val;
    }
    obj::CONST_NONE
}

/// `mp_obj_list_remove`
pub fn list_remove(self_in: Obj, value: Obj) -> Obj {
    check_self(self_in);
    let mut args = [self_in, value];
    args[1] = list_index(2, &args);
    list_pop(2, &args);
    obj::CONST_NONE
}

pub fn list_reverse(self_in: Obj) -> Obj {
    check_self(self_in);
    let self_ = unsafe { &mut *list_ptr(self_in) };
    let len = self_.len;
    for i in 0..len / 2 {
        unsafe {
            let a = *self_.items.add(i);
            *self_.items.add(i) = *self_.items.add(len - i - 1);
            *self_.items.add(len - i - 1) = a;
        }
    }
    obj::CONST_NONE
}

/// `mp_obj_list_store`
pub fn list_store(self_in: Obj, index: Obj, value: Obj) {
    check_self(self_in);
    let self_ = unsafe { &mut *list_ptr(self_in) };
    let i = obj::get_index(unsafe { &*self_.base.type_ }, self_.len, index, false);
    unsafe {
        *self_.items.add(i) = value;
    }
}

/// `mp_obj_list_get`
pub fn list_get(o: Obj) -> (usize, Vec<Obj>) {
    unsafe {
        let l = &*(list_ptr(o) as *const ObjList);
        let items = std::slice::from_raw_parts(l.items, l.len).to_vec();
        (l.len, items)
    }
}

/// `mp_obj_list_set_len`
pub fn list_set_len(self_in: Obj, len: usize) {
    let self_ = unsafe { &mut *list_ptr(self_in) };
    self_.len = len;
}

/// `mp_obj_list_optional_arg`
pub fn list_optional_arg(arg_in: Obj, min_len: usize) -> *mut ObjList {
    if arg_in == obj::OBJ_NULL || arg_in == obj::CONST_NONE {
        list_new(min_len)
    } else {
        list_ensure(arg_in, min_len)
    }
}

/// `mp_obj_list_ensure`
pub fn list_ensure(in_: Obj, min_len: usize) -> *mut ObjList {
    if !obj::is_exact_type(in_, type_list()) {
        raise::raise(MpRaise::TypeError("expected list"));
    }
    let list = list_ptr(in_);
    if unsafe { (*list).len } < min_len {
        raise::raise(MpRaise::ValueError("list too short"));
    }
    list
}

/// `mp_obj_new_list_iterator`
pub fn new_list_iterator(list: Obj, cur: usize, iter_buf: *mut ObjIterBuf) -> Obj {
    debug_assert!(size_of::<ObjListIter>() <= size_of::<ObjIterBuf>());
    let o = unsafe { &mut *(iter_buf as *mut ObjListIter) };
    o.base.type_ = objpolyiter::type_polymorph_iter() as *const ObjType;
    o.iternext = list_it_iternext;
    o.list = list;
    o.cur = cur;
    obj::from_ptr(iter_buf as *const ObjListIter as *const ())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gc;

    fn setup() {
        let _ = gc::init();
    }

    #[test]
    fn append_and_get() {
        setup();
        let l = new_list(0, None);
        list_append(l, obj::new_small_int(1));
        list_append(l, obj::new_small_int(2));
        let (len, items) = list_get(l);
        assert_eq!(len, 2);
        assert_eq!(obj::small_int_value(items[0]), 1);
        assert_eq!(obj::small_int_value(items[1]), 2);
    }

    #[test]
    fn add_concat() {
        setup();
        let a = new_list(1, Some(&[obj::new_small_int(1)]));
        let b = new_list(1, Some(&[obj::new_small_int(2)]));
        let c = list_binary_op(BinaryOp::Add, a, b);
        let (len, items) = list_get(c);
        assert_eq!(len, 2);
        assert_eq!(obj::small_int_value(items[1]), 2);
    }

    #[test]
    fn pop_default() {
        setup();
        let l = new_list(2, Some(&[obj::new_small_int(10), obj::new_small_int(20)]));
        let v = list_pop(1, &[l]);
        assert_eq!(obj::small_int_value(v), 20);
    }
}
