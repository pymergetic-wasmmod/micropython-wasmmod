//! rewrite of py/objtype.c + py/objtype.h
// symmetry: done

use core::mem::{offset_of, size_of};

use crate::argcheck;
use crate::map::{self, Map, MapElem};
use crate::mpconfig;
use crate::mpprint::{self, Print, PrintKind, VaArg};
use crate::obj::{
    self, AttrFn, BinaryOpFn, BufferFn, BufferInfo, CallFn, GetIterFn, Int, MakeNewFn, Obj,
    ObjBase, ObjIterBuf, ObjType, PrintFn, SubscrFn, UnaryOpFn, OBJ_SENTINEL, TYPE_FLAG_BINDS_SELF,
    TYPE_FLAG_BUILTIN_FUN, TYPE_FLAG_EQ_CHECKS_OTHER_TYPE, TYPE_FLAG_EQ_HAS_NEQ_TEST,
    TYPE_FLAG_EQ_NOT_REFLEXIVE, TYPE_FLAG_HAS_SPECIAL_ACCESSORS, TYPE_FLAG_INSTANCE_TYPE,
    TYPE_FLAG_IS_SUBCLASSED, TYPE_FLAG_ITER_IS_ITERNEXT,
};
use crate::objdict::{self, ObjDict};
use crate::objfun;
use crate::objproperty;
use crate::objstr;
use crate::objtuple;
use crate::qstr::{self, Qstr};
use crate::raise::{self, MpRaise};
use crate::runtime::{self, RuntimeError};
use crate::runtime0::{BinaryOp, UnaryOp};

const ENABLE_SPECIAL_ACCESSORS: bool =
    mpconfig::PY_DESCRIPTORS || mpconfig::PY_DELATTR_SETATTR || mpconfig::PY_BUILTINS_PROPERTY;

pub const OFFSETOF_SLOT_MAKE_NEW: usize = offset_of!(ObjType, slot_index_make_new);
pub const OFFSETOF_SLOT_PRINT: usize = offset_of!(ObjType, slot_index_print);
pub const OFFSETOF_SLOT_CALL: usize = offset_of!(ObjType, slot_index_call);
pub const OFFSETOF_SLOT_UNARY_OP: usize = offset_of!(ObjType, slot_index_unary_op);
pub const OFFSETOF_SLOT_BINARY_OP: usize = offset_of!(ObjType, slot_index_binary_op);
pub const OFFSETOF_SLOT_ATTR: usize = offset_of!(ObjType, slot_index_attr);
pub const OFFSETOF_SLOT_SUBSCR: usize = offset_of!(ObjType, slot_index_subscr);
pub const OFFSETOF_SLOT_ITER: usize = offset_of!(ObjType, slot_index_iter);
pub const OFFSETOF_SLOT_BUFFER: usize = offset_of!(ObjType, slot_index_buffer);

// Native base init placeholder (mp_native_base_init_wrapper_obj).
static NATIVE_BASE_INIT_WRAPPER: Obj = Obj(0xdead_beef);

static mut TYPE_TYPE_SLOTS: [*const (); 4] = [
    instance_make_new as *const (),
    type_print as *const (),
    type_call as *const (),
    type_attr as *const (),
];

static TYPE_TYPE: ObjType = ObjType {
    base: ObjBase { type_: core::ptr::null() },
    flags: obj::TYPE_FLAG_NONE,
    name: 0,
    slot_index_make_new: 1,
    slot_index_print: 2,
    slot_index_call: 3,
    slot_index_unary_op: 0,
    slot_index_binary_op: 0,
    slot_index_attr: 4,
    slot_index_subscr: 0,
    slot_index_iter: 0,
    slot_index_buffer: 0,
    slot_index_protocol: 0,
    slot_index_parent: 0,
    slot_index_locals_dict: 0,
    slots: unsafe { TYPE_TYPE_SLOTS.as_ptr() },
};

static TYPE_OBJECT: ObjType = TYPE_TYPE;

static mut SUPER_SLOTS: [*const (); 3] = [
    super_make_new as *const (),
    super_print as *const (),
    super_attr as *const (),
];

static TYPE_SUPER: ObjType = ObjType {
    base: ObjBase { type_: core::ptr::null() },
    flags: obj::TYPE_FLAG_NONE,
    name: 0,
    slot_index_make_new: 1,
    slot_index_print: 2,
    slot_index_call: 0,
    slot_index_unary_op: 0,
    slot_index_binary_op: 0,
    slot_index_attr: 3,
    slot_index_subscr: 0,
    slot_index_iter: 0,
    slot_index_buffer: 0,
    slot_index_protocol: 0,
    slot_index_parent: 0,
    slot_index_locals_dict: 0,
    slots: unsafe { SUPER_SLOTS.as_ptr() },
};

static mut STATICMETHOD_SLOTS: [*const (); 1] = [static_class_method_make_new as *const ()];
static TYPE_STATICMETHOD: ObjType = ObjType {
    base: ObjBase { type_: core::ptr::null() },
    flags: obj::TYPE_FLAG_NONE,
    name: 0,
    slot_index_make_new: 1,
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
    slots: unsafe { STATICMETHOD_SLOTS.as_ptr() },
};

