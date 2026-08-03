//! rewrite of py/objpolyiter.c (polymorph iterator for list/tuple iters)
// symmetry: done

use crate::obj::{self, IterNextFn, Obj, ObjBase, ObjType, TYPE_FLAG_BINDS_SELF, TYPE_FLAG_BUILTIN_FUN, TYPE_FLAG_ITER_IS_ITERNEXT};
use crate::map::{self, MapElem};
use crate::malloc;
use crate::objdict::{self, ObjDict};
use crate::qstr;

/// Universal iterator shell (`mp_obj_polymorph_iter_t`).
#[repr(C)]
pub struct ObjPolymorphIter {
    pub base: ObjBase,
    pub iternext: IterNextFn,
}

/// Iterator with finaliser (`mp_obj_polymorph_iter_with_finaliser_t`).
#[repr(C)]
pub struct ObjPolymorphIterWithFinaliser {
    pub base: ObjBase,
    pub iternext: IterNextFn,
    pub finaliser: IterNextFn,
}

type BuiltinFn1 = fn(Obj) -> Obj;

#[repr(C)]
struct ObjFunBuiltin1 {
    base: ObjBase,
    fun: BuiltinFn1,
}

static mut POLYMORPH_ITER_SLOTS: [*const (); 1] = [polymorph_it_iternext as *const ()];

static TYPE_POLYMORPH_ITER: ObjType = ObjType {
    base: ObjBase { type_: core::ptr::null() },
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
    slots: unsafe { POLYMORPH_ITER_SLOTS.as_ptr() },
};

static mut POLYMORPH_ITER_FINAL_SLOTS: [*const (); 2] =
    [polymorph_it_iternext as *const (), core::ptr::null()];
static mut F1: [*const (); 1] = [call1 as *const ()];
static TF1: ObjType = ObjType {
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
    slots: unsafe { F1.as_ptr() },
};
static mut TYPE_POLYMORPH_ITER_WITH_FINALISER: ObjType = ObjType {
    base: ObjBase { type_: core::ptr::null() },
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
    slot_index_locals_dict: 2,
    slots: unsafe { POLYMORPH_ITER_FINAL_SLOTS.as_ptr() },
};

static FINALISER_INIT: std::sync::OnceLock<()> = std::sync::OnceLock::new();

fn call1(s: Obj, n: usize, k: usize, a: &[Obj]) -> Obj {
    crate::argcheck::check_num(n, k, 1, 1, false);
    unsafe { ((*(obj::as_ptr(s) as *const ObjFunBuiltin1)).fun)(a[0]) }
}

fn mk1(f: BuiltinFn1) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltin1>().expect("polyiter fn1");
    unsafe {
        (*o).base.type_ = &TF1;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltin1 as *const ())
    }
}

fn init_finaliser_type() {
    FINALISER_INIT.get_or_init(|| {
        let table = vec![MapElem {
            key: obj::new_qstr(qstr::from_str("__del__")),
            value: mk1(polymorph_it_del),
        }];
        let ptr =
            obj::malloc_helper(core::mem::size_of::<ObjDict>(), objdict::type_dict()) as *mut ObjDict;
        unsafe {
            map::init_fixed_table(&mut (*ptr).map, table);
            POLYMORPH_ITER_FINAL_SLOTS[1] =
                obj::from_ptr(ptr as *const ObjDict as *const ()).0 as *const ();
            TYPE_POLYMORPH_ITER_WITH_FINALISER.name = qstr::from_str("iterator");
        }
    });
}

pub fn type_polymorph_iter() -> &'static ObjType {
    &TYPE_POLYMORPH_ITER
}

pub fn type_polymorph_iter_with_finaliser() -> &'static ObjType {
    init_finaliser_type();
    unsafe { &TYPE_POLYMORPH_ITER_WITH_FINALISER }
}

/// `polymorph_it_iternext` — iter slot redirects to per-instance iternext.
pub fn polymorph_it_iternext(self_in: Obj) -> Obj {
    let self_ = unsafe { &*(obj::as_ptr(self_in) as *const ObjPolymorphIter) };
    (self_.iternext)(self_in)
}

fn polymorph_it_del(self_in: Obj) -> Obj {
    let self_ = unsafe { &*(obj::as_ptr(self_in) as *const ObjPolymorphIterWithFinaliser) };
    (self_.finaliser)(self_in)
}
