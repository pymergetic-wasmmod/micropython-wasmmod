//! rewrite of py/objtuple.c + py/objtuple.h
// symmetry: done

use core::mem::size_of;

use crate::argcheck;
use crate::map::{self, MapElem};
use crate::mpconfig;
use crate::mpprint::{self, Print, PrintKind};
use crate::obj::{
    self, IterNextFn, Obj, ObjBase, ObjIterBuf, ObjType, OBJ_SENTINEL, TYPE_FLAG_BINDS_SELF,
    TYPE_FLAG_BUILTIN_FUN,
};
use crate::objdict::{self, ObjDict};
use crate::objpolyiter;
use crate::objslice;
use crate::objtype;
use crate::qstr;
use crate::raise::{self, MpRaise};
use crate::runtime;
use crate::runtime0::{BinaryOp, UnaryOp};
use crate::sequence;

#[repr(C)]
pub struct ObjTuple {
    pub base: ObjBase,
    pub len: usize,
}

// --- builtin method wrappers --------------------------------------------------

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

static mut FUN_BUILTIN_1_SLOTS: [*const (); 1] = [fun_builtin_1_call as *const ()];
static mut FUN_BUILTIN_2_SLOTS: [*const (); 1] = [fun_builtin_2_call as *const ()];
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

fn fun_builtin_var_call(self_in: Obj, n_args: usize, n_kw: usize, args: &[Obj]) -> Obj {
    let self_ = unsafe { &*(obj::as_ptr(self_in) as *const ObjFunBuiltinVar) };
    argcheck::check_num(n_args, n_kw, self_.min_args as usize, self_.max_args as usize, false);
    (self_.fun)(n_args, args)
}

fn new_fun_builtin_1(fun: BuiltinFn1) -> Obj {
    let o = crate::malloc::new_obj::<ObjFunBuiltin1>().expect("fun_builtin_1 alloc");
    unsafe {
        (*o).base.type_ = &TYPE_FUN_BUILTIN_1 as *const ObjType;
        (*o).fun = fun;
        obj::from_ptr(o as *const ObjFunBuiltin1 as *const ())
    }
}

fn new_fun_builtin_2(fun: BuiltinFn2) -> Obj {
    let o = crate::malloc::new_obj::<ObjFunBuiltin2>().expect("fun_builtin_2 alloc");
    unsafe {
        (*o).base.type_ = &TYPE_FUN_BUILTIN_2 as *const ObjType;
        (*o).fun = fun;
        obj::from_ptr(o as *const ObjFunBuiltin2 as *const ())
    }
}

fn new_fun_builtin_var(min_args: u8, max_args: u8, fun: BuiltinFnVar) -> Obj {
    let o = crate::malloc::new_obj::<ObjFunBuiltinVar>().expect("fun_builtin_var alloc");
    unsafe {
        (*o).base.type_ = &TYPE_FUN_BUILTIN_VAR as *const ObjType;
        (*o).min_args = min_args;
        (*o).max_args = max_args;
        (*o).fun = fun;
        obj::from_ptr(o as *const ObjFunBuiltinVar as *const ())
    }
}

// --- tuple iterator -----------------------------------------------------------

#[repr(C)]
struct ObjTupleIter {
    base: ObjBase,
    iternext: IterNextFn,
    tuple: Obj,
    cur: usize,
}

fn tuple_it_iternext(self_in: Obj) -> Obj {
    let self_ = unsafe { &mut *(obj::as_ptr(self_in) as *mut ObjTupleIter) };
    let tuple = unsafe { &*(obj::as_ptr(self_.tuple) as *const ObjTuple) };
    if self_.cur < tuple.len {
        let o_out = unsafe { *items_ptr(tuple as *const ObjTuple).add(self_.cur) };
        self_.cur += 1;
        o_out
    } else {
        obj::OBJ_STOP_ITERATION
    }
}

// --- type slots ---------------------------------------------------------------

static mut TUPLE_SLOTS: [*const (); 7] = [
    tuple_make_new as *const (),
    tuple_print as *const (),
    tuple_unary_op as *const (),
    tuple_binary_op as *const (),
    tuple_subscr as *const (),
    tuple_getiter as *const (),
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
    slots: unsafe { TUPLE_SLOTS.as_ptr() },
};

static TUPLE_INIT: std::sync::OnceLock<()> = std::sync::OnceLock::new();
static EMPTY_TUPLE: std::sync::OnceLock<Obj> = std::sync::OnceLock::new();

static mut EMPTY_TUPLE_STORAGE: ObjTuple = ObjTuple {
    base: ObjBase {
        type_: core::ptr::null(),
    },
    len: 0,
};