static mut CLASSMETHOD_SLOTS: [*const (); 1] = [static_class_method_make_new as *const ()];
static TYPE_CLASSMETHOD: ObjType = ObjType {
    base: ObjBase { type_: core::ptr::null() },
    flags: obj::TYPE_FLAG_NONE,
    name: 0,
    slot_index_make_new: 1,
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
    slots: unsafe { CLASSMETHOD_SLOTS.as_ptr() },
};

pub fn type_type() -> &'static ObjType { &TYPE_TYPE }
pub fn type_object() -> &'static ObjType { &TYPE_OBJECT }
pub fn type_super() -> &'static ObjType { &TYPE_SUPER }
pub fn type_staticmethod() -> &'static ObjType { &TYPE_STATICMETHOD }
pub fn type_classmethod() -> &'static ObjType { &TYPE_CLASSMETHOD }

#[repr(C)]
pub struct ObjInstance {
    pub base: ObjBase,
    pub members: Map,
}

pub unsafe fn subobj_ptr(o: *mut ObjInstance) -> *mut Obj {
    (o as *mut u8).add(size_of::<ObjInstance>()) as *mut Obj
}

pub unsafe fn subobj_get(o: *const ObjInstance, i: usize) -> Obj {
    *subobj_ptr(o as *mut ObjInstance).add(i)
}

pub unsafe fn subobj_set(o: *mut ObjInstance, i: usize, v: Obj) {
    *subobj_ptr(o).add(i) = v;
}

struct ClassLookupData {
    obj: Option<*mut ObjInstance>,
    attr: Qstr,
    slot_offset: usize,
    is_type: bool,
}

fn instance_count_native_bases(type_: &'static ObjType, last_native_base: &mut Option<&'static ObjType>) -> usize {
    let mut count = 0usize;
    let mut cur = type_;
    loop {
        if core::ptr::eq(cur, type_object()) {
            return count;
        }
        if obj::is_native_type(cur) {
            *last_native_base = Some(cur);
            return count + 1;
        }
        if !obj::type_has_slot(cur, cur.slot_index_parent) {
            return count;
        }
        if mpconfig::MULTIPLE_INHERITANCE {
            if let Some(parent) = obj::type_get_slot_parent(cur) {
                if obj::is_exact_type(parent, obj::type_tuple()) {
                    let (_, items) = objtuple::tuple_get(parent);
                    for item in items {
                        if !obj::is_exact_type(item, type_type()) { continue; }
                        let bt = unsafe { &*(obj::as_ptr(item) as *const ObjType) };
                        count += instance_count_native_bases(bt, last_native_base);
                    }
                    return count;
                }
            }
        }
        cur = obj::type_get_slot_parent(cur).map(|p| obj::get_type(p)).unwrap_or(type_object());
    }
}

fn class_lookup(lookup: &ClassLookupData, dest: &mut [Obj; 2], mut type_: &'static ObjType) {
    assert!(dest[0] == obj::OBJ_NULL);
    assert!(dest[1] == obj::OBJ_NULL);
    loop {
        if lookup.slot_offset != 0 && obj::is_native_type(type_) {
            if obj::type_has_slot_by_offset(type_, lookup.slot_offset)
                || (lookup.slot_offset == OFFSETOF_SLOT_ITER
                    && (type_.flags & obj::TYPE_FLAG_ITER_IS_STREAM) != 0)
            {
                dest[0] = OBJ_SENTINEL;
                return;
            }
        }
        if obj::type_has_slot(type_, type_.slot_index_locals_dict) {
            if let Some(dict_o) = obj::type_get_slot_locals_dict(type_) {
                let dict = unsafe { &*(obj::as_ptr(dict_o) as *const ObjDict) };
                let mut map_copy = dict.map.clone();
                if let Some(elem) = map::lookup(&mut map_copy, obj::new_qstr(lookup.attr), map::LookupKind::Lookup); m })
                    if lookup.is_type {
                        let org = unsafe { &*(lookup.obj.unwrap() as *const ObjType) };
                        { let v = elem.value; runtime::convert_member_lookup(obj::OBJ_NULL, org, v, dest); }
                    } else {
                        let inst = lookup.obj.unwrap();
                        let obj_obj = if obj::is_native_type(type_) && !core::ptr::eq(type_, type_object()) {
                            unsafe { subobj_get(inst, 0) }
                        } else {
                            obj::from_ptr(inst as *const ())
                        };
                        { let v = elem.value; runtime::convert_member_lookup(obj_obj, type_, v, dest); }
                    }
                    return;
                }
            }
        }
        if let Some(inst) = lookup.obj {
            if !lookup.is_type && obj::is_native_type(type_) && !core::ptr::eq(type_, type_object()) {
                let sub = unsafe { subobj_get(inst, 0) };
                runtime::load_method_maybe(sub, lookup.attr, dest);
                if dest[0] != obj::OBJ_NULL {
                    return;
                }
            }
        }
        if !obj::type_has_slot(type_, type_.slot_index_parent) {
            return;
        }
        if mpconfig::MULTIPLE_INHERITANCE {
            if let Some(parent) = obj::type_get_slot_parent(type_) {
                if obj::is_exact_type(parent, obj::type_tuple()) {
                    let (len, items) = objtuple::tuple_get(parent);
                    for i in 0..len.saturating_sub(1) {
                        let item = items[i];
                        if !obj::is_exact_type(item, type_type()) { continue; }
                        let bt = unsafe { &mut *(obj::as_ptr(item) as *mut ObjType) };
                        if core::ptr::eq(bt, type_object()) { continue; }
                        class_lookup(lookup, bt);
                        if dest[0] != obj::OBJ_NULL { return; }
                    }
                    if len > 0 {
                        let item = items[len - 1];
                        type_ = unsafe { &*(obj::as_ptr(item) as *const ObjType) };
                        continue;
                    }
                }
            }
        }
        type_ = obj::type_get_slot_parent(type_)
            .map(|p| obj::get_type(p))
            .unwrap_or(type_object());
        if core::ptr::eq(type_, type_object()) {
            return;
        }
    }
}

