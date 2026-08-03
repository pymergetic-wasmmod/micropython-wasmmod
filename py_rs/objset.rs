//! rewrite of py/objset.c
// symmetry: done

use core::mem::size_of;

use crate::argcheck;
use crate::map::{self, LookupKind, MapElem, Set};
use crate::malloc;
use crate::mpconfig;
use crate::mpprint::{self, Print, PrintKind};
use crate::obj::{
    self, IterNextFn, Obj, ObjBase, ObjIterBuf, ObjType, OBJ_NULL, TYPE_FLAG_BINDS_SELF,
    TYPE_FLAG_BUILTIN_FUN, TYPE_FLAG_EQ_CHECKS_OTHER_TYPE,
};
use crate::objdict::{self, ObjDict};
use crate::objlist;
use crate::objpolyiter;
use crate::qstr;
use crate::raise::{self, MpRaise};
use crate::runtime;
use crate::runtime0::{BinaryOp, UnaryOp};

#[repr(C)]
pub struct ObjSet {
    pub base: ObjBase,
    pub set: Set,
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

// --- set iterator -------------------------------------------------------------

#[repr(C)]
struct ObjSetIter {
    base: ObjBase,
    iternext: IterNextFn,
    set: *mut ObjSet,
    cur: usize,
}

fn set_it_iternext(self_in: Obj) -> Obj {
    let self_ = unsafe { &mut *(obj::as_ptr(self_in) as *mut ObjSetIter) };
    let set = unsafe { &*self_.set };
    let max = set.set.alloc;
    for i in self_.cur..max {
        if map::set_slot_is_filled(&set.set, i) {
            self_.cur = i + 1;
            return set.set.table[i];
        }
    }
    obj::OBJ_STOP_ITERATION
}

// --- set type slots -----------------------------------------------------------

static mut SET_SLOTS: [*const (); 7] = [
    set_make_new as *const (),
    set_print as *const (),
    set_unary_op as *const (),
    set_binary_op as *const (),
    core::ptr::null(),
    set_getiter as *const (),
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
    slot_index_subscr: 0,
    slot_index_call: 0,
    slot_index_attr: 0,
    slot_index_iter: 6,
    slot_index_buffer: 0,
    slot_index_protocol: 0,
    slot_index_parent: 0,
    slot_index_locals_dict: 7,
    slots: unsafe { SET_SLOTS.as_ptr() },
};

static SET_INIT: std::sync::OnceLock<()> = std::sync::OnceLock::new();

static mut FROZENSET_SLOTS: [*const (); 7] = [
    set_make_new as *const (),
    set_print as *const (),
    set_unary_op as *const (),
    set_binary_op as *const (),
    core::ptr::null(),
    set_getiter as *const (),
    core::ptr::null(),
];

static TYPE_FROZENSET: ObjType = ObjType {
    base: ObjBase { type_: core::ptr::null() },
    flags: TYPE_FLAG_EQ_CHECKS_OTHER_TYPE,
    name: 0,
    slot_index_make_new: 1,
    slot_index_print: 2,
    slot_index_unary_op: 3,
    slot_index_binary_op: 4,
    slot_index_subscr: 0,
    slot_index_call: 0,
    slot_index_attr: 0,
    slot_index_iter: 6,
    slot_index_buffer: 0,
    slot_index_protocol: 0,
    slot_index_parent: 0,
    slot_index_locals_dict: 7,
    slots: unsafe { FROZENSET_SLOTS.as_ptr() },
};

static FROZENSET_INIT: std::sync::OnceLock<()> = std::sync::OnceLock::new();

fn init_set_type() {
    if !mpconfig::PY_BUILTINS_SET {
        return;
    }
    SET_INIT.get_or_init(|| {
        let table = vec![
            MapElem { key: obj::new_qstr(qstr::from_str("add")), value: new_fun_builtin_2(set_add) },
            MapElem { key: obj::new_qstr(qstr::from_str("clear")), value: new_fun_builtin_1(set_clear) },
            MapElem { key: obj::new_qstr(qstr::from_str("copy")), value: new_fun_builtin_1(set_copy) },
            MapElem { key: obj::new_qstr(qstr::from_str("discard")), value: new_fun_builtin_2(set_discard) },
            MapElem { key: obj::new_qstr(qstr::from_str("difference")), value: new_fun_builtin_var(1, usize::MAX as u8, set_diff) },
            MapElem { key: obj::new_qstr(qstr::from_str("difference_update")), value: new_fun_builtin_var(1, usize::MAX as u8, set_diff_update) },
            MapElem { key: obj::new_qstr(qstr::from_str("intersection")), value: new_fun_builtin_2(set_intersect) },
            MapElem { key: obj::new_qstr(qstr::from_str("intersection_update")), value: new_fun_builtin_2(set_intersect_update) },
            MapElem { key: obj::new_qstr(qstr::from_str("isdisjoint")), value: new_fun_builtin_2(set_isdisjoint) },
            MapElem { key: obj::new_qstr(qstr::from_str("issubset")), value: new_fun_builtin_2(set_issubset) },
            MapElem { key: obj::new_qstr(qstr::from_str("issuperset")), value: new_fun_builtin_2(set_issuperset) },
            MapElem { key: obj::new_qstr(qstr::from_str("pop")), value: new_fun_builtin_1(set_pop) },
            MapElem { key: obj::new_qstr(qstr::from_str("remove")), value: new_fun_builtin_2(set_remove) },
            MapElem { key: obj::new_qstr(qstr::from_str("symmetric_difference")), value: new_fun_builtin_2(set_symmetric_difference) },
            MapElem { key: obj::new_qstr(qstr::from_str("symmetric_difference_update")), value: new_fun_builtin_2(set_symmetric_difference_update) },
            MapElem { key: obj::new_qstr(qstr::from_str("union")), value: new_fun_builtin_2(set_union) },
            MapElem { key: obj::new_qstr(qstr::from_str("update")), value: new_fun_builtin_var(1, usize::MAX as u8, set_update) },
            MapElem { key: obj::new_qstr(qstr::from_str("__contains__")), value: new_fun_builtin_2(op_contains) },
        ];
        let ptr = obj::malloc_helper(size_of::<ObjDict>(), &TYPE) as *mut ObjDict;
        unsafe {
            map::init_fixed_table(&mut (*ptr).map, table);
            SET_SLOTS[6] = obj::from_ptr(ptr as *const ObjDict as *const ()).0 as *const ();
        }
    });
}

fn init_frozenset_type() {
    if !mpconfig::PY_BUILTINS_SET || !mpconfig::PY_BUILTINS_FROZENSET {
        return;
    }
    FROZENSET_INIT.get_or_init(|| {
        init_set_type();
        let table = vec![
            MapElem { key: obj::new_qstr(qstr::from_str("copy")), value: new_fun_builtin_1(set_copy) },
            MapElem { key: obj::new_qstr(qstr::from_str("difference")), value: new_fun_builtin_var(1, usize::MAX as u8, set_diff) },
            MapElem { key: obj::new_qstr(qstr::from_str("intersection")), value: new_fun_builtin_2(set_intersect) },
            MapElem { key: obj::new_qstr(qstr::from_str("isdisjoint")), value: new_fun_builtin_2(set_isdisjoint) },
            MapElem { key: obj::new_qstr(qstr::from_str("issubset")), value: new_fun_builtin_2(set_issubset) },
            MapElem { key: obj::new_qstr(qstr::from_str("issuperset")), value: new_fun_builtin_2(set_issuperset) },
            MapElem { key: obj::new_qstr(qstr::from_str("symmetric_difference")), value: new_fun_builtin_2(set_symmetric_difference) },
            MapElem { key: obj::new_qstr(qstr::from_str("union")), value: new_fun_builtin_2(set_union) },
            MapElem { key: obj::new_qstr(qstr::from_str("__contains__")), value: new_fun_builtin_2(op_contains) },
        ];
        let ptr = obj::malloc_helper(size_of::<ObjDict>(), &TYPE_FROZENSET) as *mut ObjDict;
        unsafe {
            map::init_fixed_table(&mut (*ptr).map, table);
            FROZENSET_SLOTS[6] = obj::from_ptr(ptr as *const ObjDict as *const ()).0 as *const ();
        }
    });
}

pub fn type_set() -> &'static ObjType {
    if !mpconfig::PY_BUILTINS_SET {
        panic!("set builtin disabled");
    }
    init_set_type();
    &TYPE
}