fn init_tuple_type() {
    TUPLE_INIT.get_or_init(|| {
        let table = vec![
            MapElem {
                key: obj::new_qstr(qstr::from_str("count")),
                value: new_fun_builtin_2(tuple_count),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("index")),
                value: new_fun_builtin_var(2, 4, tuple_index),
            },
        ];
        let ptr = obj::malloc_helper(size_of::<ObjDict>(), objdict::type_dict()) as *mut ObjDict;
        unsafe {
            map::init_fixed_table(&mut (*ptr).map, table);
            TUPLE_SLOTS[6] = obj::from_ptr(ptr as *const ObjDict as *const ()).0 as *const ();
            EMPTY_TUPLE_STORAGE.base.type_ = &raw const TYPE as *const ObjType;
            EMPTY_TUPLE_STORAGE.len = 0;
        }
        let _ = EMPTY_TUPLE.set(obj::from_ptr(
            &raw const EMPTY_TUPLE_STORAGE as *const ObjTuple as *const (),
        ));
    });
}

pub fn type_tuple() -> &'static ObjType {
    init_tuple_type();
    &TYPE
}

pub fn const_empty_tuple() -> Obj {
    init_tuple_type();
    *EMPTY_TUPLE.get().expect("empty tuple")
}

// --- helpers ------------------------------------------------------------------

fn items_ptr(o: *const ObjTuple) -> *const Obj {
    unsafe { (o as *const u8).add(size_of::<ObjTuple>()) as *const Obj }
}

fn items_ptr_mut(o: *mut ObjTuple) -> *mut Obj {
    unsafe { (o as *mut u8).add(size_of::<ObjTuple>()) as *mut Obj }
}

fn check_self(self_in: Obj) {
    if !obj::is_exact_type(self_in, type_tuple()) {
        raise::raise(MpRaise::TypeError("tuple method on non-tuple"));
    }
}

/// Type check via getiter slot (allows tuple, namedtuple, attrtuple).
pub fn is_tuple_compatible(o: Obj) -> bool {
    if !obj::is_obj(o) {
        return false;
    }
    let t = obj::get_type(o);
    if let Some(getiter_fn) = obj::type_get_iter(t) {
        getiter_fn as *const () == tuple_getiter as *const ()
    } else {
        false
    }
}

fn tuple_subclass_helper(obj_in: Obj) -> Option<*const ObjTuple> {
    if obj_in == obj::OBJ_NULL {
        return None;
    }
    let tuple_type = obj::get_type(obj_in);
    if obj::type_get_iter(tuple_type) != Some(tuple_getiter) {
        let native = objtype::cast_to_native_base(obj_in, obj::from_ptr(type_tuple() as *const ObjType as *const ()));
        if native == obj::OBJ_NULL {
            return None;
        }
        return Some(obj::as_ptr(native) as *const ObjTuple);
    }
    Some(obj::as_ptr(obj_in) as *const ObjTuple)
}

fn seq_cat(dest: *mut Obj, src1: *const Obj, len1: usize, src2: *const Obj, len2: usize) {
    unsafe {
        std::ptr::copy_nonoverlapping(src1, dest, len1);
        std::ptr::copy_nonoverlapping(src2, dest.add(len1), len2);
    }
}

fn seq_copy(dest: *mut Obj, src: *const Obj, len: usize) {
    unsafe {
        std::ptr::copy_nonoverlapping(src, dest, len);
    }
}

fn tuple_cmp_helper(op: BinaryOp, self_in: Obj, another_in: Obj) -> Obj {
    if !is_tuple_compatible(self_in) {
        raise::raise(MpRaise::TypeError("tuple method on non-tuple"));
    }
    let self_ = unsafe { &*(obj::as_ptr(self_in) as *const ObjTuple) };
    let another = match tuple_subclass_helper(another_in) {
        Some(p) => p,
        None => return obj::OBJ_NULL,
    };
    let self_items = unsafe { std::slice::from_raw_parts(items_ptr(self_), self_.len) };
    let other_items = unsafe { std::slice::from_raw_parts(items_ptr(another), (*another).len) };
    obj::new_bool(sequence::cmp_objs(op, self_items, other_items))
}

// --- public API ---------------------------------------------------------------

/// `mp_obj_new_tuple`
pub fn new_tuple(n: usize, items: Option<&[Obj]>) -> Obj {
    if n == 0 {
        return const_empty_tuple();
    }
    let extra = n * size_of::<Obj>();
    let base = obj::malloc_var::<ObjTuple>(extra, type_tuple());
    unsafe {
        (*base).len = n;
        if let Some(items) = items {
            std::ptr::copy_nonoverlapping(items.as_ptr(), items_ptr_mut(base), n);
        }
        obj::from_ptr(base as *const ObjTuple as *const ())
    }
}

