//! rewrite of py/objnamedtuple.c + py/objnamedtuple.h
// symmetry: done

use core::mem::size_of;

use crate::argcheck;
use crate::map::{self, MapElem};
use crate::malloc;
use crate::mpconfig;
use crate::mpprint::{self, Print, PrintKind, VaArg};
use crate::obj::{self, Obj, ObjBase, ObjType, TYPE_FLAG_BINDS_SELF, TYPE_FLAG_BUILTIN_FUN, TYPE_FLAG_EQ_CHECKS_OTHER_TYPE};
use crate::objattrtuple;
use crate::objdict;
use crate::objstr;
use crate::objtuple::{self, ObjTuple};
use crate::qstr::{self, Qstr};
use crate::raise::{self, MpRaise};

#[repr(C)]
pub struct ObjNamedtupleType {
    pub base: ObjType,
    pub n_fields: usize,
}

#[repr(C)]
pub struct ObjNamedtuple {
    pub tuple: ObjTuple,
}

type BuiltinFn1 = fn(Obj) -> Obj;

#[repr(C)]
struct ObjFunBuiltin1 {
    base: ObjBase,
    fun: BuiltinFn1,
}

static mut ASDICT_SLOTS: [*const (); 1] = [fun1_call as *const ()];
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
    slots: unsafe { ASDICT_SLOTS.as_ptr() },
};

fn fun1_call(self_in: Obj, n_args: usize, n_kw: usize, args: &[Obj]) -> Obj {
    argcheck::check_num(n_args, n_kw, 1, 1, false);
    let self_ = unsafe { &*(obj::as_ptr(self_in) as *const ObjFunBuiltin1) };
    (self_.fun)(args[0])
}

fn fields_ptr(nt: *const ObjNamedtupleType) -> *const Qstr {
    unsafe { (nt as *const u8).add(size_of::<ObjNamedtupleType>()) as *const Qstr }
}

fn fields_ptr_mut(nt: *mut ObjNamedtupleType) -> *mut Qstr {
    unsafe { (nt as *mut u8).add(size_of::<ObjNamedtupleType>()) as *mut Qstr }
}

/// `mp_obj_namedtuple_find_field`
pub fn namedtuple_find_field(type_: *const ObjNamedtupleType, name: Qstr) -> usize {
    let n = unsafe { (*type_).n_fields };
    let fields = unsafe { std::slice::from_raw_parts(fields_ptr(type_), n) };
    for (i, &f) in fields.iter().enumerate() {
        if f == name {
            return i;
        }
    }
    usize::MAX
}

fn tuple_items(o: &ObjTuple) -> *const Obj {
    unsafe { (o as *const ObjTuple as *const u8).add(size_of::<ObjTuple>()) as *const Obj }
}

fn namedtuple_asdict(self_in: Obj) -> Obj {
    let self_ = unsafe { &*(obj::as_ptr(self_in) as *const ObjNamedtuple) };
    let type_ptr = obj::get_type(obj::from_ptr(self_ as *const ObjNamedtuple as *const ()));
    let type_ = unsafe { &*(type_ptr as *const ObjType as *const ObjNamedtupleType) };
    let fields = unsafe { std::slice::from_raw_parts(fields_ptr(type_), type_.n_fields) };
    let dict = objdict::new_dict(self_.tuple.len);
    unsafe {
        let d = objdict::dict_ptr(dict);
        (*d).map.is_ordered = true;
    }
    for i in 0..self_.tuple.len {
        let key = obj::new_qstr(fields[i]);
        let val = unsafe { *tuple_items(&self_.tuple).add(i) };
        objdict::dict_store(dict, key, val);
    }
    dict
}

fn namedtuple_print(print: &Print, o_in: Obj, _kind: PrintKind) {
    let o = unsafe { &*(obj::as_ptr(o_in) as *const ObjNamedtuple) };
    let t = obj::get_type(o_in);
    mpprint::printf(
        print,
        "{}",
        [VaArg::Str(qstr::str_from_qstr(t.name).unwrap_or_default().as_str())],
    );
    let type_ = unsafe { &*(t as *const ObjType as *const ObjNamedtupleType) };
    let fields = unsafe { std::slice::from_raw_parts(fields_ptr(type_), type_.n_fields) };
    let items = unsafe { std::slice::from_raw_parts(tuple_items(&o.tuple), o.tuple.len) };
    objattrtuple::attrtuple_print_helper(print, fields, &o.tuple, items);
}

fn namedtuple_attr(self_in: Obj, attr: Qstr, dest: &mut [Obj; 2]) {
    if dest[0] == obj::OBJ_NULL {
        let self_ = unsafe { &*(obj::as_ptr(self_in) as *const ObjNamedtuple) };
        if mpconfig::PY_COLLECTIONS_NAMEDTUPLE__ASDICT && attr == qstr::from_str("_asdict") {
            static mut ASDICT_FUN: Option<Obj> = None;
            unsafe {
                if ASDICT_FUN.is_none() {
                    let o = malloc::new_obj::<ObjFunBuiltin1>().expect("asdict");
                    (*o).base.type_ = &TYPE_FUN1 as *const ObjType;
                    (*o).fun = namedtuple_asdict;
                    ASDICT_FUN = Some(obj::from_ptr(o as *const ObjFunBuiltin1 as *const ()));
                }
                dest[0] = ASDICT_FUN.unwrap();
            }
            dest[1] = self_in;
            return;
        }
        let type_ptr = obj::get_type(self_in);
        let type_ = unsafe { &*(type_ptr as *const ObjType as *const ObjNamedtupleType) };
        let id = namedtuple_find_field(type_, attr);
        if id == usize::MAX {
            return;
        }
        dest[0] = unsafe { *tuple_items(&self_.tuple).add(id) };
    } else {
        raise::raise(MpRaise::AttributeError("can't set attribute"));
    }
}