pub fn type_frozenset() -> &'static ObjType {
    if !mpconfig::PY_BUILTINS_SET {
        panic!("set builtin disabled");
    }
    if !mpconfig::PY_BUILTINS_FROZENSET {
        panic!("frozenset builtin disabled");
    }
    init_frozenset_type();
    &TYPE_FROZENSET
}

fn set_ptr(o: Obj) -> *mut ObjSet {
    obj::as_ptr(o) as *mut ObjSet
}

fn is_set_or_frozenset(o: Obj) -> bool {
    if !obj::is_obj(o) {
        return false;
    }
    if obj::is_exact_type(o, type_set()) {
        return true;
    }
    if mpconfig::PY_BUILTINS_FROZENSET && obj::is_exact_type(o, type_frozenset()) {
        return true;
    }
    false
}

fn check_set(o: Obj) {
    if !obj::is_exact_type(o, type_set()) {
        raise::raise(MpRaise::TypeError("set method on non-set"));
    }
}

fn check_set_or_frozenset(o: Obj) {
    if !is_set_or_frozenset(o) {
        raise::raise(MpRaise::TypeError("set method on non-set"));
    }
}

fn set_new_empty(type_in: &'static ObjType) -> *mut ObjSet {
    let o = malloc::new_obj::<ObjSet>().expect("objset alloc");
    unsafe {
        (*o).base.type_ = type_in as *const ObjType;
        map::set_init(&mut (*o).set, 0);
    }
    o
}

