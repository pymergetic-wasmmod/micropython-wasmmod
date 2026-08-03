//! rewrite of py/obj.h + py/obj.c (core tagged-object model; REPR_A)
// symmetry: done

use crate::gc;
use crate::malloc;
use crate::map::{self, Map};
use crate::mpconfig;
use crate::mpprint::{self, Print, PrintKind};
use crate::objbool;
use crate::objexcept;
use crate::objint;
use crate::objlist;
use crate::objnone;
use crate::objstr;
use crate::objtuple;
use crate::objtype;
use crate::qstr::{self, Qstr};
use crate::raise::{self, MpRaise};
use crate::runtime;
use crate::runtime0::{BinaryOp, UnaryOp};
use crate::smallint;

/// Opaque MicroPython object word (`mp_obj_t`).
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct Obj(pub usize);

pub type Int = isize;
pub type Uint = usize;

/// Concrete heap object header (`mp_obj_base_t`).
#[repr(C)]
#[derive(Copy, Clone)]
pub struct ObjBase {
    pub type_: *const ObjType,
}

/// Type flags (`MP_TYPE_FLAG_*`).
pub const TYPE_FLAG_NONE: u16 = 0x0000;
pub const TYPE_FLAG_IS_SUBCLASSED: u16 = 0x0001;
pub const TYPE_FLAG_HAS_SPECIAL_ACCESSORS: u16 = 0x0002;
pub const TYPE_FLAG_EQ_NOT_REFLEXIVE: u16 = 0x0004;
pub const TYPE_FLAG_EQ_CHECKS_OTHER_TYPE: u16 = 0x0008;
pub const TYPE_FLAG_EQ_HAS_NEQ_TEST: u16 = 0x0010;
pub const TYPE_FLAG_BINDS_SELF: u16 = 0x0020;
pub const TYPE_FLAG_BUILTIN_FUN: u16 = 0x0040;
pub const TYPE_FLAG_ITER_IS_ITERNEXT: u16 = 0x0080;
pub const TYPE_FLAG_ITER_IS_CUSTOM: u16 = 0x0100;
pub const TYPE_FLAG_ITER_IS_STREAM: u16 = TYPE_FLAG_ITER_IS_ITERNEXT | TYPE_FLAG_ITER_IS_CUSTOM;
pub const TYPE_FLAG_INSTANCE_TYPE: u16 = 0x0200;
pub const TYPE_FLAG_SUBSCR_ALLOWS_STACK_SLICE: u16 = 0x0400;

