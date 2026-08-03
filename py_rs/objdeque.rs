//! rewrite of py/objdeque.c
// symmetry: done

use crate::argcheck;
use crate::map::{self, MapElem};
use crate::malloc;
use crate::mpconfig;
use crate::obj::{
    self, IterNextFn, Obj, ObjBase, ObjIterBuf, ObjType, OBJ_SENTINEL, TYPE_FLAG_BINDS_SELF,
    TYPE_FLAG_BUILTIN_FUN, TYPE_FLAG_ITER_IS_CUSTOM,
};
use crate::objdict::{self, ObjDict};
use crate::objexcept;
use crate::objpolyiter;
use crate::objstr;
use crate::qstr;
use crate::raise::{self, MpRaise};
use crate::runtime;
use crate::runtime0::UnaryOp;
use crate::sequence;

const FLAG_CHECK_OVERFLOW: u32 = 1;

#[repr(C)]
pub struct ObjDeque {
    pub base: ObjBase,
    pub alloc: usize,
    pub i_get: usize,
    pub i_put: usize,
    pub items: *mut Obj,
    pub flags: u32,
}

type BuiltinFn1 = fn(Obj) -> Obj;
type BuiltinFn2 = fn(Obj, Obj) -> Obj;

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

static mut FUN1_SLOTS: [*const (); 1] = [fun1_call as *const ()];
static mut FUN2_SLOTS: [*const (); 1] = [fun2_call as *const ()];

static TYPE_FUN1: ObjType = ObjType {
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
    slots: unsafe { FUN1_SLOTS.as_ptr() },
};

static TYPE_FUN2: ObjType = ObjType {
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
    slots: unsafe { FUN2_SLOTS.as_ptr() },
};

fn fun1_call(self_in: Obj, n_args: usize, n_kw: usize, args: &[Obj]) -> Obj {
    argcheck::check_num(n_args, n_kw, 1, 1, false);
    let self_ = unsafe { &*(obj::as_ptr(self_in) as *const ObjFunBuiltin1) };
    (self_.fun)(args[0])
}

fn fun2_call(self_in: Obj, n_args: usize, n_kw: usize, args: &[Obj]) -> Obj {
    argcheck::check_num(n_args, n_kw, 2, 2, false);
    let self_ = unsafe { &*(obj::as_ptr(self_in) as *const ObjFunBuiltin2) };
    (self_.fun)(args[0], args[1])
}

fn new_fun1(fun: BuiltinFn1) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltin1>().expect("deque fun1");
    unsafe {
        (*o).base.type_ = &TYPE_FUN1 as *const ObjType;
        (*o).fun = fun;
        obj::from_ptr(o as *const ObjFunBuiltin1 as *const ())
    }
}

fn new_fun2(fun: BuiltinFn2) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltin2>().expect("deque fun2");
    unsafe {
        (*o).base.type_ = &TYPE_FUN2 as *const ObjType;
        (*o).fun = fun;
        obj::from_ptr(o as *const ObjFunBuiltin2 as *const ())
    }
}

fn empty_iter_buf() -> ObjIterBuf {
    ObjIterBuf {
        base: ObjBase {
            type_: core::ptr::null(),
        },
        buf: [obj::OBJ_NULL; 3],
    }
}

fn deque_index_error(msg: &'static str) -> ! {
    raise::raise_obj(objexcept::new_exception_args(
        objexcept::type_index_error(),
        1,
        &[objstr::new_str(msg.as_bytes())],
    ));
}
fn deque_ptr(o: Obj) -> *mut ObjDeque {
    obj::as_ptr(o) as *mut ObjDeque
}

fn deque_len(self_: &ObjDeque) -> usize {
    let mut len = self_.i_put as isize - self_.i_get as isize;
    if len < 0 {
        len += self_.alloc as isize;
    }
    len as usize
}

fn deque_append(self_in: Obj, arg: Obj) -> Obj {
    let self_ = unsafe { &mut *deque_ptr(self_in) };
    let mut new_i_put = self_.i_put + 1;
    if new_i_put == self_.alloc {
        new_i_put = 0;
    }
    if (self_.flags & FLAG_CHECK_OVERFLOW) != 0 && new_i_put == self_.i_get {
        deque_index_error("full");
    }
    unsafe {
        *self_.items.add(self_.i_put) = arg;
    }
    self_.i_put = new_i_put;
    if self_.i_get == new_i_put {
        self_.i_get += 1;
        if self_.i_get == self_.alloc {
            self_.i_get = 0;
        }
    }
    obj::CONST_NONE
}