fn iter_next(iter: Obj) -> Obj {
    let t = obj::get_type(iter);
    if core::ptr::eq(t, objpolyiter::type_polymorph_iter()) {
        objpolyiter::polymorph_it_iternext(iter)
    } else if (t.flags & obj::TYPE_FLAG_ITER_IS_ITERNEXT) != 0 {
        if let Some(slot) = obj::type_get_iter(t) {
            unsafe {
                std::mem::transmute::<_, fn(Obj, *mut ObjIterBuf) -> Obj>(slot)(iter, core::ptr::null_mut())
            }
        } else {
            obj::OBJ_STOP_ITERATION
        }
    } else {
        runtime::iternext(iter)
    }
}

fn for_each_item(other_in: Obj, mut f: impl FnMut(Obj)) {
    if is_set_or_frozenset(other_in) {
        let s = unsafe { &*set_ptr(other_in) };
        for i in 0..s.set.alloc {
            if map::set_slot_is_filled(&s.set, i) {
                f(s.set.table[i]);
            }
        }
        return;
    }
    if obj::is_exact_type(other_in, objlist::type_list()) {
        let (_len, items) = objlist::list_get(other_in);
        for item in items {
            f(item);
        }
        return;
    }
    let mut iter_buf = ObjIterBuf {
        base: ObjBase { type_: core::ptr::null() },
        buf: [obj::OBJ_NULL; 3],
    };
    let iter = runtime::getiter(other_in, Some(&mut iter_buf));
    loop {
        let item = iter_next(iter);
        if item == obj::OBJ_STOP_ITERATION {
            break;
        }
        f(item);
    }
}

fn set_extend_from_iter(set: Obj, iterable: Obj) {
    for_each_item(iterable, |item| set_store(set, item));
}

fn set_update_int(self_: *mut ObjSet, other_in: Obj) {
    for_each_item(other_in, |next| {
        map::set_lookup(&mut unsafe { &mut *self_ }.set, next, LookupKind::AddIfNotFound);
    });
}

// --- public C API -------------------------------------------------------------

