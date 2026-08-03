//! rewrite of py/objproperty.c
// symmetry: done

use core::mem::size_of;

use crate::argcheck::{self, Arg, ArgFlag, ArgVal};
use crate::malloc;
use crate::map::{self, MapElem};
use crate::mpconfig;
use crate::obj::{self, Obj, ObjBase, ObjType};
use crate::objdict::{self, ObjDict};
use crate::objfun::{self, BuiltinFn2, ObjFunBuiltinFixed};
use crate::qstr;
use crate::raise::{self, MpRaise};

#[repr(C)]
pub struct ObjProperty {
    pub base: ObjBase,
    pub proxy: [Obj; 3],
}

static mut PROPERTY_SLOTS: [*const (); 2] = [property_make_new as *const (), core::ptr::null()];

static mut TYPE: ObjType = ObjType {
    base: ObjBase {
        type_: core::ptr::null(),
    },
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
    slot_index_locals_dict: 2,
    slots: unsafe { PROPERTY_SLOTS.as_ptr() },
};

static PROPERTY_INIT: std::sync::OnceLock<()> = std::sync::OnceLock::new();

fn new_fun_builtin_2(fun: BuiltinFn2) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltinFixed>().expect("fun_builtin_2 alloc");
    unsafe {
        (*o).base.type_ = objfun::type_fun_builtin_2() as *const ObjType;
        (*o).fun.f2 = fun;
        obj::from_ptr(o as *const ObjFunBuiltinFixed as *const ())
    }
}

/// `property_locals_dict_table` (py/objproperty.c): exposes `.getter`,
/// `.setter`, `.deleter` so `@x.setter` decorator syntax works.
fn init_property_type() {
    PROPERTY_INIT.get_or_init(|| {
        let table = vec![
            MapElem {
                key: obj::new_qstr(qstr::from_str("getter")),
                value: new_fun_builtin_2(property_getter),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("setter")),
                value: new_fun_builtin_2(property_setter),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("deleter")),
                value: new_fun_builtin_2(property_deleter),
            },
        ];
        let ptr = obj::malloc_helper(size_of::<ObjDict>(), objdict::type_dict()) as *mut ObjDict;
        unsafe {
            map::init_fixed_table(&mut (*ptr).map, table);
            PROPERTY_SLOTS[1] = obj::from_ptr(ptr as *const ObjDict as *const ()).0 as *const ();
            TYPE.name = qstr::from_str("property");
        }
    });
}

pub fn type_property() -> &'static ObjType {
    init_property_type();
    unsafe { &*core::ptr::addr_of!(TYPE) }
}

pub fn is_property(o: Obj) -> bool {
    mpconfig::PY_BUILTINS_PROPERTY && obj::is_exact_type(o, type_property())
}

fn check_self(o: Obj) {
    if !is_property(o) {
        raise::raise(MpRaise::TypeError("argument has wrong type"));
    }
}

/// Return getter/setter/deleter proxies (`mp_obj_property_get`).
pub fn get(o: Obj) -> [Obj; 3] {
    check_self(o);
    unsafe { (*(obj::as_ptr(o) as *const ObjProperty)).proxy }
}

fn clone_with_proxy(self_in: Obj, index: usize, value: Obj) -> Obj {
    let src = unsafe { &*(obj::as_ptr(self_in) as *const ObjProperty) };
    let o = malloc::new_obj::<ObjProperty>().expect("property alloc");
    unsafe {
        (*o).base.type_ = type_property() as *const ObjType;
        (*o).proxy = src.proxy;
        (*o).proxy[index] = value;
        obj::from_ptr(o as *const ObjProperty as *const ())
    }
}

/// `property_make_new`
pub fn property_make_new(_type_in: &ObjType, n_args: usize, n_kw: usize, args: &[Obj]) -> Obj {
    const ARG_FGET: usize = 0;
    const ARG_FSET: usize = 1;
    const ARG_FDEL: usize = 2;

    let allowed = [
        Arg {
            qst: 0,
            flags: ArgFlag::Obj as u16,
            defval: ArgVal::Obj(obj::CONST_NONE),
        },
        Arg {
            qst: 0,
            flags: ArgFlag::Obj as u16,
            defval: ArgVal::Obj(obj::CONST_NONE),
        },
        Arg {
            qst: 0,
            flags: ArgFlag::Obj as u16,
            defval: ArgVal::Obj(obj::CONST_NONE),
        },
        Arg {
            qst: qstr::from_str("doc"),
            flags: ArgFlag::Obj as u16,
            defval: ArgVal::Obj(obj::CONST_NONE),
        },
    ];

    let mut vals = [ArgVal::default(); 4];
    argcheck::parse_all_kw_array(n_args, n_kw, args, allowed.len(), &allowed, &mut vals);

    let o = malloc::new_obj::<ObjProperty>().expect("property alloc");
    unsafe {
        (*o).base.type_ = type_property() as *const ObjType;
        (*o).proxy[0] = match vals[ARG_FGET] {
            ArgVal::Obj(v) => v,
            _ => obj::CONST_NONE,
        };
        (*o).proxy[1] = match vals[ARG_FSET] {
            ArgVal::Obj(v) => v,
            _ => obj::CONST_NONE,
        };
        (*o).proxy[2] = match vals[ARG_FDEL] {
            ArgVal::Obj(v) => v,
            _ => obj::CONST_NONE,
        };
        obj::from_ptr(o as *const ObjProperty as *const ())
    }
}

/// `property_getter` decorator helper.
pub fn property_getter(self_in: Obj, getter: Obj) -> Obj {
    clone_with_proxy(self_in, 0, getter)
}

/// `property_setter` decorator helper.
pub fn property_setter(self_in: Obj, setter: Obj) -> Obj {
    clone_with_proxy(self_in, 1, setter)
}

/// `property_deleter` decorator helper.
pub fn property_deleter(self_in: Obj, deleter: Obj) -> Obj {
    clone_with_proxy(self_in, 2, deleter)
}