pub fn new_instance(class: &'static ObjType, native_base: &mut Option<&'static ObjType>) -> *mut ObjInstance {
    let count = instance_count_native_bases(class, native_base);
    assert!(count < 2);
    let o = obj::malloc_var::<ObjInstance>(count * size_of::<Obj>(), class) as *mut ObjInstance;
    unsafe {
        map::init(&mut (*o).members, 0);
        for i in 0..count {
            subobj_set(o, i, if count != 0 { NATIVE_BASE_INIT_WRAPPER } else { obj::OBJ_NULL });
        }
    }
    o
}

fn instance_print(print: &Print, self_in: Obj, kind: PrintKind) {
    let self_ptr = unsafe { &*(obj::as_ptr(self_in) as *const ObjInstance) };
    let meth = if kind == PrintKind::Str { qstr::from_str("__str__") } else { qstr::from_str("__repr__") };
    let mut member = [obj::OBJ_NULL; 2];
    let mut lookup = ClassLookupData {
        obj: Some(self_ptr as *const ObjInstance as *mut ObjInstance),
        attr: meth,
        slot_offset: OFFSETOF_SLOT_PRINT,
        is_type: false,
    };
    class_lookup(&lookup, dest, self_ptr.base.type_);
    if member[0] == obj::OBJ_NULL && kind == PrintKind::Str {
        lookup.attr = qstr::from_str("__repr__");
        lookup.slot_offset = 0;
        class_lookup(&lookup, dest, self_ptr.base.type_);
    }
    if member[0] == OBJ_SENTINEL {
        let sub = unsafe { subobj_get(self_ptr, 0) };
        obj::print_helper(print, sub, kind);
        return;
    }
    if member[0] != obj::OBJ_NULL {
        let r = runtime::call_function_1(member[0], self_in);
        obj::print_helper(print, r, PrintKind::Str);
        return;
    }
    let _ = mpprint::printf(print, "<%s object>", std::iter::once(VaArg::Str(obj::get_type_str(self_in))));
}

fn instance_make_new(self_: &'static ObjType, n_args: usize, n_kw: usize, args: &[Obj]) -> Obj {
    assert!(obj::is_instance_type(self_));
    let mut init_fn = [obj::OBJ_NULL; 2];
    let mut lookup = ClassLookupData {
        obj: None,
        attr: qstr::from_str("__new__"),
        slot_offset: OFFSETOF_SLOT_MAKE_NEW,
        is_type: false,
    };
    class_lookup(&lookup, dest, self_);
    let mut native_base = None;
    let o: *mut ObjInstance;
    if init_fn[0] == obj::OBJ_NULL || init_fn[0] == OBJ_SENTINEL {
        o = new_instance(self_, &mut native_base);
    } else {
        let mut args2 = Vec::with_capacity(1 + n_args + 2 * n_kw);
        args2.push(obj::from_ptr(self_ as *const ObjType as *const ()));
        args2.extend_from_slice(args);
        let new_ret = runtime::call_function_n_kw(init_fn[0], n_args + 1, n_kw, &args2);
        if obj::get_type(new_ret) as *const ObjType != self_ as *const ObjType {
            return new_ret;
        }
        o = obj::as_ptr(new_ret) as *mut ObjInstance;
    }
    init_fn = [obj::OBJ_NULL; 2];
    lookup.obj = Some(o);
    lookup.attr = qstr::from_str("__init__");
    lookup.slot_offset = 0;
    class_lookup(&lookup, dest, self_);
    if init_fn[0] != obj::OBJ_NULL {
        let init_ret = if n_args == 0 && n_kw == 0 {
            runtime::call_method_n_kw(0, 0, &init_fn)
        } else {
            let mut buf = Vec::with_capacity(2 + n_args + 2 * n_kw);
            buf.extend_from_slice(&init_fn);
            buf.extend_from_slice(args);
            runtime::call_method_n_kw(n_args, n_kw, &buf)
        };
        if init_ret != obj::CONST_NONE {
            raise::raise(MpRaise::TypeError("__init__() should return None"));
        }
    }
    if let Some(nb) = native_base {
        unsafe {
            if subobj_get(o, 0) == NATIVE_BASE_INIT_WRAPPER {
                if let Some(make_new) = obj::type_get_make_new(nb) {
                    subobj_set(o, 0, make_new(nb, n_args, n_kw, args));
                }
            }
        }
    }
    unsafe {
        let hdr = o;
        (*hdr).slots = (hdr as *mut u8).add(std::mem::size_of::<ObjType>()) as *const *const ();
    }
    obj::from_ptr(o as *const ())
}