/// `mp_obj_new_set`
pub fn new_set(n_args: usize, items: Option<&[Obj]>) -> Obj {
    let o = set_new_empty(type_set());
    map::set_init(unsafe { &mut (*o).set }, n_args);
    if let Some(items) = items {
        for &item in items {
            map::set_lookup(&mut unsafe { &mut *o }.set, item, LookupKind::AddIfNotFound);
        }
    }
    obj::from_ptr(o as *const ObjSet as *const ())
}

/// `mp_obj_set_store`
pub fn set_store(self_in: Obj, item: Obj) {
    check_set(self_in);
    let self_ = unsafe { &mut *set_ptr(self_in) };
    map::set_lookup(&mut self_.set, item, LookupKind::AddIfNotFound);
}

// --- type methods -------------------------------------------------------------

pub fn set_print(print: &Print, self_in: Obj, _kind: PrintKind) {
    let self_ = unsafe { &*set_ptr(self_in) };
    let is_frozen = mpconfig::PY_BUILTINS_FROZENSET && obj::is_exact_type(self_in, type_frozenset());
    if self_.set.used == 0 {
        if is_frozen {
            mpprint::print_str(print, "frozen");
        }
        mpprint::print_str(print, "set()");
        return;
    }
    let mut first = true;
    if is_frozen {
        mpprint::print_str(print, "frozenset(");
    }
    mpprint::print_str(print, "{");
    for i in 0..self_.set.alloc {
        if map::set_slot_is_filled(&self_.set, i) {
            if !first {
                mpprint::print_str(print, ", ");
            }
            first = false;
            obj::print_helper(print, self_.set.table[i], PrintKind::Repr);
        }
    }
    mpprint::print_str(print, "}");
    if is_frozen {
        mpprint::print_str(print, ")");
    }
}

pub fn set_make_new(type_in: &ObjType, n_args: usize, n_kw: usize, args: &[Obj]) -> Obj {
    argcheck::check_num(n_args, n_kw, 0, 1, false);
    match n_args {
        0 => {
            let set = new_set(0, None);
            unsafe {
                (*set_ptr(set)).base.type_ = type_in as *const ObjType;
            }
            set
        }
        _ => {
            let set = new_set(0, None);
            set_extend_from_iter(set, args[0]);
            unsafe {
                (*set_ptr(set)).base.type_ = type_in as *const ObjType;
            }
            set
        }
    }
}

pub fn set_getiter(set_in: Obj, iter_buf: *mut ObjIterBuf) -> Obj {
    debug_assert!(size_of::<ObjSetIter>() <= size_of::<ObjIterBuf>());
    let o = unsafe { &mut *(iter_buf as *mut ObjSetIter) };
    o.base.type_ = objpolyiter::type_polymorph_iter() as *const ObjType;
    o.iternext = set_it_iternext;
    o.set = set_ptr(set_in);
    o.cur = 0;
    obj::from_ptr(iter_buf as *const ObjSetIter as *const ())
}

pub fn set_unary_op(op: UnaryOp, self_in: Obj) -> Obj {
    let self_ = unsafe { &*set_ptr(self_in) };
    match op {
        UnaryOp::Bool => obj::new_bool(self_.set.used != 0),
        UnaryOp::Len => obj::new_small_int(self_.set.used as obj::Int),
        UnaryOp::Hash if mpconfig::PY_BUILTINS_FROZENSET && obj::is_exact_type(self_in, type_frozenset()) => {
            let mut hash = type_frozenset() as *const ObjType as usize as obj::Int;
            for i in 0..self_.set.alloc {
                if map::set_slot_is_filled(&self_.set, i) {
                    hash += obj::small_int_value(runtime::unary_op_obj(UnaryOp::Hash, self_.set.table[i]));
                }
            }
            obj::new_small_int(hash)
        }
        _ => obj::OBJ_NULL,
    }
}