pub type PrintFn = fn(&Print, Obj, PrintKind);
pub type MakeNewFn = fn(&'static ObjType, usize, usize, &[Obj]) -> Obj;
pub type CallFn = fn(Obj, usize, usize, &[Obj]) -> Obj;
pub type UnaryOpFn = fn(UnaryOp, Obj) -> Obj;
pub type BinaryOpFn = fn(BinaryOp, Obj, Obj) -> Obj;
pub type AttrFn = fn(Obj, Qstr, &mut [Obj; 2]);
pub type SubscrFn = fn(Obj, Obj, Obj) -> Obj;
pub type GetIterFn = fn(Obj, *mut ObjIterBuf) -> Obj;
pub type IterNextFn = fn(Obj) -> Obj;
pub type BufferFn = fn(Obj, &mut BufferInfo, u32) -> Int;

/// Variable-length type object (`mp_obj_type_t`).
#[repr(C)]
#[derive(Copy, Clone)]
pub struct ObjType {
    pub base: ObjBase,
    pub flags: u16,
    pub name: Qstr,
    pub slot_index_make_new: u8,
    pub slot_index_print: u8,
    pub slot_index_call: u8,
    pub slot_index_unary_op: u8,
    pub slot_index_binary_op: u8,
    pub slot_index_attr: u8,
    pub slot_index_subscr: u8,
    pub slot_index_iter: u8,
    pub slot_index_buffer: u8,
    pub slot_index_protocol: u8,
    pub slot_index_parent: u8,
    pub slot_index_locals_dict: u8,
    pub slots: *const *const (),
}

/// Iterator scratch buffer (`mp_obj_iter_buf_t`).
#[repr(C)]
pub struct ObjIterBuf {
    pub base: ObjBase,
    pub buf: [Obj; 3],
}

pub const ITER_BUF_NSLOTS: usize = (std::mem::size_of::<ObjIterBuf>() + std::mem::size_of::<Obj>()
    - 1)
    / std::mem::size_of::<Obj>();

/// Buffer protocol descriptor (`mp_buffer_info_t`).
#[derive(Debug, Default)]
pub struct BufferInfo {
    pub buf: *mut u8,
    pub len: usize,
    pub typecode: i32,
}

impl BufferInfo {
    /// Safe view of buffer bytes. Empty / null buffers yield `&[]` (empty
    /// `bytearray`/`bytes` use a null `items` pointer with `len == 0`).
    pub fn as_bytes(&self) -> &[u8] {
        if self.len == 0 || self.buf.is_null() {
            &[]
        } else {
            unsafe { std::slice::from_raw_parts(self.buf as *const u8, self.len) }
        }
    }

    pub fn as_bytes_mut(&mut self) -> &mut [u8] {
        if self.len == 0 || self.buf.is_null() {
            &mut []
        } else {
            unsafe { std::slice::from_raw_parts_mut(self.buf, self.len) }
        }
    }
}

pub const BUFFER_READ: u32 = 1;
pub const BUFFER_WRITE: u32 = 2;
pub const BUFFER_RW: u32 = BUFFER_READ | BUFFER_WRITE;
pub const BUFFER_RAISE_IF_UNSUPPORTED: u32 = 4;

unsafe impl Send for ObjType {}
unsafe impl Sync for ObjType {}
unsafe impl Send for ObjBase {}
unsafe impl Sync for ObjBase {}

pub const OBJ_NULL: Obj = Obj(0);
pub const OBJ_STOP_ITERATION: Obj = Obj(0);
pub const OBJ_SENTINEL: Obj = Obj(4);

#[inline]
pub fn from_ptr(p: *const ()) -> Obj {
    Obj(p as usize)
}

#[inline]
pub fn to_ptr(o: Obj) -> *const () {
    o.0 as *const ()
}

#[inline]
pub fn as_ptr(o: Obj) -> *const () {
    to_ptr(o)
}

// --- REPR_A tagging ---------------------------------------------------------

#[inline]
pub const fn is_small_int(o: Obj) -> bool {
    debug_assert!(mpconfig::OBJ_REPR == mpconfig::OBJ_REPR_A);
    (o.0 as Int & 1) != 0
}

#[inline]
pub const fn small_int_value(o: Obj) -> Int {
    (o.0 as Int) >> 1
}

#[inline]
pub const fn new_small_int(v: Int) -> Obj {
    Obj((((v as Uint) << 1) | 1) as usize)
}

#[inline]
pub const fn is_qstr(o: Obj) -> bool {
    (o.0 as Int & 7) == 2
}

#[inline]
pub const fn qstr_value(o: Obj) -> usize {
    (o.0 as Uint) >> 3
}

#[inline]
pub const fn new_qstr(q: usize) -> Obj {
    Obj(((q << 3) | 2) as usize)
}

#[inline]
pub const fn is_immediate(o: Obj) -> bool {
    (o.0 as Int & 7) == 6
}

#[inline]
pub const fn immediate_value(o: Obj) -> usize {
    (o.0 as Uint) >> 3
}

#[inline]
pub const fn new_immediate(v: usize) -> Obj {
    Obj(((v << 3) | 6) as usize)
}

pub const CONST_NONE: Obj = new_immediate(0);
pub const CONST_FALSE: Obj = new_immediate(1);
pub const CONST_TRUE: Obj = new_immediate(3);

#[inline]
pub const fn is_obj(o: Obj) -> bool {
    (o.0 as Int & 3) == 0
}

pub const WORD_MSBIT_HIGH: Uint = 1usize << (usize::BITS - 1);

#[inline]
pub fn new_bool(x: bool) -> Obj {
    if x {
        CONST_TRUE
    } else {
        CONST_FALSE
    }
}

#[inline]
pub fn is_bool(o: Obj) -> bool {
    debug_assert!(mpconfig::OBJ_IMMEDIATE_OBJS);
    o == CONST_FALSE || o == CONST_TRUE
}

#[inline]
pub fn bool_value(o: Obj) -> bool {
    o != CONST_FALSE
}

/// Create an integer object, promoting to heap int when needed (`mp_obj_new_int`).
#[inline]
pub fn new_int(v: Int) -> Obj {
    objint::new_int(v)
}

// --- type slot helpers ------------------------------------------------------

#[inline]
pub fn type_has_slot(t: &ObjType, index: u8) -> bool {
    index != 0
}

#[inline]
unsafe fn slot_ptr(t: &ObjType, index: u8) -> *const () {
    debug_assert!(index > 0);
    *t.slots.add((index - 1) as usize)
}

pub fn type_get_print(t: &ObjType) -> Option<PrintFn> {
    if type_has_slot(t, t.slot_index_print) {
        Some(unsafe { std::mem::transmute(slot_ptr(t, t.slot_index_print)) })
    } else {
        None
    }
}

pub fn type_get_make_new(t: &ObjType) -> Option<MakeNewFn> {
    if type_has_slot(t, t.slot_index_make_new) {
        Some(unsafe { std::mem::transmute(slot_ptr(t, t.slot_index_make_new)) })
    } else {
        None
    }
}

pub fn type_get_call(t: &ObjType) -> Option<CallFn> {
    if type_has_slot(t, t.slot_index_call) {
        Some(unsafe { std::mem::transmute(slot_ptr(t, t.slot_index_call)) })
    } else {
        None
    }
}

pub fn type_get_unary_op(t: &ObjType) -> Option<UnaryOpFn> {
    if type_has_slot(t, t.slot_index_unary_op) {
        Some(unsafe { std::mem::transmute(slot_ptr(t, t.slot_index_unary_op)) })
    } else {
        None
    }
}

pub fn type_get_binary_op(t: &ObjType) -> Option<BinaryOpFn> {
    if type_has_slot(t, t.slot_index_binary_op) {
        Some(unsafe { std::mem::transmute(slot_ptr(t, t.slot_index_binary_op)) })
    } else {
        None
    }
}

pub fn type_get_attr(t: &ObjType) -> Option<AttrFn> {
    if type_has_slot(t, t.slot_index_attr) {
        Some(unsafe { std::mem::transmute(slot_ptr(t, t.slot_index_attr)) })
    } else {
        None
    }
}

pub fn type_get_subscr(t: &ObjType) -> Option<SubscrFn> {
    if type_has_slot(t, t.slot_index_subscr) {
        Some(unsafe { std::mem::transmute(slot_ptr(t, t.slot_index_subscr)) })
    } else {
        None
    }
}

pub fn type_get_buffer(t: &ObjType) -> Option<BufferFn> {
    if type_has_slot(t, t.slot_index_buffer) {
        Some(unsafe { std::mem::transmute(slot_ptr(t, t.slot_index_buffer)) })
    } else {
        None
    }
}

pub fn type_get_protocol(t: &ObjType) -> Option<*const ()> {
    if type_has_slot(t, t.slot_index_protocol) {
        Some(unsafe { slot_ptr(t, t.slot_index_protocol) })
    } else {
        None
    }
}

pub fn type_get_iter(t: &ObjType) -> Option<GetIterFn> {
    if type_has_slot(t, t.slot_index_iter) {
        if (t.flags & TYPE_FLAG_ITER_IS_CUSTOM) != 0 {
            type_get_iternext_custom(t).map(|c| c.getiter)
        } else {
            Some(unsafe { std::mem::transmute(slot_ptr(t, t.slot_index_iter)) })
        }
    } else {
        None
    }
}

/// Custom getiter/iternext slot pair (`mp_getiter_iternext_custom_t`).
#[repr(C)]
pub struct GetiterIternextCustom {
    pub getiter: GetIterFn,
    pub iternext: IterNextFn,
}

pub fn type_get_iternext_custom(t: &ObjType) -> Option<&'static GetiterIternextCustom> {
    if type_has_slot(t, t.slot_index_iter) && (t.flags & TYPE_FLAG_ITER_IS_CUSTOM) != 0 {
        Some(unsafe { &*(slot_ptr(t, t.slot_index_iter) as *const GetiterIternextCustom) })
    } else {
        None
    }
}

