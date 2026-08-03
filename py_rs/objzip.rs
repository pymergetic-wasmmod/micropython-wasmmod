//! rewrite of py/objzip.c
// symmetry: done

use core::mem::size_of;

use crate::argcheck;
use crate::obj::{self, Obj, ObjBase, ObjType, TYPE_FLAG_ITER_IS_ITERNEXT};
use crate::objtuple;
use crate::qstr;
use crate::raise::{self, MpRaise};
use crate::runtime;

#[repr(C)]
pub struct ObjZip {
    pub base: ObjBase,
    pub n_iters: usize,
}

fn iters_ptr(o: *const ObjZip) -> *const Obj {
    unsafe { (o as *const u8).add(size_of::<ObjZip>()) as *const Obj }
}

fn iters_ptr_mut(o: *mut ObjZip) -> *mut Obj {
    unsafe { (o as *mut u8).add(size_of::<ObjZip>()) as *mut Obj }
}

static mut ZIP_SLOTS: [*const (); 2] = [zip_make_new as *const (), zip_iternext as *const ()];

static mut TYPE_ZIP: ObjType = ObjType {
    base: ObjBase { type_: core::ptr::null() },
    flags: TYPE_FLAG_ITER_IS_ITERNEXT,
    name: 0,
    slot_index_make_new: 1,
    slot_index_print: 0,
    slot_index_call: 0,
    slot_index_unary_op: 0,
    slot_index_binary_op: 0,
    slot_index_attr: 0,
    slot_index_subscr: 0,
    slot_index_iter: 2,
    slot_index_buffer: 0,
    slot_index_protocol: 0,
    slot_index_parent: 0,
    slot_index_locals_dict: 0,
    slots: unsafe { ZIP_SLOTS.as_ptr() },
};

static INIT: std::sync::OnceLock<()> = std::sync::OnceLock::new();

fn init_type() {
    INIT.get_or_init(|| unsafe {
        TYPE_ZIP.name = qstr::from_str("zip");
    });
}

pub fn type_zip() -> &'static ObjType {
    init_type();
    unsafe { &TYPE_ZIP }
}

fn zip_make_new(type_in: &ObjType, n_args: usize, n_kw: usize, args: &[Obj]) -> Obj {
    argcheck::check_num(n_args, n_kw, 0, usize::MAX, false);
    let type_static: &'static ObjType = unsafe { &*(type_in as *const ObjType) };
    let o = obj::malloc_var::<ObjZip>(n_args * size_of::<Obj>(), type_static);
    unsafe {
        (*o).n_iters = n_args;
        let iters = std::slice::from_raw_parts_mut(iters_ptr_mut(o), n_args);
        for (i, &arg) in args.iter().enumerate() {
            iters[i] = runtime::getiter(arg, None);
        }
        obj::from_ptr(o as *const ObjZip as *const ())
    }
}

fn zip_iternext(self_in: Obj) -> Obj {
    if !obj::is_exact_type(self_in, type_zip()) {
        raise::raise(MpRaise::TypeError("zip iternext"));
    }
    let self_ = unsafe { &*(obj::as_ptr(self_in) as *const ObjZip) };
    let mut items = Vec::with_capacity(self_.n_iters);
    for i in 0..self_.n_iters {
        let next = runtime::iternext(unsafe { *iters_ptr(self_).add(i) });
        if next == obj::OBJ_STOP_ITERATION {
            return obj::OBJ_STOP_ITERATION;
        }
        items.push(next);
    }
    if items.is_empty() {
        return obj::OBJ_STOP_ITERATION;
    }
    objtuple::new_tuple(items.len(), Some(&items))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gc;
    use crate::objlist;

    fn setup() {
        let _ = gc::init();
        runtime::init();
        init_type();
    }

    #[test]
    fn zip_pairs_lists() {
        setup();
        let a = objlist::new_list(3, Some(&[
            obj::new_small_int(1),
            obj::new_small_int(2),
            obj::new_small_int(3),
        ]));
        let b = objlist::new_list(3, Some(&[
            obj::new_small_int(4),
            obj::new_small_int(5),
            obj::new_small_int(6),
        ]));
        let z = zip_make_new(type_zip(), 2, 0, &[a, b]);
        let t1 = zip_iternext(z);
        let (n, items) = objtuple::tuple_get(t1);
        assert_eq!(n, 2);
        assert_eq!(obj::small_int_value(items[0]), 1);
        assert_eq!(obj::small_int_value(items[1]), 4);
        let _ = zip_iternext(z);
        let _ = zip_iternext(z);
        assert_eq!(zip_iternext(z), obj::OBJ_STOP_ITERATION);
    }
}
