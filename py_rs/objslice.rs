//! rewrite of py/objslice.c
// symmetry: done

use crate::argcheck;
use crate::malloc;
use crate::misc::{max_isize, min_isize};
use crate::mpconfig;
use crate::mpprint::{self, Print, PrintKind};
use crate::obj::{self, Int, Obj, ObjBase, ObjType, TYPE_FLAG_BINDS_SELF, TYPE_FLAG_BUILTIN_FUN};
use crate::qstr;
use crate::raise::{self, MpRaise};
use crate::runtime0::UnaryOp;

/// Resolved slice bounds (`mp_bound_slice_t`).
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct BoundSlice {
    pub start: Int,
    pub stop: Int,
    pub step: Int,
}

#[repr(C)]
pub struct ObjSlice {
    pub base: ObjBase,
    pub start: Obj,
    pub stop: Obj,
    pub step: Obj,
}

#[repr(C)]
struct ObjFunBuiltin2 {
    base: ObjBase,
    fun: fn(Obj, Obj) -> Obj,
}

static mut SLICE_INDICES_FUN_SLOTS: [*const (); 1] = [slice_indices_call as *const ()];

static TYPE_SLICE_INDICES_FUN: ObjType = ObjType {
    base: ObjBase {
        type_: core::ptr::null(),
    },
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
    slots: unsafe { SLICE_INDICES_FUN_SLOTS.as_ptr() },
};

static SLICE_FUN_INIT: std::sync::OnceLock<()> = std::sync::OnceLock::new();

fn init_slice_indices_fun() {
    SLICE_FUN_INIT.get_or_init(|| unsafe {
        SLICE_INDICES_FUN.base.type_ = &TYPE_SLICE_INDICES_FUN as *const ObjType;
    });
}

static mut SLICE_INDICES_FUN: ObjFunBuiltin2 = ObjFunBuiltin2 {
    base: ObjBase {
        type_: core::ptr::null(),
    },
    fun: slice_indices_method,
};

fn slice_indices_call(self_in: Obj, n_args: usize, n_kw: usize, args: &[Obj]) -> Obj {
    argcheck::check_num(n_args, n_kw, 2, 2, false);
    let self_ = unsafe { &*(obj::as_ptr(self_in) as *const ObjFunBuiltin2) };
    (self_.fun)(args[0], args[1])
}

static mut SLICE_SLOTS: [*const (); 3] = [
    slice_print as *const (),
    slice_unary_op as *const (),
    slice_attr as *const (),
];

static mut TYPE: ObjType = ObjType {
    base: ObjBase {
        type_: core::ptr::null(),
    },
    flags: obj::TYPE_FLAG_NONE,
    name: 0,
    slot_index_make_new: 0,
    slot_index_print: 1,
    slot_index_call: 0,
    slot_index_unary_op: 2,
    slot_index_binary_op: 0,
    slot_index_attr: if mpconfig::PY_BUILTINS_SLICE_ATTRS {
        3
    } else {
        0
    },
    slot_index_subscr: 0,
    slot_index_iter: 0,
    slot_index_buffer: 0,
    slot_index_protocol: 0,
    slot_index_parent: 0,
    slot_index_locals_dict: 0,
    slots: unsafe { SLICE_SLOTS.as_ptr() },
};

pub fn type_slice() -> &'static ObjType {
    static INIT: std::sync::OnceLock<()> = std::sync::OnceLock::new();
    INIT.get_or_init(|| unsafe {
        TYPE.name = crate::qstr::from_str("slice");
    });
    unsafe { &TYPE }
}

/// `mp_obj_new_slice`
pub fn new_slice(start: Obj, stop: Obj, step: Obj) -> Obj {
    debug_assert!(mpconfig::PY_BUILTINS_SLICE);
    let o = malloc::new_obj::<ObjSlice>().expect("objslice alloc");
    unsafe {
        (*o).base.type_ = type_slice() as *const ObjType;
        (*o).start = start;
        (*o).stop = stop;
        (*o).step = step;
        obj::from_ptr(o as *const ObjSlice as *const ())
    }
}