/// Native `__next__` implementation for types with iternext slots.
pub fn type_get_iternext_fn(t: &ObjType) -> Option<IterNextFn> {
    if !type_has_slot(t, t.slot_index_iter) {
        return None;
    }
    if (t.flags & TYPE_FLAG_ITER_IS_ITERNEXT) != 0 {
        Some(unsafe { std::mem::transmute(slot_ptr(t, t.slot_index_iter)) })
    } else if (t.flags & TYPE_FLAG_ITER_IS_CUSTOM) != 0 {
        Some(type_get_iternext_custom(t)?.iternext)
    } else {
        None
    }
}

#[inline]
pub fn type_has_slot_at(t: &ObjType, offset: usize) -> bool {
    let p = (t as *const ObjType as *const u8).wrapping_add(offset);
    unsafe { *p != 0 }
}

#[inline]
pub fn type_has_slot_by_offset(t: &ObjType, offset: usize) -> bool {
    type_has_slot_at(t, offset)
}

pub fn type_get_slot_parent(t: &ObjType) -> Option<Obj> {
    if type_has_slot(t, t.slot_index_parent) {
        Some(unsafe { std::mem::transmute::<_, Obj>(slot_ptr(t, t.slot_index_parent)) })
    } else {
        None
    }
}

pub fn type_get_slot_locals_dict(t: &ObjType) -> Option<Obj> {
    if type_has_slot(t, t.slot_index_locals_dict) {
        Some(unsafe { std::mem::transmute::<_, Obj>(slot_ptr(t, t.slot_index_locals_dict)) })
    } else {
        None
    }
}