pub fn set_binary_op(op: BinaryOp, lhs: Obj, rhs: Obj) -> Obj {
    let args = [lhs, rhs];
    let update = if mpconfig::PY_BUILTINS_FROZENSET {
        obj::is_exact_type(lhs, type_set())
    } else {
        true
    };
    if op != BinaryOp::Contains && !is_set_or_frozenset(rhs) {
        return obj::OBJ_NULL;
    }
    match op {
        BinaryOp::Or => set_union(lhs, rhs),
        BinaryOp::Xor => set_symmetric_difference(lhs, rhs),
        BinaryOp::And => set_intersect(lhs, rhs),
        BinaryOp::Subtract => set_diff(2, &args),
        BinaryOp::InplaceOr => {
            if update {
                set_update(2, &args);
                lhs
            } else {
                set_union(lhs, rhs)
            }
        }
        BinaryOp::InplaceXor => {
            if update {
                set_symmetric_difference_update(lhs, rhs);
                lhs
            } else {
                set_symmetric_difference(lhs, rhs)
            }
        }
        BinaryOp::InplaceAnd => {
            let result = set_intersect_int(lhs, rhs, update);
            if update {
                lhs
            } else {
                result
            }
        }
        BinaryOp::InplaceSubtract => set_diff_int(2, &args, update),
        BinaryOp::Less => set_issubset_proper(lhs, rhs),
        BinaryOp::More => set_issuperset_proper(lhs, rhs),
        BinaryOp::Equal => set_equal(lhs, rhs),
        BinaryOp::LessEqual => set_issubset(lhs, rhs),
        BinaryOp::MoreEqual => set_issuperset(lhs, rhs),
        BinaryOp::Contains => {
            let o = unsafe { &mut *set_ptr(lhs) };
            let elem = map::set_lookup(&mut o.set, rhs, LookupKind::Lookup);
            obj::new_bool(elem != OBJ_NULL)
        }
        _ => obj::OBJ_NULL,
    }
}

// --- set methods --------------------------------------------------------------

fn op_contains(self_in: Obj, item: Obj) -> Obj {
    set_binary_op(BinaryOp::Contains, self_in, item)
}

pub fn set_add(self_in: Obj, item: Obj) -> Obj {
    check_set(self_in);
    let self_ = unsafe { &mut *set_ptr(self_in) };
    map::set_lookup(&mut self_.set, item, LookupKind::AddIfNotFound);
    obj::CONST_NONE
}

pub fn set_clear(self_in: Obj) -> Obj {
    check_set(self_in);
    let self_ = unsafe { &mut *set_ptr(self_in) };
    map::set_clear(&mut self_.set);
    obj::CONST_NONE
}

pub fn set_copy(self_in: Obj) -> Obj {
    check_set_or_frozenset(self_in);
    let self_ = unsafe { &*set_ptr(self_in) };
    let other = malloc::new_obj::<ObjSet>().expect("objset copy");
    unsafe {
        (*other).base.type_ = self_.base.type_;
        map::set_init(&mut (*other).set, self_.set.alloc);
        (*other).set.used = self_.set.used;
        (*other).set.table = self_.set.table.clone();
    }
    obj::from_ptr(other as *const ObjSet as *const ())
}

pub fn set_discard(self_in: Obj, item: Obj) -> Obj {
    check_set(self_in);
    let self_ = unsafe { &mut *set_ptr(self_in) };
    map::set_lookup(&mut self_.set, item, LookupKind::RemoveIfFound);
    obj::CONST_NONE
}

fn set_diff_int(n_args: usize, args: &[Obj], update: bool) -> Obj {
    let self_out = if update {
        check_set(args[0]);
        args[0]
    } else {
        set_copy(args[0])
    };

    for i in 1..n_args {
        let other = args[i];
        if self_out == other {
            set_clear(self_out);
        } else {
            let self_ = unsafe { &mut *set_ptr(self_out) };
            for_each_item(other, |next| {
                map::set_lookup(&mut self_.set, next, LookupKind::RemoveIfFound);
            });
        }
    }

    self_out
}

pub fn set_diff(n_args: usize, args: &[Obj]) -> Obj {
    set_diff_int(n_args, args, false)
}