fn unary_op_method_name(op: UnaryOp) -> Qstr {
    match op {
        UnaryOp::Bool => qstr::from_str("__bool__"),
        UnaryOp::Len => qstr::from_str("__len__"),
        UnaryOp::Hash => qstr::from_str("__hash__"),
        UnaryOp::IntMaybe => qstr::from_str("__int__"),
        UnaryOp::Positive => qstr::from_str("__pos__"),
        UnaryOp::Negative => qstr::from_str("__neg__"),
        UnaryOp::Invert => qstr::from_str("__invert__"),
        UnaryOp::Abs => qstr::from_str("__abs__"),
        UnaryOp::FloatMaybe => qstr::from_str("__float__"),
        _ => 0,
    }
}

fn binary_op_method_name(op: BinaryOp) -> Qstr {
    match op {
        BinaryOp::Less => qstr::from_str("__lt__"),
        BinaryOp::More => qstr::from_str("__gt__"),
        BinaryOp::Equal => qstr::from_str("__eq__"),
        BinaryOp::LessEqual => qstr::from_str("__le__"),
        BinaryOp::MoreEqual => qstr::from_str("__ge__"),
        BinaryOp::NotEqual => qstr::from_str("__ne__"),
        BinaryOp::Contains => qstr::from_str("__contains__"),
        BinaryOp::InplaceAdd => qstr::from_str("__iadd__"),
        BinaryOp::InplaceSubtract => qstr::from_str("__isub__"),
        BinaryOp::Add => qstr::from_str("__add__"),
        BinaryOp::Subtract => qstr::from_str("__sub__"),
        _ => 0,
    }
}

fn instance_unary_op(op: UnaryOp, self_in: Obj) -> Obj {
    let self_ptr = unsafe { &*(obj::as_ptr(self_in) as *const ObjInstance) };
    let op_name = unary_op_method_name(op);
    let mut member = [obj::OBJ_NULL; 2];
    let mut lookup = ClassLookupData {
        obj: Some(self_ptr as *const ObjInstance as *mut ObjInstance),
        attr: op_name,
        slot_offset: OFFSETOF_SLOT_UNARY_OP,
        is_type: false,
    };
    class_lookup(&lookup, dest, self_ptr.base.type_);
    if member[0] == OBJ_SENTINEL {
        return runtime::unary_op_obj(op, unsafe { subobj_get(self_ptr, 0) });
    }
    if member[0] != obj::OBJ_NULL {
        let val = runtime::call_function_1(member[0], self_in);
        return match op {
            UnaryOp::Hash => obj::new_small_int(obj::get_int_truncated(val)),
            UnaryOp::IntMaybe => {
                if !obj::is_int(val) { raise::raise(MpRaise::TypeError("bad int")); }
                val
            }
            _ => val,
        };
    }
    if op == UnaryOp::Hash {
        lookup.attr = qstr::from_str("__eq__");
        class_lookup(&lookup, dest, self_ptr.base.type_);
        if member[0] == obj::OBJ_NULL {
            return obj::new_small_int(self_in.0 as Int);
        }
    }
    obj::OBJ_NULL
}

fn instance_binary_op(op: BinaryOp, lhs_in: Obj, rhs_in: Obj) -> Obj {
    let lhs = unsafe { &*(obj::as_ptr(lhs_in) as *const ObjInstance) };
    let op_name = binary_op_method_name(op);
    let mut dest = [obj::OBJ_NULL; 3];
    let mut lookup = ClassLookupData {
        obj: Some(lhs as *const ObjInstance as *mut ObjInstance),
        attr: op_name,
        slot_offset: OFFSETOF_SLOT_BINARY_OP,
        is_type: false,
    };
    class_lookup(&lookup, dest, lhs.base.type_);
    let res = if dest[0] == OBJ_SENTINEL {
        runtime::binary_op_obj(op, unsafe { subobj_get(lhs, 0) }, rhs_in)
    } else if dest[0] != obj::OBJ_NULL {
        dest[2] = rhs_in;
        runtime::call_method_n_kw(1, 0, &dest)
    } else {
        return obj::OBJ_NULL;
    };
    if op == BinaryOp::Contains {
        obj::new_bool(obj::is_true(res))
    } else {
        res
    }
}

fn instance_load_attr(self_in: Obj, attr: Qstr, dest: &mut [Obj; 2]) {
    let self_ptr = unsafe { &mut *(obj::as_ptr(self_in) as *mut ObjInstance) };
    if let Some(elem) = map::lookup(&mut self_ptr.members, obj::new_qstr(attr), map::LookupKind::Lookup); m })
        dest[0] = elem.value;
        return;
    }
    let mut lookup = ClassLookupData {
        obj: Some(self_ptr),
        attr,
        slot_offset: 0,
        is_type: false,
    };
    class_lookup(&lookup, dest, unsafe { (*self_ptr).base.type_ });
    if dest[0] != obj::OBJ_NULL {
        let t = unsafe { &*(*self_ptr).base.type_ };
        if (t.flags & TYPE_FLAG_HAS_SPECIAL_ACCESSORS) == 0 {
            return;
        }
        if mpconfig::PY_BUILTINS_PROPERTY && objproperty::is_property(dest[0]) {
            let proxy = objproperty::get(dest[0]);
            if proxy[0] == obj::CONST_NONE {
                raise::raise(MpRaise::AttributeError("unreadable attribute"));
            }
            dest[0] = runtime::call_function_n_kw(proxy[0], 1, 0, &[self_in]);
            return;
        }
        if mpconfig::PY_DESCRIPTORS {
            let mut m = [obj::OBJ_NULL; 4];
            runtime::load_method_maybe(dest[0], qstr::from_str("__get__"), &mut m[..2]);
            if m[0] != obj::OBJ_NULL {
                m[2] = self_in;
                m[3] = obj::from_ptr(t as *const ObjType as *const ());
                dest[0] = runtime::call_method_n_kw(2, 0, &m);
            }
        }
        return;
    }
    if attr != qstr::from_str("__getattr__") {
        let mut d2 = [obj::OBJ_NULL; 3];
        runtime::load_method_maybe(self_in, qstr::from_str("__getattr__"), &mut d2[..2]);
        if d2[0] != obj::OBJ_NULL {
            d2[2] = obj::new_qstr(attr);
            dest[0] = runtime::call_method_n_kw(1, 0, &d2);
        }
    }
}

