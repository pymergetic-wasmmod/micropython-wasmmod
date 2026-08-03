//! rewrite of py/objattrtuple.c
// symmetry: done

use core::mem::size_of;

use crate::mpconfig;
use crate::mpprint::{self, Print, PrintKind};
use crate::obj::{self, Obj, ObjBase, ObjType, TYPE_FLAG_ITER_IS_CUSTOM};
use crate::objstr;
use crate::objtuple::{self, ObjTuple};
use crate::qstr::{self, Qstr};

/// `mp_obj_attrtuple_print_helper` — shared with collections.namedtuple.
pub fn attrtuple_print_helper(print: &Print, fields: &[Qstr], o: &ObjTuple, items: &[Obj]) {
    mpprint::print_str(print, "(");
    for i in 0..o.len {
        if i > 0 {
            mpprint::print_str(print, ", ");
        }
        mpprint::print_str(
            print,
            &format!("{}=", qstr::str_from_qstr(fields[i]).unwrap_or_default()),
        );
        obj::print_helper(print, items[i], PrintKind::Repr);
    }
    mpprint::print_str(print, ")");
}

fn items_ptr(o: *const ObjTuple) -> *const Obj {
    unsafe { (o as *const u8).add(size_of::<ObjTuple>()) as *const Obj }
}

fn items_ptr_mut(o: *mut ObjTuple) -> *mut Obj {
    unsafe { (o as *mut u8).add(size_of::<ObjTuple>()) as *mut Obj }
}

fn attrtuple_print(print: &Print, o_in: Obj, _kind: PrintKind) {
    let o = unsafe { &*(obj::as_ptr(o_in) as *const ObjTuple) };
    let fields_obj = unsafe { *items_ptr(o).add(o.len) };
    let (_n, field_objs) = objtuple::tuple_get(fields_obj);
    let fields: Vec<Qstr> = field_objs
        .iter()
        .map(|&fo| objstr::str_get_qstr(fo))
        .collect();
    let items = unsafe { std::slice::from_raw_parts(items_ptr(o), o.len) };
    attrtuple_print_helper(print, &fields, o, items);
}

fn attrtuple_attr(self_in: Obj, attr: Qstr, dest: &mut [Obj; 2]) {
    if dest[0] != obj::OBJ_NULL {
        return;
    }
    let self_ = unsafe { &*(obj::as_ptr(self_in) as *const ObjTuple) };
    let len = self_.len;
    let fields_obj = unsafe { *items_ptr(self_).add(len) };
    let (_n, field_objs) = objtuple::tuple_get(fields_obj);
    for i in 0..len {
        if objstr::str_get_qstr(field_objs[i]) == attr {
            dest[0] = unsafe { *items_ptr(self_).add(i) };
            return;
        }
    }
}

/// `mp_obj_new_attrtuple`
pub fn new_attrtuple(fields: &[Qstr], n: usize, items: &[Obj]) -> Obj {
    assert_eq!(fields.len(), n);
    assert_eq!(items.len(), n);
    // Store field names as a real tuple of qstrs so GC tracing stays sound
    // (C stuffs a raw qstr[] pointer into the last slot; that is not GC-safe here).
    let field_objs: Vec<Obj> = fields.iter().map(|&q| obj::new_qstr(q)).collect();
    let fields_tuple = objtuple::new_tuple(n, Some(&field_objs));
    let o = obj::malloc_var::<ObjTuple>(size_of::<Obj>() * (n + 1), type_attrtuple());
    unsafe {
        (*o).len = n;
        let dst = std::slice::from_raw_parts_mut(items_ptr_mut(o), n + 1);
        dst[..n].copy_from_slice(items);
        dst[n] = fields_tuple;
        obj::from_ptr(o as *const ObjTuple as *const ())
    }
}

static mut ATTRTUPLE_SLOTS: [*const (); 6] = [
    attrtuple_print as *const (),
    objtuple::tuple_unary_op as *const (),
    objtuple::tuple_binary_op as *const (),
    attrtuple_attr as *const (),
    objtuple::tuple_subscr as *const (),
    objtuple::tuple_getiter as *const (),
];

static mut TYPE_ATTRTUPLE: ObjType = ObjType {
    base: ObjBase {
        type_: core::ptr::null(),
    },
    flags: TYPE_FLAG_ITER_IS_CUSTOM,
    name: 0,
    slot_index_make_new: 0,
    slot_index_print: 1,
    slot_index_call: 0,
    slot_index_unary_op: 2,
    slot_index_binary_op: 3,
    slot_index_attr: 4,
    slot_index_subscr: 5,
    slot_index_iter: 6,
    slot_index_buffer: 0,
    slot_index_protocol: 0,
    slot_index_parent: 0,
    slot_index_locals_dict: 0,
    slots: unsafe { ATTRTUPLE_SLOTS.as_ptr() },
};

static INIT: std::sync::OnceLock<()> = std::sync::OnceLock::new();

fn init_type() {
    INIT.get_or_init(|| unsafe {
        TYPE_ATTRTUPLE.name = qstr::from_str("tuple");
    });
}

pub fn type_attrtuple() -> &'static ObjType {
    if !(mpconfig::PY_ATTRTUPLE || mpconfig::PY_COLLECTIONS) {
        return objtuple::type_tuple();
    }
    init_type();
    unsafe { &TYPE_ATTRTUPLE }
}
