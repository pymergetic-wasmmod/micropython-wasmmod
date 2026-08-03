//! rewrite of py/objmap.c
// symmetry: done

use core::mem::size_of;

use crate::argcheck;
use crate::malloc;
use crate::obj::{self, Obj, ObjBase, ObjType, TYPE_FLAG_ITER_IS_ITERNEXT};
use crate::qstr;
use crate::raise::{self, MpRaise};
use crate::runtime;

#[repr(C)]
pub struct ObjMap {
    pub base: ObjBase,
    pub n_iters: usize,
    pub fun: Obj,
}

fn iters_ptr(o: *const ObjMap) -> *const Obj {
    unsafe { (o as *const u8).add(size_of::<ObjMap>()) as *const Obj }
}

static mut MAP_SLOTS: [*const (); 2] = [map_make_new as *const (), map_iternext as *const ()];

static mut TYPE_MAP: ObjType = ObjType {
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
    slots: unsafe { MAP_SLOTS.as_ptr() },
};

static INIT: std::sync::OnceLock<()> = std::sync::OnceLock::new();

fn init_type() {
    INIT.get_or_init(|| unsafe {
        TYPE_MAP.name = qstr::from_str("map");
    });
}

pub fn type_map() -> &'static ObjType {
    init_type();
    unsafe { &TYPE_MAP }
}

fn map_make_new(type_in: &ObjType, n_args: usize, n_kw: usize, args: &[Obj]) -> Obj {
    argcheck::check_num(n_args, n_kw, 2, usize::MAX, false);
    let n_iters = n_args - 1;
    let type_static: &'static ObjType = unsafe { &*(type_in as *const ObjType) };
    let o = obj::malloc_var::<ObjMap>(n_iters * size_of::<Obj>(), type_static);
    unsafe {
        (*o).n_iters = n_iters;
        (*o).fun = args[0];
        let iters = std::slice::from_raw_parts_mut(
            (o as *mut u8).add(size_of::<ObjMap>()) as *mut Obj,
            n_iters,
        );
        for (i, &arg) in args[1..].iter().enumerate() {
            iters[i] = runtime::getiter(arg, None);
        }
        obj::from_ptr(o as *const ObjMap as *const ())
    }
}

fn map_iternext(self_in: Obj) -> Obj {
    if !obj::is_exact_type(self_in, type_map()) {
        raise::raise(MpRaise::TypeError("map iternext"));
    }
    let self_ = unsafe { &*(obj::as_ptr(self_in) as *const ObjMap) };
    let mut nextses = vec![obj::OBJ_NULL; self_.n_iters];
    for i in 0..self_.n_iters {
        let next = runtime::iternext(unsafe { *iters_ptr(self_).add(i) });
        if next == obj::OBJ_STOP_ITERATION {
            return obj::OBJ_STOP_ITERATION;
        }
        nextses[i] = next;
    }
    runtime::call_function_n_kw(self_.fun, self_.n_iters, 0, &nextses)
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
    fn map_applies_fun() {
        setup();
        let module = modbuiltins::init_builtins_module();
        let globals = crate::objmodule::module_get_globals(module);
        let abs_key = obj::new_qstr(qstr::from_str("abs"));
        let abs_fn = crate::map::lookup(
            unsafe { &mut (*globals).map },
            abs_key,
            crate::map::LookupKind::Lookup,
        )
        .expect("abs builtin")
        .value;
        let lst = objlist::new_list(2, Some(&[
            obj::new_small_int(1),
            obj::new_small_int(2),
        ]));
        let m = map_make_new(type_map(), 2, 0, &[abs_fn, lst]);
        assert_eq!(obj::small_int_value(map_iternext(m)), 1);
        assert_eq!(obj::small_int_value(map_iternext(m)), 2);
        assert_eq!(map_iternext(m), obj::OBJ_STOP_ITERATION);
    }
}
