//! rewrite of py/objgetitemiter.c
// symmetry: done

use crate::malloc;
use crate::nlr::{self, NlrBuf};
use crate::obj::{self, Obj, ObjBase, ObjIterBuf, ObjType, TYPE_FLAG_ITER_IS_ITERNEXT};
use crate::objexcept;
use crate::qstr::{self, Qstr};
use crate::runtime;

/// Wrapper iterator for objects with `__getitem__` (`mp_obj_getitem_iter_t`).
#[repr(C)]
pub struct ObjGetitemIter {
    pub base: ObjBase,
    pub args: [Obj; 3],
}

static mut GETITEM_ITER_SLOTS: [*const (); 1] = [getitem_iternext as *const ()];

static mut TYPE_GETITEM_ITER: ObjType = ObjType {
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
    slots: unsafe { GETITEM_ITER_SLOTS.as_ptr() },
};

static INIT: std::sync::OnceLock<()> = std::sync::OnceLock::new();

fn init_type() {
    INIT.get_or_init(|| unsafe {
        TYPE_GETITEM_ITER.name = qstr::from_str("iterator");
    });
}

pub fn type_getitem_iter() -> &'static ObjType {
    init_type();
    unsafe { &TYPE_GETITEM_ITER }
}

fn getitem_iternext(self_in: Obj, _buf: *mut ObjIterBuf) -> Obj {
    let self_ = unsafe { &mut *(obj::as_ptr(self_in) as *mut ObjGetitemIter) };
    let mut nlr_buf = NlrBuf::default();
    match nlr::protect(&mut nlr_buf, || runtime::call_method_n_kw(1, 0, &self_.args)) {
        Ok(value) => {
            if obj::is_small_int(self_.args[2]) {
                let n = obj::small_int_value(self_.args[2]) + 1;
                self_.args[2] = obj::new_small_int(n);
            }
            value
        }
        Err(exc) => {
            let exc_obj = Obj(exc);
            let t = obj::get_type(exc_obj);
            if core::ptr::eq(t, objexcept::type_stop_iteration())
                || core::ptr::eq(t, objexcept::type_index_error())
            {
                return obj::OBJ_STOP_ITERATION;
            }
            crate::raise::raise_obj(exc_obj);
        }
    }
}

/// `mp_obj_new_getitem_iter` — `args` are those returned from `mp_load_method_maybe`.
pub fn new_getitem_iter(args: &[Obj; 2], iter_buf: &mut ObjIterBuf) -> Obj {
    assert!(core::mem::size_of::<ObjGetitemIter>() <= core::mem::size_of::<ObjIterBuf>());
    init_type();
    let o = unsafe { &mut *(iter_buf as *mut ObjIterBuf as *mut ObjGetitemIter) };
    o.base.type_ = type_getitem_iter() as *const ObjType;
    o.args[0] = args[0];
    o.args[1] = args[1];
    o.args[2] = obj::new_small_int(0);
    obj::from_ptr(o as *const ObjGetitemIter as *const ())
}

/// Heap-backed getitem iterator when no stack buffer is supplied.
pub fn new_getitem_iter_heap(args: &[Obj; 2]) -> Obj {
    init_type();
    let o = malloc::new_obj::<ObjGetitemIter>().expect("getitem iter");
    unsafe {
        (*o).base.type_ = type_getitem_iter() as *const ObjType;
        (*o).args[0] = args[0];
        (*o).args[1] = args[1];
        (*o).args[2] = obj::new_small_int(0);
    }
    obj::from_ptr(o as *const ObjGetitemIter as *const ())
}
