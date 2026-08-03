//! rewrite of py/objfilter.c
// symmetry: done

use crate::argcheck;
use crate::malloc;
use crate::mpconfig;
use crate::obj::{self, Obj, ObjBase, ObjType, TYPE_FLAG_ITER_IS_ITERNEXT};
use crate::qstr;
use crate::runtime;

#[repr(C)]
pub struct ObjFilter {
    pub base: ObjBase,
    pub fun: Obj,
    pub iter: Obj,
}

static mut FILTER_SLOTS: [*const (); 2] =
    [filter_make_new as *const (), filter_iternext as *const ()];

static mut TYPE_FILTER: ObjType = ObjType {
    base: ObjBase {
        type_: core::ptr::null(),
    },
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
    slots: unsafe { FILTER_SLOTS.as_ptr() },
};

static INIT: std::sync::OnceLock<()> = std::sync::OnceLock::new();

fn init_type() {
    INIT.get_or_init(|| unsafe {
        TYPE_FILTER.name = qstr::from_str("filter");
    });
}

pub fn type_filter() -> &'static ObjType {
    if !mpconfig::PY_BUILTINS_FILTER {
        init_type();
    } else {
        init_type();
    }
    unsafe { &TYPE_FILTER }
}

fn filter_make_new(type_in: &ObjType, n_args: usize, n_kw: usize, args: &[Obj]) -> Obj {
    argcheck::check_num(n_args, n_kw, 2, 2, false);
    let o = malloc::new_obj::<ObjFilter>().expect("filter alloc");
    unsafe {
        (*o).base.type_ = type_in as *const ObjType;
        (*o).fun = args[0];
        (*o).iter = runtime::getiter(args[1], None);
        obj::from_ptr(o as *const ObjFilter as *const ())
    }
}

fn filter_iternext(self_in: Obj) -> Obj {
    let self_ = unsafe { &*(obj::as_ptr(self_in) as *const ObjFilter) };
    loop {
        let next = runtime::iternext(self_.iter);
        if next == obj::OBJ_STOP_ITERATION {
            return obj::OBJ_STOP_ITERATION;
        }
        let val = if self_.fun != obj::CONST_NONE {
            runtime::call_function_n_kw(self_.fun, 1, 0, &[next])
        } else {
            next
        };
        if obj::is_true(val) {
            return next;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gc;
    use crate::modbuiltins;
    use crate::objlist;

    fn setup() {
        let _ = gc::init();
        runtime::init();
        let _ = modbuiltins::init_builtins_module();
        init_type();
    }

    #[test]
    fn filter_none_keeps_truthy() {
        setup();
        let lst = objlist::new_list(
            3,
            Some(&[
                obj::new_small_int(0),
                obj::new_small_int(1),
                obj::new_small_int(2),
            ]),
        );
        let f = filter_make_new(type_filter(), 2, 0, &[obj::CONST_NONE, lst]);
        let v = filter_iternext(f);
        assert_eq!(obj::small_int_value(v), 1);
        assert_eq!(obj::small_int_value(filter_iternext(f)), 2);
        assert_eq!(filter_iternext(f), obj::OBJ_STOP_ITERATION);
    }
}