pub enum SlotKind {
    MakeNew,
    Print,
    Call,
    UnaryOp,
    BinaryOp,
    Attr,
    Subscr,
    Iter,
    Buffer,
    Protocol,
    Parent,
    LocalsDict,
}

pub fn type_set_slot(t: &mut ObjType, kind: SlotKind, value: Obj, n: u8) {
    type_set_slot_ptr(t, kind, value.0 as *const (), n);
}

pub fn type_set_slot_fn(t: &mut ObjType, kind: SlotKind, value: *const (), n: u8) {
    type_set_slot_ptr(t, kind, value, n);
}

fn type_set_slot_ptr(t: &mut ObjType, kind: SlotKind, value: *const (), n: u8) {
    let idx = n + 1;
    match kind {
        SlotKind::MakeNew => t.slot_index_make_new = idx,
        SlotKind::Print => t.slot_index_print = idx,
        SlotKind::Call => t.slot_index_call = idx,
        SlotKind::UnaryOp => t.slot_index_unary_op = idx,
        SlotKind::BinaryOp => t.slot_index_binary_op = idx,
        SlotKind::Attr => t.slot_index_attr = idx,
        SlotKind::Subscr => t.slot_index_subscr = idx,
        SlotKind::Iter => t.slot_index_iter = idx,
        SlotKind::Buffer => t.slot_index_buffer = idx,
        SlotKind::Protocol => t.slot_index_protocol = idx,
        SlotKind::Parent => t.slot_index_parent = idx,
        SlotKind::LocalsDict => t.slot_index_locals_dict = idx,
    }
    unsafe {
        let slots = t.slots as *mut *const ();
        if !slots.is_null() {
            *slots.add(n as usize) = value;
        }
    }
}

/// Zero-initialised ROM type shell for singleton/builtin types.
pub const fn empty_type(name: Qstr) -> ObjType {
    ObjType {
        base: ObjBase {
            type_: core::ptr::null(),
        },
        flags: TYPE_FLAG_NONE,
        name,
        slot_index_make_new: 0,
        slot_index_print: 0,
        slot_index_call: 0,
        slot_index_unary_op: 0,
        slot_index_binary_op: 0,
        slot_index_attr: 0,
        slot_index_subscr: 0,
        slot_index_iter: 0,
        slot_index_buffer: 0,
        slot_index_protocol: 0,
        slot_index_parent: 0,
        slot_index_locals_dict: 0,
        slots: core::ptr::null(),
    }
}

pub fn is_instance_type(t: &ObjType) -> bool {
    (t.flags & TYPE_FLAG_INSTANCE_TYPE) != 0
}

pub fn is_native_type(t: &ObjType) -> bool {
    !is_instance_type(t)
}

// --- type tests -------------------------------------------------------------

pub fn is_exact_type(o: Obj, t: &'static ObjType) -> bool {
    is_obj(o) && unsafe { (*(as_ptr(o) as *const ObjBase)).type_ } == t as *const ObjType
}

pub fn is_type(o: Obj, t: &'static ObjType) -> bool {
    debug_assert!(t as *const ObjType != type_bool() as *const ObjType);
    debug_assert!(t as *const ObjType != type_int() as *const ObjType);
    debug_assert!(t as *const ObjType != type_none() as *const ObjType);
    is_exact_type(o, t)
}

pub fn is_int(o: Obj) -> bool {
    is_small_int(o) || is_exact_type(o, type_int())
}