pub fn tuple_get(o: Obj) -> (usize, Vec<Obj>) {
    unsafe {
        let t = &*(obj::as_ptr(o) as *const ObjTuple);
        let items = std::slice::from_raw_parts(items_ptr(t as *const ObjTuple), t.len).to_vec();
        (t.len, items)
    }
}

// --- type methods -------------------------------------------------------------

pub fn tuple_print(print: &Print, o_in: Obj, kind: PrintKind) {
    let o = unsafe { &*(obj::as_ptr(o_in) as *const ObjTuple) };
    let mut kind = kind;
    let item_separator = if mpconfig::PY_JSON && kind == PrintKind::Json {
        mpprint::json_item_separator(print)
    } else {
        ", "
    };
    if mpconfig::PY_JSON && kind == PrintKind::Json {
        mpprint::print_str(print, "[");
    } else {
        mpprint::print_str(print, "(");
        kind = PrintKind::Repr;
    }
    for i in 0..o.len {
        if i > 0 {
            mpprint::print_str(print, item_separator);
        }
        let item = unsafe { *items_ptr(o as *const ObjTuple).add(i) };
        obj::print_helper(print, item, kind);
    }
    if mpconfig::PY_JSON && kind == PrintKind::Json {
        mpprint::print_str(print, "]");
    } else {
        if o.len == 1 {
            mpprint::print_str(print, ",");
        }
        mpprint::print_str(print, ")");
    }
}

pub fn tuple_make_new(_type_in: &ObjType, n_args: usize, n_kw: usize, args: &[Obj]) -> Obj {
    argcheck::check_num(n_args, n_kw, 0, 1, false);
    match n_args {
        0 => const_empty_tuple(),
        1 => {
            if obj::is_exact_type(args[0], type_tuple()) {
                return args[0];
            }
            let mut alloc = 4usize;
            let mut len = 0usize;
            let mut items: Vec<Obj> = Vec::with_capacity(alloc);
            items.resize(alloc, obj::OBJ_NULL);
            let iterable = runtime::getiter(args[0], None);
            loop {
                let item = runtime::iternext(iterable);
                if item == obj::OBJ_STOP_ITERATION {
                    break;
                }
                if len >= alloc {
                    alloc *= 2;
                    items.resize(alloc, obj::OBJ_NULL);
                }
                items[len] = item;
                len += 1;
            }
            new_tuple(len, Some(&items[..len]))
        }
        _ => unreachable!(),
    }
}

pub fn tuple_unary_op(op: UnaryOp, self_in: Obj) -> Obj {
    let self_ = unsafe { &*(obj::as_ptr(self_in) as *const ObjTuple) };
    match op {
        UnaryOp::Bool => obj::new_bool(self_.len != 0),
        UnaryOp::Hash => {
            let mut hash = const_empty_tuple().0 as obj::Int;
            for i in 0..self_.len {
                let item = unsafe { *items_ptr(self_ as *const ObjTuple).add(i) };
                let h = runtime::unary_op_obj(UnaryOp::Hash, item);
                hash += obj::small_int_value(h);
            }
            obj::new_small_int(hash)
        }
        UnaryOp::Len => obj::new_small_int(self_.len as obj::Int),
        _ => obj::OBJ_NULL,
    }
}

pub fn tuple_binary_op(op: BinaryOp, lhs: Obj, rhs: Obj) -> Obj {
    let o = unsafe { &*(obj::as_ptr(lhs) as *const ObjTuple) };
    match op {
        BinaryOp::Add | BinaryOp::InplaceAdd => {
            let p = match tuple_subclass_helper(rhs) {
                Some(p) => p,
                None => return obj::OBJ_NULL,
            };
            let total = o.len + unsafe { (*p).len };
            let s = obj::malloc_var::<ObjTuple>(total * size_of::<Obj>(), type_tuple());
            unsafe {
                (*s).len = total;
                seq_cat(
                    items_ptr_mut(s),
                    items_ptr(o as *const ObjTuple),
                    o.len,
                    items_ptr(p),
                    (*p).len,
                );
            }
            obj::from_ptr(s as *const ObjTuple as *const ())
        }
        BinaryOp::Multiply | BinaryOp::InplaceMultiply => {
            let mut n = 0;
            if !obj::get_int_maybe(rhs, &mut n) {
                return obj::OBJ_NULL;
            }
            if n <= 0 {
                return const_empty_tuple();
            }
            let n = n as usize;
            let s = obj::malloc_var::<ObjTuple>(o.len * n * size_of::<Obj>(), type_tuple());
            unsafe {
                (*s).len = o.len * n;
                sequence::multiply(
                    std::slice::from_raw_parts(items_ptr(o as *const ObjTuple) as *const u8, o.len * size_of::<Obj>()),
                    size_of::<Obj>(),
                    o.len,
                    n,
                    std::slice::from_raw_parts_mut(items_ptr_mut(s) as *mut u8, o.len * n * size_of::<Obj>()),
                );
            }
            obj::from_ptr(s as *const ObjTuple as *const ())
        }
        BinaryOp::Equal | BinaryOp::Less | BinaryOp::LessEqual | BinaryOp::More | BinaryOp::MoreEqual => {
            tuple_cmp_helper(op, lhs, rhs)
        }
        _ => obj::OBJ_NULL,
    }
}