fn instance_store_attr(self_in: Obj, attr: Qstr, value: Obj) -> bool {
    let self_ptr = unsafe { &mut *(obj::as_ptr(self_in) as *mut ObjInstance) };
    let t = unsafe { &*(*self_ptr).base.type_ };
    if (t.flags & TYPE_FLAG_HAS_SPECIAL_ACCESSORS) != 0 {
        let mut member = [obj::OBJ_NULL; 2];
        let mut lookup = ClassLookupData {
            obj: Some(self_ptr),
            attr,
            slot_offset: 0,
            dest: &mut member,
            is_type: false,
        };
        class_lookup(&lookup, dest, t);
        if member[0] != obj::OBJ_NULL {
            if mpconfig::PY_BUILTINS_PROPERTY && objproperty::is_property(member[0]) {
                let proxy = objproperty::get(member[0]);
                let dest = [self_in, value];
                if value == obj::OBJ_NULL {
                    if proxy[2] == obj::CONST_NONE { return false; }
                    runtime::call_function_n_kw(proxy[2], 1, 0, &dest[..1]);
                    return true;
                }
                if proxy[1] == obj::CONST_NONE { return false; }
                runtime::call_function_n_kw(proxy[1], 2, 0, &dest);
                return true;
            }
        }
    }
    if value == obj::OBJ_NULL {
        map::lookup(&mut self_ptr.members, obj::new_qstr(attr), map::LookupKind::RemoveIfFound).is_some()
    } else {
        map::lookup(&mut self_ptr.members, obj::new_qstr(attr), map::LookupKind::AddIfNotFound).unwrap().value = value;
        true
    }
}

fn instance_attr(self_in: Obj, attr: Qstr, dest: &mut [Obj; 2]) {
    if dest[0] == obj::OBJ_NULL {
        instance_load_attr(self_in, attr, dest);
    } else if instance_store_attr(self_in, attr, dest[1]) {
        dest[0] = obj::OBJ_NULL;
    }
}

fn instance_subscr(self_in: Obj, index: Obj, value: Obj) -> Obj {
    let self_ptr = unsafe { &*(obj::as_ptr(self_in) as *const ObjInstance) };
    let (attr, slot) = if value == obj::OBJ_NULL {
        (qstr::from_str("__delitem__"), OFFSETOF_SLOT_SUBSCR)
    } else if value == OBJ_SENTINEL {
        (qstr::from_str("__getitem__"), OFFSETOF_SLOT_SUBSCR)
    } else {
        (qstr::from_str("__setitem__"), OFFSETOF_SLOT_SUBSCR)
    };
    let mut member = [obj::OBJ_NULL; 4];
    let mut lookup = ClassLookupData {
        obj: Some(self_ptr as *const ObjInstance as *mut ObjInstance),
        attr,
        slot_offset: slot,
        is_type: false,
    };
    class_lookup(&lookup, dest, self_ptr.base.type_);
    if member[0] == OBJ_SENTINEL {
        return obj::subscr(unsafe { subobj_get(self_ptr, 0) }, index, value);
    }
    if member[0] != obj::OBJ_NULL {
        member[2] = index;
        member[3] = value;
        let n = if value == obj::OBJ_NULL || value == OBJ_SENTINEL { 1 } else { 2 };
        let ret = runtime::call_method_n_kw(n, 0, &member);
        return if value == OBJ_SENTINEL { ret } else { obj::CONST_NONE };
    }
    obj::OBJ_NULL
}

fn instance_get_call(self_in: Obj, member: &mut [Obj; 2]) -> Obj {
    let self_ptr = unsafe { &*(obj::as_ptr(self_in) as *const ObjInstance) };
    let mut lookup = ClassLookupData {
        obj: Some(self_ptr as *const ObjInstance as *mut ObjInstance),
        attr: qstr::from_str("__call__"),
        slot_offset: OFFSETOF_SLOT_CALL,
        dest: member,
        is_type: false,
    };
    class_lookup(&lookup, dest, self_ptr.base.type_);
    member[0]
}

pub fn instance_is_callable(self_in: Obj) -> bool {
    let mut m = [obj::OBJ_NULL; 2];
    instance_get_call(self_in, &mut m) != obj::OBJ_NULL
}