fn namedtuple_make_new(type_in: &ObjType, n_args: usize, n_kw: usize, args: &[Obj]) -> Obj {
    let type_ = unsafe { &*(type_in as *const ObjType as *const ObjNamedtupleType) };
    let num_fields = type_.n_fields;
    if n_args + n_kw != num_fields {
        raise::raise(MpRaise::TypeError("namedtuple argument mismatch"));
    }
    let type_static: &'static ObjType = unsafe { &*(type_in as *const ObjType) };
    let o = obj::malloc_var::<ObjTuple>(size_of::<Obj>() * num_fields, type_static);
    unsafe {
        (*o).len = num_fields;
        let items = std::slice::from_raw_parts_mut(
            (o as *mut u8).add(size_of::<ObjTuple>()) as *mut Obj,
            num_fields,
        );
        items[..n_args].copy_from_slice(args);
        for i in n_args..num_fields {
            items[i] = obj::OBJ_NULL;
        }
        for i in (n_args..n_args + 2 * n_kw).step_by(2) {
            let kw = objstr::str_get_qstr(args[i]);
            let id = namedtuple_find_field(type_, kw);
            if id == usize::MAX {
                raise::raise(MpRaise::TypeError("unexpected keyword argument"));
            }
            if items[id] != obj::OBJ_NULL {
                raise::raise(MpRaise::TypeError("multiple values for argument"));
            }
            items[id] = args[i + 1];
        }
        obj::from_ptr(o as *const ObjTuple as *const ())
    }
}

static mut NT_TYPE_SLOTS: [*const (); 8] = [
    namedtuple_make_new as *const (),
    namedtuple_print as *const (),
    objtuple::tuple_unary_op as *const (),
    objtuple::tuple_binary_op as *const (),
    namedtuple_attr as *const (),
    objtuple::tuple_subscr as *const (),
    objtuple::tuple_getiter as *const (),
    core::ptr::null(),
];

fn new_namedtuple_type(name: Qstr, n_fields: usize, fields: &[Obj]) -> Obj {
    let extra = size_of::<Qstr>() * n_fields;
    let o = obj::malloc_var::<ObjNamedtupleType>(extra, obj::type_type());
    unsafe {
        (*o).base.base.type_ = obj::type_type();
        (*o).base.flags = TYPE_FLAG_EQ_CHECKS_OTHER_TYPE;
        (*o).base.name = name;
        (*o).base.slot_index_make_new = 1;
        (*o).base.slot_index_print = 2;
        (*o).base.slot_index_unary_op = 3;
        (*o).base.slot_index_binary_op = 4;
        (*o).base.slot_index_attr = 5;
        (*o).base.slot_index_subscr = 6;
        (*o).base.slot_index_iter = 7;
        (*o).base.slot_index_parent = 8;
        (*o).n_fields = n_fields;
        NT_TYPE_SLOTS[7] = objtuple::type_tuple() as *const ObjType as *const ();
        (*o).base.slots = NT_TYPE_SLOTS.as_ptr();
        for (i, &f) in fields.iter().enumerate() {
            *fields_ptr_mut(o).add(i) = objstr::str_get_qstr(f);
        }
        obj::from_ptr(o as *const ObjNamedtupleType as *const ())
    }
}

fn new_namedtuple_type_fn(name_in: Obj, fields_in: Obj) -> Obj {
    let name = objstr::str_get_qstr(name_in);
    let (n_fields, fields) = obj::get_array(fields_in);
    new_namedtuple_type(name, n_fields, &fields)
}

static mut NAMEDTUPLE_OBJ: Option<Obj> = None;

#[repr(C)]
struct ObjFunBuiltin2 {
    base: ObjBase,
    fun: fn(Obj, Obj) -> Obj,
}

static mut NT_SLOTS: [*const (); 1] = [nt_call as *const ()];
static TYPE_NT: ObjType = ObjType {
    base: ObjBase { type_: core::ptr::null() },
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
    slots: unsafe { NT_SLOTS.as_ptr() },
};

fn nt_call(self_in: Obj, n_args: usize, n_kw: usize, args: &[Obj]) -> Obj {
    argcheck::check_num(n_args, n_kw, 2, 2, false);
    let self_ = unsafe { &*(obj::as_ptr(self_in) as *const ObjFunBuiltin2) };
    (self_.fun)(args[0], args[1])
}

/// `mp_obj_new_namedtuple_base`
pub fn new_namedtuple_base(n_fields: usize, fields: &[Obj]) -> *mut ObjNamedtupleType {
    let o = obj::malloc_var::<ObjNamedtupleType>(size_of::<Qstr>() * n_fields, obj::type_type());
    unsafe {
        (*o).n_fields = n_fields;
        for (i, &f) in fields.iter().enumerate() {
            *fields_ptr_mut(o).add(i) = objstr::str_get_qstr(f);
        }
    }
    o
}

pub fn namedtuple_obj() -> Obj {
    if !mpconfig::PY_COLLECTIONS {
        return obj::OBJ_NULL;
    }
    unsafe {
        if NAMEDTUPLE_OBJ.is_none() {
            let o = malloc::new_obj::<ObjFunBuiltin2>().expect("namedtuple");
            (*o).base.type_ = &TYPE_NT as *const ObjType;
            (*o).fun = new_namedtuple_type_fn;
            NAMEDTUPLE_OBJ = Some(obj::from_ptr(o as *const ObjFunBuiltin2 as *const ()));
        }
        NAMEDTUPLE_OBJ.unwrap()
    }
}
