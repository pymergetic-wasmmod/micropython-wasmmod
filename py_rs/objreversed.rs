//! rewrite of py/objreversed.c
// symmetry: done

use crate::argcheck;
use crate::malloc;
use crate::mpconfig;
use crate::obj::{self, Obj, ObjBase, ObjType, OBJ_SENTINEL, TYPE_FLAG_ITER_IS_ITERNEXT};
use crate::qstr;
use crate::raise::{self, MpRaise};
use crate::runtime;

#[repr(C)]
pub struct ObjReversed {
    pub base: ObjBase,
    pub seq: Obj,
    /// Current index plus 1; 0 means exhausted.
    pub cur_index: usize,
}

static mut REV_SLOTS: [*const (); 2] = [reversed_make_new as *const (), reversed_iternext as *const ()];

static mut TYPE_REVERSED: ObjType = ObjType {
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
    slots: unsafe { REV_SLOTS.as_ptr() },
};

static INIT: std::sync::OnceLock<()> = std::sync::OnceLock::new();

fn init_type() {
    INIT.get_or_init(|| unsafe {
        TYPE_REVERSED.name = qstr::from_str("reversed");
    });
}

pub fn type_reversed() -> &'static ObjType {
    init_type();
    unsafe { &TYPE_REVERSED }
}

fn reversed_make_new(type_in: &ObjType, n_args: usize, n_kw: usize, args: &[Obj]) -> Obj {
    argcheck::check_num(n_args, n_kw, 1, 1, false);
    let mut dest = [obj::OBJ_NULL, obj::OBJ_NULL];
    runtime::load_method_maybe(args[0], qstr::from_str("__reversed__"), &mut dest);
    if dest[0] != obj::OBJ_NULL {
        return runtime::call_method_n_kw(0, 0, &dest);
    }
    let o = malloc::new_obj::<ObjReversed>().expect("reversed alloc");
    unsafe {
        (*o).base.type_ = type_in as *const ObjType;
        (*o).seq = args[0];
        (*o).cur_index = obj::get_int(obj::len(args[0])) as usize;
        obj::from_ptr(o as *const ObjReversed as *const ())
    }
}

fn reversed_iternext(self_in: Obj) -> Obj {
    if !obj::is_exact_type(self_in, type_reversed()) {
        raise::raise(MpRaise::TypeError("reversed iternext"));
    }
    let self_ = unsafe { &mut *(obj::as_ptr(self_in) as *mut ObjReversed) };
    if self_.cur_index == 0 {
        return obj::OBJ_STOP_ITERATION;
    }
    self_.cur_index -= 1;
    obj::subscr(self_.seq, obj::new_small_int(self_.cur_index as isize), OBJ_SENTINEL)
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
    fn reversed_list() {
        setup();
        let lst = objlist::new_list(3, Some(&[
            obj::new_small_int(1),
            obj::new_small_int(2),
            obj::new_small_int(3),
        ]));
        let r = reversed_make_new(type_reversed(), 1, 0, &[lst]);
        assert_eq!(obj::small_int_value(reversed_iternext(r)), 3);
        assert_eq!(obj::small_int_value(reversed_iternext(r)), 2);
        assert_eq!(obj::small_int_value(reversed_iternext(r)), 1);
        assert_eq!(reversed_iternext(r), obj::OBJ_STOP_ITERATION);
    }
}
