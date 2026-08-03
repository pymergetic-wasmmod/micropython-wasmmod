//! rewrite of py/objclosure.c
// symmetry: done

use crate::malloc;
use crate::mpconfig;
use crate::mpprint::{self, Print, PrintKind};
use crate::obj::{self, Obj, ObjBase, ObjType, TYPE_FLAG_BINDS_SELF};
use crate::qstr::{self, Qstr};
use crate::runtime;

#[repr(C)]
pub struct ObjClosure {
    pub base: ObjBase,
    pub fun: Obj,
    pub n_closed: usize,
    // flexible array: closed[]
}

static mut CLOSURE_SLOTS: [*const (); 3] = [
    closure_call as *const (),
    core::ptr::null(),
    core::ptr::null(),
];

static TYPE: ObjType = ObjType {
    base: ObjBase { type_: core::ptr::null() },
    flags: TYPE_FLAG_BINDS_SELF,
    name: 0,
    slot_index_make_new: 0,
    slot_index_print: if mpconfig::ERROR_REPORTING == mpconfig::ERROR_REPORTING_DETAILED {
        2
    } else {
        0
    },
    slot_index_call: 1,
    slot_index_unary_op: 0,
    slot_index_binary_op: 0,
    slot_index_attr: if mpconfig::PY_FUNCTION_ATTRS { 3 } else { 0 },
    slot_index_subscr: 0,
    slot_index_iter: 0,
    slot_index_buffer: 0,
    slot_index_protocol: 0,
    slot_index_parent: 0,
    slot_index_locals_dict: 0,
    slots: unsafe { CLOSURE_SLOTS.as_ptr() },
};

static CLOSURE_INIT: std::sync::OnceLock<()> = std::sync::OnceLock::new();

fn init_closure_type() {
    CLOSURE_INIT.get_or_init(|| {
        unsafe {
            if mpconfig::ERROR_REPORTING == mpconfig::ERROR_REPORTING_DETAILED {
                CLOSURE_SLOTS[1] = closure_print as *const ();
            }
            if mpconfig::PY_FUNCTION_ATTRS {
                CLOSURE_SLOTS[2] = closure_attr as *const ();
            }
        }
    });
}

pub fn type_closure() -> &'static ObjType {
    init_closure_type();
    &TYPE
}

fn closure_ptr(o: Obj) -> *mut ObjClosure {
    obj::as_ptr(o) as *mut ObjClosure
}

fn closed_slice(o: &ObjClosure) -> &[Obj] {
    unsafe {
        let p = (o as *const ObjClosure).add(1) as *const Obj;
        std::slice::from_raw_parts(p, o.n_closed)
    }
}

fn closed_slice_mut(o: &mut ObjClosure) -> &mut [Obj] {
    unsafe {
        let p = (o as *mut ObjClosure).add(1) as *mut Obj;
        std::slice::from_raw_parts_mut(p, o.n_closed)
    }
}

fn closure_call(self_in: Obj, n_args: usize, n_kw: usize, args: &[Obj]) -> Obj {
    let self_ = unsafe { &*closure_ptr(self_in) };
    let n_total = self_.n_closed + n_args + 2 * n_kw;
    if n_total <= 5 {
        let mut args2 = [obj::OBJ_NULL; 5];
        let closed = closed_slice(self_);
        args2[..self_.n_closed].copy_from_slice(closed);
        args2[self_.n_closed..n_total].copy_from_slice(args);
        runtime::call_function_n_kw(self_.fun, self_.n_closed + n_args, n_kw, &args2[..n_total])
    } else {
        let mut args2 = vec![obj::OBJ_NULL; n_total];
        let closed = closed_slice(self_);
        args2[..self_.n_closed].copy_from_slice(closed);
        args2[self_.n_closed..].copy_from_slice(args);
        runtime::call_function_n_kw(self_.fun, self_.n_closed + n_args, n_kw, &args2)
    }
}

fn closure_print(print: &Print, o_in: Obj, _kind: PrintKind) {
    let o = unsafe { &*closure_ptr(o_in) };
    mpprint::print_str(print, "<closure ");
    obj::print_helper(print, o.fun, PrintKind::Repr);
    mpprint::print_str(print, " at ");
    mpprint::print_str(print, &format!("{o_in:?}"));
    mpprint::print_str(print, ", n_closed=");
    mpprint::print_str(print, &o.n_closed.to_string());
    mpprint::print_str(print, " ");
    for item in closed_slice(o) {
        if *item == obj::OBJ_NULL {
            mpprint::print_str(print, "(nil)");
        } else {
            obj::print_helper(print, *item, PrintKind::Repr);
        }
        mpprint::print_str(print, " ");
    }
    mpprint::print_str(print, ">");
}

fn closure_attr(self_in: Obj, attr: Qstr, dest: &mut [Obj; 2]) {
    let o = unsafe { &*closure_ptr(self_in) };
    runtime::load_method_maybe(o.fun, attr, dest);
}

/// `mp_obj_new_closure`
pub fn new_closure(fun: Obj, n_closed_over: usize, closed: &[Obj]) -> Obj {
    let extra = n_closed_over * core::mem::size_of::<Obj>();
    let o = obj::malloc_var::<ObjClosure>(extra, type_closure()) as *mut ObjClosure;
    unsafe {
        (*o).fun = fun;
        (*o).n_closed = n_closed_over;
        closed_slice_mut(&mut *o).copy_from_slice(closed);
        obj::from_ptr(o as *const ObjClosure as *const ())
    }
}