pub fn set_diff_update(n_args: usize, args: &[Obj]) -> Obj {
    set_diff_int(n_args, args, true);
    obj::CONST_NONE
}

fn set_intersect_int(self_in: Obj, other: Obj, update: bool) -> Obj {
    if update {
        check_set(self_in);
    } else {
        check_set_or_frozenset(self_in);
    }

    if self_in == other {
        return if update { obj::CONST_NONE } else { set_copy(self_in) };
    }

    let self_mut = unsafe { &mut *set_ptr(self_in) };
    let out = set_new_empty(type_set());
    for_each_item(other, |next| {
        if map::set_lookup(&mut self_mut.set, next, LookupKind::Lookup) != OBJ_NULL {
            map::set_lookup(&mut unsafe { &mut *out }.set, next, LookupKind::AddIfNotFound);
        }
    });

    if update {
        let self_mut = unsafe { &mut *set_ptr(self_in) };
        self_mut.set = std::mem::take(&mut unsafe { &mut *out }.set);
        obj::CONST_NONE
    } else {
        obj::from_ptr(out as *const ObjSet as *const ())
    }
}

pub fn set_intersect(self_in: Obj, other: Obj) -> Obj {
    set_intersect_int(self_in, other, false)
}

pub fn set_intersect_update(self_in: Obj, other: Obj) -> Obj {
    set_intersect_int(self_in, other, true)
}

pub fn set_isdisjoint(self_in: Obj, other: Obj) -> Obj {
    check_set_or_frozenset(self_in);
    let self_ = unsafe { &mut *set_ptr(self_in) };
    let mut disjoint = true;
    for_each_item(other, |next| {
        if map::set_lookup(&mut self_.set, next, LookupKind::Lookup) != OBJ_NULL {
            disjoint = false;
        }
    });
    obj::new_bool(disjoint)
}

fn set_issubset_internal(self_in: Obj, other_in: Obj, proper: bool) -> Obj {
    let (self_obj, cleanup_self) = if is_set_or_frozenset(self_in) {
        (self_in, false)
    } else {
        (set_make_new(type_set(), 1, 0, &[self_in]), true)
    };

    let (other_obj, cleanup_other) = if is_set_or_frozenset(other_in) {
        (other_in, false)
    } else {
        (set_make_new(type_set(), 1, 0, &[other_in]), true)
    };

    let self_ = unsafe { &*set_ptr(self_obj) };
    let other = unsafe { &*set_ptr(other_obj) };
    let out = if proper && self_.set.used == other.set.used {
        obj::CONST_FALSE
    } else {
        let mut iter_buf = ObjIterBuf {
            base: ObjBase { type_: core::ptr::null() },
            buf: [obj::OBJ_NULL; 3],
        };
        let iter = set_getiter(self_obj, &mut iter_buf as *mut ObjIterBuf);
        let mut result = obj::CONST_TRUE;
        loop {
            let next = set_it_iternext(iter);
            if next == obj::OBJ_STOP_ITERATION {
                break;
            }
            if map::set_lookup(&mut unsafe { &mut *set_ptr(other_obj) }.set, next, LookupKind::Lookup)
                == OBJ_NULL
            {
                result = obj::CONST_FALSE;
                break;
            }
        }
        result
    };

    if cleanup_self {
        set_clear(self_obj);
    }
    if cleanup_other {
        set_clear(other_obj);
    }
    out
}

pub fn set_issubset(self_in: Obj, other_in: Obj) -> Obj {
    set_issubset_internal(self_in, other_in, false)
}

fn set_issubset_proper(self_in: Obj, other_in: Obj) -> Obj {
    set_issubset_internal(self_in, other_in, true)
}

pub fn set_issuperset(self_in: Obj, other_in: Obj) -> Obj {
    set_issubset_internal(other_in, self_in, false)
}

fn set_issuperset_proper(self_in: Obj, other_in: Obj) -> Obj {
    set_issubset_internal(other_in, self_in, true)
}