pub fn instance_call(self_in: Obj, n_args: usize, n_kw: usize, args: &[Obj]) -> Obj {
    let mut member = [obj::OBJ_NULL; 2];
    let call = instance_get_call(self_in, &mut member);
    if call == obj::OBJ_NULL {
        raise::raise(MpRaise::TypeError("object not callable"));
    }
    if call == OBJ_SENTINEL {
        let self_ptr = unsafe { &*(obj::as_ptr(self_in) as *const ObjInstance) };
        return runtime::call_function_n_kw(unsafe { subobj_get(self_ptr, 0) }, n_args, n_kw, args);
    }
    runtime::call_method_self_n_kw(member[0], member[1], n_args, n_kw, args)
}

pub fn instance_getiter(self_in: Obj, iter_buf: *mut ObjIterBuf) -> Obj {
    let self_ptr = unsafe { &*(obj::as_ptr(self_in) as *const ObjInstance) };
    let mut member = [obj::OBJ_NULL; 2];
    let mut lookup = ClassLookupData {
        obj: Some(self_ptr as *const ObjInstance as *mut ObjInstance),
        attr: qstr::from_str("__iter__"),
        slot_offset: OFFSETOF_SLOT_ITER,
        is_type: false,
    };
    class_lookup(&lookup, dest, self_ptr.base.type_);
    if member[0] == obj::OBJ_NULL {
        obj::OBJ_NULL
    } else if member[0] == OBJ_SENTINEL {
        let sub = unsafe { subobj_get(self_ptr, 0) };
        let t = obj::get_type(sub);
        if (t.flags & TYPE_FLAG_ITER_IS_ITERNEXT) != 0 {
            sub
        } else {
            obj::OBJ_NULL
        }
    } else {
        runtime::call_method_n_kw(0, 0, &member)
    }
}

fn instance_get_buffer(self_in: Obj, bufinfo: &mut BufferInfo, flags: u32) -> Int {
    let self_ptr = unsafe { &*(obj::as_ptr(self_in) as *const ObjInstance) };
    let mut member = [obj::OBJ_NULL; 2];
    let mut lookup = ClassLookupData {
        obj: Some(self_ptr as *const ObjInstance as *mut ObjInstance),
        attr: 0,
        slot_offset: OFFSETOF_SLOT_BUFFER,
        is_type: false,
    };
    class_lookup(&lookup, dest, self_ptr.base.type_);
    if member[0] == OBJ_SENTINEL {
        let sub = unsafe { subobj_get(self_ptr, 0) };
        if let Some(buf) = obj::type_get_buffer(obj::get_type(sub)) {
            return buf(sub, bufinfo, flags);
        }
    }
    1
}

fn type_print(print: &Print, self_in: Obj, _kind: PrintKind) {
    let self_ = unsafe { &*(obj::as_ptr(self_in) as *const ObjType) };
    let _ = mpprint::printf(print, "<class '%q'>", std::iter::once(VaArg::Qstr(self_.name)));
}

fn type_make_new(_type_in: &'static ObjType, n_args: usize, n_kw: usize, args: &[Obj]) -> Obj {
    argcheck::check_num(n_args, n_kw, 1, 3, false);
    match n_args {
        1 => obj::from_ptr(obj::get_type(args[0]) as *const ()),
        3 => new_type(objstr::str_get_qstr(args[0]), args[1], args[2]),
        _ => raise::raise(MpRaise::TypeError("type takes 1 or 3 arguments")),
    }
}

fn type_call(self_in: Obj, n_args: usize, n_kw: usize, args: &[Obj]) -> Obj {
    let self_ = unsafe { &*(obj::as_ptr(self_in) as *const ObjType) };
    if let Some(make_new) = obj::type_get_make_new(self_) {
        make_new(self_, n_args, n_kw, args)
    } else {
        raise::raise(MpRaise::TypeError("can't create instance"));
    }
}

fn type_attr(self_in: Obj, attr: Qstr, dest: &mut [Obj; 2]) {
    let self_ = unsafe { &mut *(obj::as_ptr(self_in) as *mut ObjType) };
    if dest[0] == obj::OBJ_NULL {
        if mpconfig::CPYTHON_COMPAT && attr == qstr::from_str("__name__") {
            dest[0] = obj::new_qstr(self_.name);
            return;
        }
        let mut lookup = ClassLookupData {
            obj: Some(self_ as *mut ObjType as *mut ObjInstance),
            attr,
            slot_offset: 0,
            dest,
            is_type: true,
        };
        class_lookup(&lookup, dest, self_);
    } else if obj::type_has_slot(self_, self_.slot_index_locals_dict) {
        if let Some(dict_o) = obj::type_get_slot_locals_dict(self_) {
            let dict = unsafe { &mut *(obj::as_ptr(dict_o) as *mut ObjDict) };
            if dict.map.is_fixed {
                return;
            }
            if dest[1] == obj::OBJ_NULL {
                if map::lookup(&mut dict.map, obj::new_qstr(attr), map::LookupKind::RemoveIfFound).is_some() {
                    dest[0] = obj::OBJ_NULL;
                }
            } else {
                map::lookup(&mut dict.map, obj::new_qstr(attr), map::LookupKind::AddIfNotFound).unwrap().value = dest[1];
                dest[0] = obj::OBJ_NULL;
            }
        }
    }
}