pub fn is_str(o: Obj) -> bool {
    is_qstr(o) || is_exact_type(o, type_str())
}

pub fn is_str_or_bytes(o: Obj) -> bool {
    is_qstr(o) || is_exact_type(o, type_str()) || is_exact_type(o, objstr::type_bytes())
}

pub fn is_dict_or_ordereddict(o: Obj) -> bool {
    objdict::is_dict_or_ordereddict(o)
}

// --- well-known types -------------------------------------------------------

pub fn type_none() -> &'static ObjType {
    objnone::type_none()
}
pub fn type_bool() -> &'static ObjType {
    objbool::type_bool()
}
pub fn type_int() -> &'static ObjType {
    objint::type_int()
}
pub fn type_str() -> &'static ObjType {
    objstr::type_str()
}
pub fn type_type() -> &'static ObjType {
    objtype::type_type()
}
pub fn type_object() -> &'static ObjType {
    objtype::type_object()
}
pub fn type_tuple() -> &'static ObjType {
    objtuple::type_tuple()
}
pub fn type_list() -> &'static ObjType {
    objlist::type_list()
}

// --- allocation -------------------------------------------------------------

pub fn malloc_helper(num_bytes: usize, type_: &'static ObjType) -> *mut ObjBase {
    let base =
        gc::alloc(num_bytes, std::mem::align_of::<ObjBase>()).expect("gc alloc") as *mut ObjBase;
    unsafe {
        (*base).type_ = type_ as *const ObjType;
    }
    base
}

pub fn malloc_var<T>(extra: usize, type_: &'static ObjType) -> *mut T {
    let size = std::mem::size_of::<T>() + extra;
    malloc_helper(size, type_) as *mut T
}

// --- get_type -------------------------------------------------------------

pub fn get_type(o: Obj) -> &'static ObjType {
    if mpconfig::OBJ_IMMEDIATE_OBJS && mpconfig::OBJ_REPR == mpconfig::OBJ_REPR_A {
        if is_obj(o) {
            let type_ptr = unsafe { (*(as_ptr(o) as *const ObjBase)).type_ };
            if type_ptr.is_null() {
                // Const `mp_obj_type_t` values use a null base.type until type_type init.
                return type_type();
            }
            unsafe { &*type_ptr }
        } else {
            match o.0 & 0xf {
                2 | 10 => type_str(),
                6 => type_none(),
                14 => type_bool(),
                _ => type_int(),
            }
        }
    } else if is_small_int(o) {
        type_int()
    } else if is_qstr(o) {
        type_str()
    } else if mpconfig::OBJ_IMMEDIATE_OBJS && is_immediate(o) {
        if immediate_value(o) & 1 == 0 {
            type_none()
        } else {
            type_bool()
        }
    } else {
        let type_ptr = unsafe { (*(as_ptr(o) as *const ObjBase)).type_ };
        if type_ptr.is_null() {
            return type_type();
        }
        unsafe { &*type_ptr }
    }
}

pub fn get_type_str(o: Obj) -> String {
    qstr::str_from_qstr(get_type(o).name).unwrap_or_else(|| "?".into())
}

// --- print ------------------------------------------------------------------

pub fn print_helper(print: &Print, o: Obj, kind: PrintKind) {
    if o == OBJ_NULL {
        mpprint::print_str(print, "(nil)");
        return;
    }
    let t = get_type(o);
    if let Some(print_fn) = type_get_print(t) {
        print_fn(print, o, kind);
    } else {
        let _ = mpprint::printf(print, "<%q>", std::iter::once(mpprint::VaArg::Qstr(t.name)));
    }
}

pub fn print(o: Obj, kind: PrintKind) {
    print_helper(&mpprint::PLAT_PRINT, o, kind);
}