fn set_equal(self_in: Obj, other_in: Obj) -> Obj {
    debug_assert!(is_set_or_frozenset(other_in));
    check_set_or_frozenset(self_in);
    let self_ = unsafe { &*set_ptr(self_in) };
    let other = unsafe { &*set_ptr(other_in) };
    if self_.set.used != other.set.used {
        return obj::CONST_FALSE;
    }
    set_issubset(self_in, other_in)
}

pub fn set_pop(self_in: Obj) -> Obj {
    check_set(self_in);
    let self_ = unsafe { &mut *set_ptr(self_in) };
    let obj_out = map::set_remove_first(&mut self_.set);
    if obj_out == OBJ_NULL {
        raise::raise(MpRaise::RuntimeError("pop from an empty set"));
    }
    obj_out
}

pub fn set_remove(self_in: Obj, item: Obj) -> Obj {
    check_set(self_in);
    let self_ = unsafe { &mut *set_ptr(self_in) };
    if map::set_lookup(&mut self_.set, item, LookupKind::RemoveIfFound) == OBJ_NULL {
        raise::raise(MpRaise::RuntimeError("KeyError"));
    }
    obj::CONST_NONE
}

pub fn set_symmetric_difference_update(self_in: Obj, other_in: Obj) -> Obj {
    check_set_or_frozenset(self_in);
    let self_ = unsafe { &mut *set_ptr(self_in) };
    for_each_item(other_in, |next| {
        map::set_lookup(
            &mut self_.set,
            next,
            LookupKind::AddIfNotFoundOrRemoveIfFound,
        );
    });
    obj::CONST_NONE
}

pub fn set_symmetric_difference(self_in: Obj, other_in: Obj) -> Obj {
    let self_out = set_copy(self_in);
    set_symmetric_difference_update(self_out, other_in);
    self_out
}

pub fn set_update(n_args: usize, args: &[Obj]) -> Obj {
    check_set(args[0]);
    for i in 1..n_args {
        set_update_int(set_ptr(args[0]), args[i]);
    }
    obj::CONST_NONE
}