pub fn slice_print(print: &Print, o_in: Obj, _kind: PrintKind) {
    let o = unsafe { &*(obj::as_ptr(o_in) as *const ObjSlice) };
    mpprint::print_str(print, "slice(");
    obj::print_helper(print, o.start, PrintKind::Repr);
    mpprint::print_str(print, ", ");
    obj::print_helper(print, o.stop, PrintKind::Repr);
    mpprint::print_str(print, ", ");
    obj::print_helper(print, o.step, PrintKind::Repr);
    mpprint::print_str(print, ")");
}

pub fn slice_unary_op(_op: UnaryOp, _o_in: Obj) -> Obj {
    obj::OBJ_NULL
}

fn slice_indices_method(self_in: Obj, length_obj: Obj) -> Obj {
    let length = obj::get_int(length_obj);
    let mut bound = BoundSlice {
        start: 0,
        stop: 0,
        step: 1,
    };
    slice_indices(self_in, length, &mut bound);
    let results = [
        obj::new_small_int(bound.start),
        obj::new_small_int(bound.stop),
        obj::new_small_int(bound.step),
    ];
    crate::objtuple::new_tuple(3, Some(&results))
}

pub fn slice_attr(self_in: Obj, attr: qstr::Qstr, dest: &mut [Obj; 2]) {
    if dest[0] != obj::OBJ_NULL {
        return;
    }
    let self_ = unsafe { &*(obj::as_ptr(self_in) as *const ObjSlice) };
    if attr == qstr::from_str("start") {
        dest[0] = self_.start;
    } else if attr == qstr::from_str("stop") {
        dest[0] = self_.stop;
    } else if attr == qstr::from_str("step") {
        dest[0] = self_.step;
    } else if mpconfig::PY_BUILTINS_SLICE_INDICES && attr == qstr::from_str("indices") {
        init_slice_indices_fun();
        dest[0] = obj::from_ptr(&raw const SLICE_INDICES_FUN as *const ObjFunBuiltin2 as *const ());
        dest[1] = self_in;
    }
}

/// `mp_obj_slice_indices`
pub fn slice_indices(self_in: Obj, length: Int, result: &mut BoundSlice) {
    debug_assert!(mpconfig::PY_BUILTINS_SLICE);
    let self_ = unsafe { &*(obj::as_ptr(self_in) as *const ObjSlice) };
    let step = if self_.step == obj::CONST_NONE {
        1
    } else {
        let s = obj::get_int(self_.step);
        if s == 0 {
            raise::raise(MpRaise::ValueError("slice step can't be zero"));
        }
        s
    };

    let (start, stop) = if step > 0 {
        let start = if self_.start == obj::CONST_NONE {
            0
        } else {
            let mut s = obj::get_int(self_.start);
            if s < 0 {
                s += length;
            }
            min_isize(length, max_isize(0, s))
        };
        let stop = if self_.stop == obj::CONST_NONE {
            length
        } else {
            let mut s = obj::get_int(self_.stop);
            if s < 0 {
                s += length;
            }
            min_isize(length, max_isize(0, s))
        };
        (start, stop)
    } else {
        let start = if self_.start == obj::CONST_NONE {
            length - 1
        } else {
            let mut s = obj::get_int(self_.start);
            if s < 0 {
                s += length;
            }
            min_isize(length - 1, max_isize(-1, s))
        };
        let stop = if self_.stop == obj::CONST_NONE {
            -1
        } else {
            let mut s = obj::get_int(self_.stop);
            if s < 0 {
                s += length;
            }
            min_isize(length - 1, max_isize(-1, s))
        };
        (start, stop)
    };

    result.start = start;
    result.stop = stop;
    result.step = step;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn positive_step_indices() {
        let s = new_slice(
            obj::CONST_NONE,
            obj::new_small_int(10),
            obj::new_small_int(2),
        );
        let mut r = BoundSlice {
            start: 0,
            stop: 0,
            step: 1,
        };
        slice_indices(s, 20, &mut r);
        assert_eq!(r.start, 0);
        assert_eq!(r.stop, 10);
        assert_eq!(r.step, 2);
    }

    #[test]
    fn negative_step_indices() {
        let s = new_slice(
            obj::new_small_int(-1),
            obj::CONST_NONE,
            obj::new_small_int(-1),
        );
        let mut r = BoundSlice {
            start: 0,
            stop: 0,
            step: 1,
        };
        slice_indices(s, 5, &mut r);
        assert_eq!(r.start, 4);
        assert_eq!(r.stop, -1);
        assert_eq!(r.step, -1);
    }
}