fn deque_appendleft(self_in: Obj, arg: Obj) -> Obj {
    let self_ = unsafe { &mut *deque_ptr(self_in) };
    let mut new_i_get = self_.i_get.wrapping_sub(1);
    if self_.i_get == 0 {
        new_i_get = self_.alloc - 1;
    }
    if (self_.flags & FLAG_CHECK_OVERFLOW) != 0 && new_i_get == self_.i_put {
        deque_index_error("full");
    }
    self_.i_get = new_i_get;
    unsafe {
        *self_.items.add(self_.i_get) = arg;
    }
    if self_.i_put == new_i_get {
        if self_.i_put == 0 {
            self_.i_put = self_.alloc - 1;
        } else {
            self_.i_put -= 1;
        }
    }
    obj::CONST_NONE
}

fn deque_extend(self_in: Obj, arg_in: Obj) -> Obj {
    let mut iter_buf = empty_iter_buf();
    let iter = runtime::getiter(arg_in, Some(&mut iter_buf));
    loop {
        let item = runtime::iternext(iter);
        if item == obj::OBJ_STOP_ITERATION {
            break;
        }
        deque_append(self_in, item);
    }
    obj::CONST_NONE
}

fn deque_popleft(self_in: Obj) -> Obj {
    let self_ = unsafe { &mut *deque_ptr(self_in) };
    if self_.i_get == self_.i_put {
        deque_index_error("empty");
    }
    let ret = unsafe { *self_.items.add(self_.i_get) };
    unsafe {
        *self_.items.add(self_.i_get) = obj::OBJ_NULL;
    }
    self_.i_get += 1;
    if self_.i_get == self_.alloc {
        self_.i_get = 0;
    }
    ret
}

fn deque_pop(self_in: Obj) -> Obj {
    let self_ = unsafe { &mut *deque_ptr(self_in) };
    if self_.i_get == self_.i_put {
        deque_index_error("empty");
    }
    if self_.i_put == 0 {
        self_.i_put = self_.alloc - 1;
    } else {
        self_.i_put -= 1;
    }
    let ret = unsafe { *self_.items.add(self_.i_put) };
    unsafe {
        *self_.items.add(self_.i_put) = obj::OBJ_NULL;
    }
    ret
}

fn deque_make_new(type_in: &ObjType, n_args: usize, n_kw: usize, args: &[Obj]) -> Obj {
    argcheck::check_num(n_args, n_kw, 2, 3, false);
    let maxlen = obj::get_int(args[1]);
    if maxlen < 0 {
        raise::raise(MpRaise::ValueError(""));
    }
    let type_static: &'static ObjType = unsafe { &*(type_in as *const ObjType) };
    let o = obj::malloc_helper(core::mem::size_of::<ObjDeque>(), type_static) as *mut ObjDeque;
    unsafe {
        (*o).alloc = maxlen as usize + 1;
        (*o).i_get = 0;
        (*o).i_put = 0;
        (*o).items = malloc::new::<Obj>((*o).alloc).expect("deque items");
        for i in 0..(*o).alloc {
            *(*o).items.add(i) = obj::OBJ_NULL;
        }
        (*o).flags = if n_args > 2 { obj::get_int(args[2]) as u32 } else { 0 };
    }
    let self_obj = obj::from_ptr(o as *const ObjDeque as *const ());
    deque_extend(self_obj, args[0]);
    self_obj
}

fn deque_unary_op(op: UnaryOp, self_in: Obj) -> Obj {
    let self_ = unsafe { &*deque_ptr(self_in) };
    match op {
        UnaryOp::Bool => obj::new_bool(self_.i_get != self_.i_put),
        UnaryOp::Len => obj::new_small_int(deque_len(self_) as isize),
        _ => obj::OBJ_NULL,
    }
}

fn deque_subscr(self_in: Obj, index: Obj, value: Obj) -> Obj {
    if !mpconfig::PY_COLLECTIONS_DEQUE_SUBSCR {
        return obj::OBJ_NULL;
    }
    if value == obj::OBJ_NULL {
        return obj::OBJ_NULL;
    }
    let self_ = unsafe { &mut *deque_ptr(self_in) };
    let offset = obj::get_index(type_deque(), deque_len(self_), index, false);
    let mut index_val = self_.i_get + offset;
    if index_val >= self_.alloc {
        index_val -= self_.alloc;
    }
    if value == OBJ_SENTINEL {
        unsafe { *self_.items.add(index_val) }
    } else {
        unsafe {
            *self_.items.add(index_val) = value;
        }
        obj::CONST_NONE
    }
}