pub fn print_exception(print: &Print, exc: Obj) {
    if objexcept::is_exception_instance(exc) {
        let mut n = 0usize;
        let mut values = core::ptr::null_mut();
        objexcept::exception_get_traceback(exc, &mut n, &mut values);
        if n > 0 {
            debug_assert!(n % 3 == 0);
            mpprint::print_str(print, "Traceback (most recent call last):\n");
            let mut i = n as isize - 3;
            while i >= 0 {
                unsafe {
                    let file = *values.add(i as usize);
                    let line = *values.add(i as usize + 1);
                    let block = *values.add(i as usize + 2);
                    if mpconfig::ENABLE_SOURCE_LINE {
                        mpprint::printf(
                            print,
                            "  File \"%q\", line %d",
                            [mpprint::VaArg::Qstr(file), mpprint::VaArg::Int(line as i32)],
                        );
                    } else {
                        mpprint::printf(print, "  File \"%q\"", [mpprint::VaArg::Qstr(file)]);
                    }
                    if block == qstr::QSTR_NULL {
                        mpprint::print_str(print, "\n");
                    } else {
                        mpprint::printf(print, ", in %q\n", [mpprint::VaArg::Qstr(block)]);
                    }
                }
                i -= 3;
            }
        }
    }
    print_helper(print, exc, PrintKind::Exc);
    mpprint::print_str(print, "\n");
}

// --- truthiness / equality --------------------------------------------------

pub fn is_true(o: Obj) -> bool {
    if o == CONST_FALSE || o == CONST_NONE {
        return false;
    }
    if o == CONST_TRUE {
        return true;
    }
    if is_small_int(o) {
        return small_int_value(o) != 0;
    }
    let t = get_type(o);
    if let Some(unary) = type_get_unary_op(t) {
        let result = unary(UnaryOp::Bool, o);
        if result != OBJ_NULL {
            return result == CONST_TRUE;
        }
    }
    if let Some(len) = len_maybe(o) {
        return len != new_small_int(0);
    }
    true
}

pub fn is_callable(o: Obj) -> bool {
    type_get_call(get_type(o)).is_some()
}

pub fn equal_not_equal(op: BinaryOp, o1: Obj, o2: Obj) -> Obj {
    let local_true = if op == BinaryOp::NotEqual {
        CONST_FALSE
    } else {
        CONST_TRUE
    };
    let local_false = if op == BinaryOp::NotEqual {
        CONST_TRUE
    } else {
        CONST_FALSE
    };

    if o1 == o2 && (is_small_int(o1) || (get_type(o1).flags & TYPE_FLAG_EQ_NOT_REFLEXIVE) == 0) {
        return local_true;
    }

    if is_str(o1) {
        if is_str(o2) {
            return if objstr::str_equal(o1, o2) {
                local_true
            } else {
                local_false
            };
        }
        return local_false;
    }

    if is_small_int(o1) && is_small_int(o2) {
        return local_false;
    }

    let mut pass_number = 0;
    let mut o1 = o1;
    let mut o2 = o2;
    while pass_number < 2 {
        let t = get_type(o1);
        if let Some(binary) = type_get_binary_op(t) {
            if (t.flags & TYPE_FLAG_EQ_CHECKS_OTHER_TYPE) != 0
                || get_type(o2) as *const ObjType == t as *const ObjType
            {
                if op == BinaryOp::NotEqual && (t.flags & TYPE_FLAG_EQ_HAS_NEQ_TEST) != 0 {
                    let r = binary(BinaryOp::NotEqual, o1, o2);
                    if r != OBJ_NULL {
                        return r;
                    }
                }
                let r = binary(BinaryOp::Equal, o1, o2);
                if r != OBJ_NULL {
                    return if op == BinaryOp::Equal {
                        r
                    } else if is_true(r) {
                        local_true
                    } else {
                        local_false
                    };
                }
            }
        }
        pass_number += 1;
        std::mem::swap(&mut o1, &mut o2);
    }
    if o1 == o2 {
        local_true
    } else {
        local_false
    }
}

pub fn equal(o1: Obj, o2: Obj) -> bool {
    is_true(equal_not_equal(BinaryOp::Equal, o1, o2))
}

// --- numeric accessors --------------------------------------------------------

pub fn get_int(o: Obj) -> Int {
    let mut val = 0;
    if !get_int_maybe(o, &mut val) {
        raise::raise(MpRaise::TypeError("can't convert to int"));
    }
    val
}

pub fn get_int_maybe(o: Obj, value: &mut Int) -> bool {
    if o == CONST_FALSE {
        *value = 0;
    } else if o == CONST_TRUE {
        *value = 1;
    } else if is_small_int(o) {
        *value = small_int_value(o);
    } else if is_exact_type(o, type_int()) {
        *value = objint::int_get_checked(o);
    } else {
        let converted = runtime::unary_op_obj(UnaryOp::IntMaybe, o);
        if converted == OBJ_NULL {
            return false;
        }
        *value = objint::int_get_checked(converted);
    }
    true
}

