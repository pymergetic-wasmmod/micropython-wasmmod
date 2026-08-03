//! rewrite of py/objenumerate.c
// symmetry: done

use crate::argcheck;
use crate::malloc;
use crate::mpconfig;
use crate::obj::{self, Int, Obj, ObjBase, ObjType, TYPE_FLAG_ITER_IS_ITERNEXT};
use crate::objtuple;
use crate::qstr;
use crate::runtime;

#[repr(C)]
pub struct ObjEnumerate {
    pub base: ObjBase,
    pub iter: Obj,
    pub cur: Int,
}

static mut ENUM_SLOTS: [*const (); 2] = [enumerate_make_new as *const (), enumerate_iternext as *const ()];

static mut TYPE_ENUMERATE: ObjType = ObjType {
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
    slots: unsafe { ENUM_SLOTS.as_ptr() },
};

static INIT: std::sync::OnceLock<()> = std::sync::OnceLock::new();

fn init_type() {
    INIT.get_or_init(|| unsafe {
        TYPE_ENUMERATE.name = qstr::from_str("enumerate");
    });
}

pub fn type_enumerate() -> &'static ObjType {
    init_type();
    unsafe { &TYPE_ENUMERATE }
}

fn enumerate_make_new(type_in: &ObjType, n_args: usize, n_kw: usize, args: &[Obj]) -> Obj {
    if mpconfig::CPYTHON_COMPAT {
        argcheck::check_num(n_args, n_kw, 1, 2, false);
    } else {
        argcheck::check_num(n_args, n_kw, 1, 2, false);
    }
    let o = malloc::new_obj::<ObjEnumerate>().expect("enumerate alloc");
    unsafe {
        (*o).base.type_ = type_in as *const ObjType;
        (*o).iter = runtime::getiter(args[0], None);
        (*o).cur = if n_args > 1 { obj::get_int(args[1]) } else { 0 };
        obj::from_ptr(o as *const ObjEnumerate as *const ())
    }
}

fn enumerate_iternext(self_in: Obj) -> Obj {
    let self_ = unsafe { &mut *(obj::as_ptr(self_in) as *mut ObjEnumerate) };
    let next = runtime::iternext(self_.iter);
    if next == obj::OBJ_STOP_ITERATION {
        return obj::OBJ_STOP_ITERATION;
    }
    let items = [obj::new_small_int(self_.cur), next];
    self_.cur += 1;
    objtuple::new_tuple(2, Some(&items))
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
    fn enumerate_yields_index_value() {
        setup();
        let lst = objlist::new_list(2, Some(&[
            obj::new_small_int(10),
            obj::new_small_int(20),
        ]));
        let e = enumerate_make_new(type_enumerate(), 1, 0, &[lst]);
        let t0 = enumerate_iternext(e);
        let (_, items) = objtuple::tuple_get(t0);
        assert_eq!(obj::small_int_value(items[0]), 0);
        assert_eq!(obj::small_int_value(items[1]), 10);
        let t1 = enumerate_iternext(e);
        let (_, items) = objtuple::tuple_get(t1);
        assert_eq!(obj::small_int_value(items[0]), 1);
        assert_eq!(obj::small_int_value(items[1]), 20);
        assert_eq!(enumerate_iternext(e), obj::OBJ_STOP_ITERATION);
    }
}