#[repr(C)]
struct ObjDequeIter {
    base: ObjBase,
    iternext: IterNextFn,
    deque: Obj,
    cur: usize,
}

fn deque_it_iternext(self_in: Obj) -> Obj {
    let self_ = unsafe { &*(obj::as_ptr(self_in) as *const ObjDequeIter) };
    let deque = unsafe { &*deque_ptr(self_.deque) };
    if self_.cur != deque.i_put {
        let o_out = unsafe { *deque.items.add(self_.cur) };
        let mut cur = self_.cur + 1;
        if cur == deque.alloc {
            cur = 0;
        }
        unsafe {
            (*(obj::as_ptr(self_in) as *mut ObjDequeIter)).cur = cur;
        }
        o_out
    } else {
        obj::OBJ_STOP_ITERATION
    }
}

fn new_deque_it(deque: Obj, iter_buf: &mut ObjIterBuf) -> Obj {
    debug_assert!(core::mem::size_of::<ObjDequeIter>() <= core::mem::size_of::<ObjIterBuf>());
    let deque_ = unsafe { &*deque_ptr(deque) };
    let o = unsafe { &mut *(iter_buf as *mut ObjIterBuf as *mut ObjDequeIter) };
    o.base.type_ = objpolyiter::type_polymorph_iter() as *const ObjType;
    o.iternext = deque_it_iternext;
    o.deque = deque;
    o.cur = deque_.i_get;
    obj::from_ptr(iter_buf as *const ObjIterBuf as *const ObjDequeIter as *const ())
}

fn deque_getiter(o_in: Obj, iter_buf: &mut ObjIterBuf) -> Obj {
    if !mpconfig::PY_COLLECTIONS_DEQUE_ITER {
        return obj::OBJ_NULL;
    }
    new_deque_it(o_in, iter_buf)
}

static mut DEQUE_SLOTS: [*const (); 5] = [
    deque_make_new as *const (),
    deque_unary_op as *const (),
    deque_subscr as *const (),
    deque_getiter as *const (),
    core::ptr::null(),
];

static mut TYPE_DEQUE: ObjType = ObjType {
    base: ObjBase { type_: core::ptr::null() },
    flags: TYPE_FLAG_ITER_IS_CUSTOM,
    name: 0,
    slot_index_make_new: 1,
    slot_index_print: 0,
    slot_index_call: 0,
    slot_index_unary_op: 2,
    slot_index_binary_op: 0,
    slot_index_attr: 0,
    slot_index_subscr: 3,
    slot_index_iter: 4,
    slot_index_buffer: 0,
    slot_index_protocol: 0,
    slot_index_parent: 0,
    slot_index_locals_dict: 5,
    slots: unsafe { DEQUE_SLOTS.as_ptr() },
};

static INIT: std::sync::OnceLock<()> = std::sync::OnceLock::new();

fn init_type() {
    INIT.get_or_init(|| {
        let table = vec![
            MapElem { key: obj::new_qstr(qstr::from_str("append")), value: new_fun2(deque_append) },
            MapElem { key: obj::new_qstr(qstr::from_str("appendleft")), value: new_fun2(deque_appendleft) },
            MapElem { key: obj::new_qstr(qstr::from_str("extend")), value: new_fun2(deque_extend) },
            MapElem { key: obj::new_qstr(qstr::from_str("pop")), value: new_fun1(deque_pop) },
            MapElem { key: obj::new_qstr(qstr::from_str("popleft")), value: new_fun1(deque_popleft) },
        ];
        let ptr = obj::malloc_helper(core::mem::size_of::<ObjDict>(), objdict::type_dict()) as *mut ObjDict;
        unsafe {
            map::init_fixed_table(&mut (*ptr).map, table);
            DEQUE_SLOTS[4] = obj::from_ptr(ptr as *const ObjDict as *const ()).0 as *const ();
            TYPE_DEQUE.name = qstr::from_str("deque");
            if !mpconfig::PY_COLLECTIONS_DEQUE_ITER {
                TYPE_DEQUE.flags = 0;
                TYPE_DEQUE.slot_index_iter = 0;
            }
            if !mpconfig::PY_COLLECTIONS_DEQUE_SUBSCR {
                TYPE_DEQUE.slot_index_subscr = 0;
            }
        }
    });
}

pub fn type_deque() -> &'static ObjType {
    if !mpconfig::PY_COLLECTIONS_DEQUE {
        panic!("deque disabled");
    }
    init_type();
    unsafe { &TYPE_DEQUE }
}