fn new_type(name: Qstr, bases_tuple: Obj, locals_dict: Obj) -> Obj {
    if !obj::is_exact_type(bases_tuple, obj::type_tuple()) || !objdict::is_dict_or_ordereddict(locals_dict) {
        raise::raise(MpRaise::TypeError("bad type() args"));
    }
    let (bases_len, bases_items) = objtuple::tuple_get(bases_tuple);
    let mut base_flags = TYPE_FLAG_EQ_NOT_REFLEXIVE
        | TYPE_FLAG_EQ_CHECKS_OTHER_TYPE
        | TYPE_FLAG_EQ_HAS_NEQ_TEST
        | TYPE_FLAG_INSTANCE_TYPE;
    for i in 0..bases_len {
        let bt = unsafe { &*(obj::as_ptr(bases_items[i]) as *const ObjType) };
        if obj::type_get_make_new(bt).is_none() {
            raise::raise(MpRaise::TypeError("type isn't an acceptable base type"));
        }
        if obj::is_instance_type(bt) {
            base_flags |= bt.flags & TYPE_FLAG_HAS_SPECIAL_ACCESSORS;
        }
    }
    let slots_count = 10 + if bases_len > 0 { 1 } else { 0 };
    let layout = obj::malloc_var::<ObjType>(slots_count * size_of::<*const ()>(), type_type());
    let o = layout as *mut ObjType;
    unsafe {
        (*o).base.type_ = type_type() as *const ObjType;
        (*o).flags = base_flags;
        (*o).name = name;
        obj::type_set_slot_fn(&mut (*o).header, obj::SlotKind::MakeNew, instance_make_new as *const (), 0);
        obj::type_set_slot_fn(&mut (*o).header, obj::SlotKind::Print, instance_print as *const (), 1);
        obj::type_set_slot_fn(&mut (*o).header, obj::SlotKind::Call, instance_call as *const (), 2);
        obj::type_set_slot_fn(&mut (*o).header, obj::SlotKind::UnaryOp, instance_unary_op as *const (), 3);
        obj::type_set_slot_fn(&mut (*o).header, obj::SlotKind::BinaryOp, instance_binary_op as *const (), 4);
        obj::type_set_slot_fn(&mut (*o).header, obj::SlotKind::Attr, instance_attr as *const (), 5);
        obj::type_set_slot_fn(&mut (*o).header, obj::SlotKind::Subscr, instance_subscr as *const (), 6);
        obj::type_set_slot_fn(&mut (*o).header, obj::SlotKind::Iter, instance_getiter as *const (), 7);
        obj::type_set_slot_fn(&mut (*o).header, obj::SlotKind::Buffer, instance_get_buffer as *const (), 8);
        obj::type_set_slot(&mut (*o).header, obj::SlotKind::LocalsDict, locals_dict, 9);
        if bases_len >= 2 && mpconfig::MULTIPLE_INHERITANCE {
            obj::type_set_slot(&mut (*o).header, obj::SlotKind::Parent, bases_tuple, 10);
        } else if bases_len == 1 {
            obj::type_set_slot(&mut (*o).header, obj::SlotKind::Parent, bases_items[0], 10);
        }
    }
    unsafe {
        let hdr = o;
        (*hdr).slots = (hdr as *mut u8).add(std::mem::size_of::<ObjType>()) as *const *const ();
    }
    obj::from_ptr(o as *const ())
}

#[repr(C)]
pub struct ObjSuper {
    pub base: ObjBase,
    pub type_: Obj,
    pub obj: Obj,
}

fn super_print(print: &Print, self_in: Obj, _kind: PrintKind) {
    let self_ = unsafe { &*(obj::as_ptr(self_in) as *const ObjSuper) };
    mpprint::print_str(print, "<super: ");
    obj::print_helper(print, self_.type_, PrintKind::Str);
    mpprint::print_str(print, ", ");
    obj::print_helper(print, self_.obj, PrintKind::Str);
    mpprint::print_str(print, ">");
}

fn super_make_new(_type_in: &'static ObjType, n_args: usize, n_kw: usize, args: &[Obj]) -> Obj {
    argcheck::check_num(n_args, n_kw, 2, 2, false);
    let second_type = obj::get_type(args[1]);
    let second_obj = if core::ptr::eq(second_type, type_type()) { args[1] } else { obj::from_ptr(second_type as *const ObjType as *const ()) };
    if !is_subclass_fast(second_obj, args[0]) {
        raise::raise(MpRaise::TypeError("super(): bad args"));
    }
    let o = obj::malloc_helper(size_of::<ObjSuper>(), type_super()) as *mut ObjSuper;
    unsafe {
        (*o).type_ = args[0];
        (*o).obj = args[1];
    }
    unsafe {
        let hdr = o;
        (*hdr).slots = (hdr as *mut u8).add(std::mem::size_of::<ObjType>()) as *const *const ();
    }
    obj::from_ptr(o as *const ())
}