pub fn set_union(self_in: Obj, other_in: Obj) -> Obj {
    check_set_or_frozenset(self_in);
    let self_out = set_copy(self_in);
    set_update_int(set_ptr(self_out), other_in);
    self_out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gc;

    fn setup() {
        let _ = gc::init();
        qstr::init();
    }

    fn int_set(values: &[obj::Int]) -> Obj {
        let s = new_set(0, None);
        for &v in values {
            set_add(s, obj::new_small_int(v));
        }
        s
    }

    fn set_len(s: Obj) -> obj::Int {
        obj::small_int_value(set_unary_op(UnaryOp::Len, s))
    }

    fn set_contains(s: Obj, v: obj::Int) -> bool {
        obj::is_true(set_binary_op(
            BinaryOp::Contains,
            s,
            obj::new_small_int(v),
        ))
    }

    #[test]
    fn add_and_contains() {
        setup();
        let s = new_set(0, None);
        set_add(s, obj::new_small_int(1));
        set_add(s, obj::new_small_int(2));
        set_add(s, obj::new_small_int(1));
        assert_eq!(set_len(s), 2);
        assert!(set_contains(s, 1));
        assert!(set_contains(s, 2));
        assert!(!set_contains(s, 3));
    }

    #[test]
    fn store_adds_item() {
        setup();
        let s = new_set(0, None);
        set_store(s, obj::new_small_int(42));
        assert!(set_contains(s, 42));
    }

    #[test]
    fn discard_and_remove() {
        setup();
        let s = int_set(&[1, 2, 3]);
        set_discard(s, obj::new_small_int(2));
        assert!(!set_contains(s, 2));
        set_remove(s, obj::new_small_int(1));
        assert!(!set_contains(s, 1));
    }

    #[test]
    fn copy_is_independent() {
        setup();
        let a = int_set(&[1, 2]);
        let b = set_copy(a);
        set_add(a, obj::new_small_int(3));
        assert_eq!(set_len(a), 3);
        assert_eq!(set_len(b), 2);
        assert!(!set_contains(b, 3));
    }

    #[test]
    fn union_and_intersection() {
        setup();
        let a = int_set(&[1, 2, 3]);
        let b = int_set(&[2, 3, 4]);
        let u = set_union(a, b);
        assert_eq!(set_len(u), 4);
        assert!(set_contains(u, 4));
        let i = set_intersect(a, b);
        assert_eq!(set_len(i), 2);
        assert!(set_contains(i, 2));
    }

    #[test]
    fn difference_and_symmetric_difference() {
        setup();
        let a = int_set(&[1, 2, 3]);
        let b = int_set(&[2, 3, 4]);
        let d = set_diff(2, &[a, b]);
        assert_eq!(set_len(d), 1);
        assert!(set_contains(d, 1));
        let x = set_symmetric_difference(a, b);
        assert_eq!(set_len(x), 2);
        assert!(set_contains(x, 1));
        assert!(set_contains(x, 4));
    }

    #[test]
    fn subset_and_superset() {
        setup();
        let a = int_set(&[1, 2]);
        let b = int_set(&[1, 2, 3]);
        assert!(obj::is_true(set_issubset(a, b)));
        assert!(obj::is_true(set_issuperset(b, a)));
        assert!(obj::is_true(set_equal(a, set_copy(a))));
    }

    #[test]
    fn isdisjoint() {
        setup();
        let a = int_set(&[1, 2]);
        let b = int_set(&[3, 4]);
        let c = int_set(&[2, 3]);
        assert!(obj::is_true(set_isdisjoint(a, b)));
        assert!(!obj::is_true(set_isdisjoint(a, c)));
    }

    #[test]
    fn pop_and_clear() {
        setup();
        let s = int_set(&[10, 20]);
        let v = set_pop(s);
        assert!(obj::small_int_value(v) == 10 || obj::small_int_value(v) == 20);
        assert_eq!(set_len(s), 1);
        set_clear(s);
        assert_eq!(set_len(s), 0);
    }

    #[test]
    fn make_new_from_iterable() {
        setup();
        let lst = objlist::new_list(2, Some(&[obj::new_small_int(5), obj::new_small_int(6)]));
        let s = set_make_new(type_set(), 1, 0, &[lst]);
        assert_eq!(set_len(s), 2);
        assert!(set_contains(s, 5));
        assert!(set_contains(s, 6));
    }

    #[test]
    fn binary_ops() {
        setup();
        let a = int_set(&[1, 2]);
        let b = int_set(&[2, 3]);
        let u = set_binary_op(BinaryOp::Or, a, b);
        assert_eq!(set_len(u), 3);
        let i = set_binary_op(BinaryOp::And, a, b);
        assert_eq!(set_len(i), 1);
        assert!(set_contains(i, 2));
        let d = set_binary_op(BinaryOp::Subtract, a, b);
        assert_eq!(set_len(d), 1);
        assert!(set_contains(d, 1));
    }

    #[test]
    fn iteration() {
        setup();
        let s = int_set(&[1, 2, 3]);
        let mut iter_buf = ObjIterBuf {
            base: ObjBase { type_: core::ptr::null() },
            buf: [obj::OBJ_NULL; 3],
        };
        let iter = set_getiter(s, &mut iter_buf as *mut ObjIterBuf);
        let mut count = 0;
        loop {
            let next = set_it_iternext(iter);
            if next == obj::OBJ_STOP_ITERATION {
                break;
            }
            count += 1;
        }
        assert_eq!(count, 3);
    }

    #[test]
    fn frozenset_hash_and_make_new() {
        if !mpconfig::PY_BUILTINS_FROZENSET {
            return;
        }
        setup();
        let fs = set_make_new(
            type_frozenset(),
            1,
            0,
            &[objlist::new_list(1, Some(&[obj::new_small_int(7)]))],
        );
        let h1 = set_unary_op(UnaryOp::Hash, fs);
        assert_ne!(h1, obj::OBJ_NULL);
        let h2 = set_unary_op(UnaryOp::Hash, fs);
        assert!(obj::equal(h1, h2));
        assert!(!obj::is_exact_type(fs, type_set()));
    }
}