pub fn tuple_subscr(self_in: Obj, index: Obj, value: Obj) -> Obj {
    if value == OBJ_SENTINEL {
        let self_ = unsafe { &*(obj::as_ptr(self_in) as *const ObjTuple) };
        if mpconfig::PY_BUILTINS_SLICE && obj::is_exact_type(index, objslice::type_slice()) {
            let mut slice = objslice::BoundSlice { start: 0, stop: 0, step: 1 };
            if !sequence::get_fast_slice_indexes(self_.len, index, &mut slice) {
                raise::raise(MpRaise::RuntimeError("only slices with step=1 (aka None) are supported"));
            }
            let res_len = (slice.stop - slice.start) as usize;
            let res = obj::malloc_var::<ObjTuple>(res_len * size_of::<Obj>(), type_tuple());
            unsafe {
                (*res).len = res_len;
                seq_copy(
                    items_ptr_mut(res),
                    items_ptr(self_ as *const ObjTuple).add(slice.start as usize),
                    res_len,
                );
            }
            return obj::from_ptr(res as *const ObjTuple as *const ());
        }
        let type_ = unsafe { &*self_.base.type_ };
        let index_value = obj::get_index(type_, self_.len, index, false);
        unsafe { *items_ptr(self_ as *const ObjTuple).add(index_value) }
    } else {
        obj::OBJ_NULL
    }
}

pub fn tuple_count(self_in: Obj, value: Obj) -> Obj {
    check_self(self_in);
    let self_ = unsafe { &*(obj::as_ptr(self_in) as *const ObjTuple) };
    let items = unsafe { std::slice::from_raw_parts(items_ptr(self_ as *const ObjTuple), self_.len) };
    sequence::count_obj(items, self_.len, value)
}

pub fn tuple_index(n_args: usize, args: &[Obj]) -> Obj {
    check_self(args[0]);
    let self_ = unsafe { &*(obj::as_ptr(args[0]) as *const ObjTuple) };
    let items = unsafe { std::slice::from_raw_parts(items_ptr(self_ as *const ObjTuple), self_.len) };
    sequence::index_obj(items, self_.len, n_args, args)
}

pub fn tuple_getiter(o_in: Obj, iter_buf: *mut ObjIterBuf) -> Obj {
    debug_assert!(size_of::<ObjTupleIter>() <= size_of::<ObjIterBuf>());
    let o = unsafe { &mut *(iter_buf as *mut ObjTupleIter) };
    o.base.type_ = objpolyiter::type_polymorph_iter() as *const ObjType;
    o.iternext = tuple_it_iternext;
    o.tuple = o_in;
    o.cur = 0;
    obj::from_ptr(iter_buf as *const ObjTupleIter as *const ())
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
    fn empty_and_len() {
        setup();
        let t = const_empty_tuple();
        assert_eq!(obj::small_int_value(tuple_unary_op(UnaryOp::Len, t)), 0);
        assert!(!obj::is_true(tuple_unary_op(UnaryOp::Bool, t)));
    }

    #[test]
    fn new_and_get() {
        setup();
        let items = [obj::new_small_int(1), obj::new_small_int(2)];
        let t = new_tuple(2, Some(&items));
        let (len, got) = tuple_get(t);
        assert_eq!(len, 2);
        assert_eq!(obj::small_int_value(got[0]), 1);
    }

    #[test]
    fn add_concat() {
        setup();
        let a = new_tuple(1, Some(&[obj::new_small_int(1)]));
        let b = new_tuple(1, Some(&[obj::new_small_int(2)]));
        let c = tuple_binary_op(BinaryOp::Add, a, b);
        let (len, items) = tuple_get(c);
        assert_eq!(len, 2);
        assert_eq!(obj::small_int_value(items[1]), 2);
    }

    #[test]
    fn subscr_index() {
        setup();
        let t = new_tuple(2, Some(&[obj::new_small_int(10), obj::new_small_int(20)]));
        let v = tuple_subscr(t, obj::new_small_int(1), OBJ_SENTINEL);
        assert_eq!(obj::small_int_value(v), 20);
    }
}