fn super_attr(self_in: Obj, attr: Qstr, dest: &mut [Obj; 2]) {
    if dest[0] != obj::OBJ_NULL {
        return;
    }
    let self_ = unsafe { &*(obj::as_ptr(self_in) as *const ObjSuper) };
    let type_ = unsafe { &*(obj::as_ptr(self_.type_) as *const ObjType) };
    let inst = obj::as_ptr(self_.obj) as *mut ObjInstance;
    let mut lookup = ClassLookupData {
        obj: Some(inst),
        attr,
        slot_offset: if attr == qstr::from_str("__init__") { OFFSETOF_SLOT_MAKE_NEW } else { 0 },
        is_type: false,
    };
    if obj::type_has_slot(type_, type_.slot_index_parent) {
        if mpconfig::MULTIPLE_INHERITANCE {
            if let Some(parent) = obj::type_get_slot_parent(type_) {
                if obj::is_exact_type(parent, obj::type_tuple()) {
                    let (len, items) = objtuple::tuple_get(parent);
                    for i in 0..len {
                        let bt = unsafe { &*(obj::as_ptr(items[i]) as *const ObjType) };
                        if core::ptr::eq(bt, type_object()) { continue; }
                        class_lookup(&lookup, dest, bt);
                        if dest[0] != obj::OBJ_NULL { break; }
                    }
                }
            }
        } else if let Some(parent) = obj::type_get_slot_parent(type_) {
            let pt = obj::get_type(parent);
            if !core::ptr::eq(pt, type_object()) {
                class_lookup(&lookup, dest, pt);
            }
        }
    }
    if dest[0] == OBJ_SENTINEL {
        dest[0] = NATIVE_BASE_INIT_WRAPPER;
        dest[1] = self_.obj;
    }
    if dest[0] == obj::OBJ_NULL {
        lookup.slot_offset = 0;
        class_lookup(&lookup, dest, type_object());
    }
}

pub fn load_super_method(attr: Qstr, dest: &mut [Obj; 3]) {
    let super_obj = ObjSuper {
        base: ObjBase { type_: type_super() as *const ObjType },
        type_: dest[1],
        obj: dest[2],
    };
    let mut d2 = [obj::OBJ_NULL; 2];
    super_attr(obj::from_ptr(&super_obj as *const ObjSuper as *const ()), attr, &mut d2);
    dest[0] = d2[0];
    dest[1] = d2[1];
}

pub fn is_subclass_fast(object: Obj, classinfo: Obj) -> bool {
    let mut cur = object;
    loop {
        if cur == classinfo {
            return true;
        }
        if !obj::is_exact_type(cur, type_type()) {
            return false;
        }
        let self_ = unsafe { &*(obj::as_ptr(cur) as *const ObjType) };
        if !obj::type_has_slot(self_, self_.slot_index_parent) {
            return false;
        }
        if mpconfig::MULTIPLE_INHERITANCE {
            if let Some(parent) = obj::type_get_slot_parent(self_) {
                if obj::is_exact_type(parent, obj::type_tuple()) {
                    let (len, items) = objtuple::tuple_get(parent);
                    for i in 0..len.saturating_sub(1) {
                        if is_subclass_fast(items[i], classinfo) {
                            return true;
                        }
                    }
                    if len > 0 {
                        cur = items[len - 1];
                        continue;
                    }
                }
            }
        }
        cur = obj::type_get_slot_parent(self_).unwrap_or(obj::from_ptr(type_object() as *const ObjType as *const ()));
    }
}

fn is_subclass(object: Obj, classinfo: Obj) -> Obj {
    let (len, items) = if obj::is_exact_type(classinfo, type_type()) {
        (1usize, vec![classinfo])
    } else if obj::is_exact_type(classinfo, obj::type_tuple()) {
        objtuple::tuple_get(classinfo)
    } else {
        raise::raise(MpRaise::TypeError("issubclass() arg 2 must be a class"));
    };
    for i in 0..len {
        if items[i] == obj::from_ptr(type_object() as *const ObjType as *const ()) || is_subclass_fast(object, items[i]) {
            return obj::CONST_TRUE;
        }
    }
    obj::CONST_FALSE
}

pub fn isinstance(object: Obj, classinfo: Obj) -> Obj {
    is_subclass(obj::from_ptr(obj::get_type(object) as *const ()), classinfo)
}

pub fn issubclass(object: Obj, classinfo: Obj) -> Obj {
    if !obj::is_exact_type(object, type_type()) {
        raise::raise(MpRaise::TypeError("issubclass() arg 1 must be a class"));
    }
    is_subclass(object, classinfo)
}

pub fn cast_to_native_base(self_in: Obj, native_type: Obj) -> Obj {
    let self_type = obj::get_type(self_in);
    if obj::from_ptr(self_type as *const ()) == native_type {
        return self_in;
    }
    if !is_subclass_fast(obj::from_ptr(self_type as *const ()), native_type) {
        return obj::OBJ_NULL;
    }
    let self_ptr = unsafe { &*(obj::as_ptr(self_in) as *const ObjInstance) };
    unsafe { subobj_get(self_ptr, 0) }
}

#[repr(C)]
pub struct ObjStaticClassMethod {
    pub base: ObjBase,
    pub fun: Obj,
}

fn static_class_method_make_new(self_: &'static ObjType, n_args: usize, n_kw: usize, args: &[Obj]) -> Obj {
    argcheck::check_num(n_args, n_kw, 1, 1, false);
    let o = obj::malloc_helper(size_of::<ObjStaticClassMethod>(), self_) as *mut ObjStaticClassMethod;
    unsafe { (*o).fun = args[0]; }
    unsafe {
        let hdr = o;
        (*hdr).slots = (hdr as *mut u8).add(std::mem::size_of::<ObjType>()) as *const *const ();
    }
    obj::from_ptr(o as *const ())
}

pub fn instance_get_call_pub(self_in: Obj) -> Option<Obj> {
    let mut m = [obj::OBJ_NULL; 2];
    let c = instance_get_call(self_in, &mut m);
    if c == obj::OBJ_NULL { None } else { Some(c) }
}