pub fn get_int_truncated(o: Obj) -> Int {
    if is_int(o) {
        objint::int_get_truncated(o)
    } else {
        get_int(o)
    }
}

pub fn get_uint(o: Obj) -> Uint {
    if !is_exact_type(o, type_int()) {
        let as_int = runtime::unary_op_obj(UnaryOp::IntMaybe, o);
        if as_int == OBJ_NULL {
            raise::raise(MpRaise::TypeError("can't convert to int"));
        }
        return objint::int_get_uint_checked(as_int);
    }
    objint::int_get_uint_checked(o)
}

// --- len / subscr / buffer --------------------------------------------------

pub fn len_maybe(o: Obj) -> Option<Obj> {
    if is_str(o) || is_exact_type(o, objstr::type_bytes()) {
        Some(new_small_int(objstr::str_len(o) as Int))
    } else if let Some(unary) = type_get_unary_op(get_type(o)) {
        let r = unary(UnaryOp::Len, o);
        if r == OBJ_NULL {
            None
        } else {
            Some(r)
        }
    } else {
        None
    }
}

pub fn len(o: Obj) -> Obj {
    len_maybe(o).unwrap_or_else(|| raise::raise(MpRaise::TypeError("object has no len")))
}

pub fn subscr(base: Obj, index: Obj, value: Obj) -> Obj {
    let t = get_type(base);
    if let Some(subscr_fn) = type_get_subscr(t) {
        let ret = subscr_fn(base, index, value);
        if ret != OBJ_NULL {
            return ret;
        }
    }
    if value == OBJ_NULL {
        raise::raise(MpRaise::TypeError("object doesn't support item deletion"));
    } else if value == OBJ_SENTINEL {
        raise::raise(MpRaise::TypeError("object isn't subscriptable"));
    } else {
        raise::raise(MpRaise::TypeError("object doesn't support item assignment"));
    }
}

pub fn get_buffer(o: Obj, bufinfo: &mut BufferInfo, flags: u32) -> bool {
    if let Some(buffer) = type_get_buffer(get_type(o)) {
        if buffer(o, bufinfo, flags & BUFFER_RW) == 0 {
            return true;
        }
    }
    if flags & BUFFER_RAISE_IF_UNSUPPORTED != 0 {
        raise::raise(MpRaise::TypeError("object with buffer protocol required"));
    }
    false
}

pub fn get_buffer_raise(o: Obj, bufinfo: &mut BufferInfo, flags: u32) {
    get_buffer(o, bufinfo, flags | BUFFER_RAISE_IF_UNSUPPORTED);
}

pub fn identity(self_: Obj) -> Obj {
    self_
}

pub fn id(o: Obj) -> Obj {
    let id = o.0 as Int;
    if !is_obj(o) {
        new_small_int(id)
    } else if id >= 0 {
        new_small_int(id)
    } else {
        objint::new_int_from_uint(id as Uint)
    }
}

pub fn get_index(type_: &ObjType, len: usize, index: Obj, is_slice: bool) -> usize {
    let i = if is_small_int(index) {
        small_int_value(index)
    } else {
        let mut tmp = 0;
        if !get_int_maybe(index, &mut tmp) {
            raise::raise(MpRaise::TypeError("indices must be integers"));
        }
        tmp
    };
    let mut i = i;
    if i < 0 {
        i += len as Int;
    }
    if is_slice {
        if i < 0 {
            i = 0;
        } else if i as usize > len {
            i = len as Int;
        }
    } else if i < 0 || i as usize >= len {
        raise::raise(MpRaise::IndexError("index out of range"));
    }
    i as usize
}

pub fn get_array(o: Obj) -> (usize, Vec<Obj>) {
    if is_exact_type(o, type_tuple()) {
        objtuple::tuple_get(o)
    } else if is_exact_type(o, type_list()) {
        objlist::list_get(o)
    } else {
        raise::raise(MpRaise::TypeError("expected tuple/list"));
    }
}

pub fn debug_str(o: Obj) -> String {
    if is_small_int(o) {
        return small_int_value(o).to_string();
    }
    if is_qstr(o) {
        return qstr::str_from_qstr(qstr_value(o))
            .unwrap_or_else(|| format!("qstr({})", qstr_value(o)));
    }
    if o == OBJ_NULL {
        return "null".into();
    }
    format!("obj({:#x})", o.0)
}

// Forwarding module refs
mod objdict {
    pub use crate::objdict::*;
}
