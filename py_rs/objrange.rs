//! rewrite of py/objrange.c
// symmetry: done

use core::mem::size_of;

use crate::argcheck;
use crate::malloc;
use crate::mpconfig;
use crate::mpprint::{self, Print, PrintKind};
use crate::obj::{self, Int, Obj, ObjBase, ObjIterBuf, ObjType, OBJ_SENTINEL, TYPE_FLAG_ITER_IS_ITERNEXT};
use crate::objslice::{self, BoundSlice};
use crate::qstr::{self, Qstr};
use crate::raise::{self, MpRaise};
use crate::runtime0::{BinaryOp, UnaryOp};

#[repr(C)]
pub struct ObjRange {
    pub base: ObjBase,
    pub start: Int,
    pub stop: Int,
    pub step: Int,
}

#[repr(C)]
struct ObjRangeIt {
    base: ObjBase,
    cur: Int,
    stop: Int,
    step: Int,
}

static mut RANGE_IT_SLOTS: [*const (); 1] = [range_it_iternext as *const ()];

static TYPE_RANGE_IT: ObjType = ObjType {
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
    slots: unsafe { RANGE_IT_SLOTS.as_ptr() },
};

static mut RANGE_SLOTS: [*const (); 7] = [
    range_make_new as *const (),
    range_print as *const (),
    range_unary_op as *const (),
    range_subscr as *const (),
    range_getiter as *const (),
    core::ptr::null(),
    core::ptr::null(),
];

static TYPE: ObjType = ObjType {
    base: ObjBase { type_: core::ptr::null() },
    flags: obj::TYPE_FLAG_NONE,
    name: 0,
    slot_index_make_new: 1,
    slot_index_print: 2,
    slot_index_unary_op: 3,
    slot_index_binary_op: if mpconfig::PY_BUILTINS_RANGE_BINOP { 6 } else { 0 },
    slot_index_attr: if mpconfig::PY_BUILTINS_RANGE_ATTRS { 6 } else { 0 },
    slot_index_subscr: 4,
    slot_index_call: 0,
    slot_index_iter: 5,
    slot_index_buffer: 0,
    slot_index_protocol: 0,
    slot_index_parent: 0,
    slot_index_locals_dict: 0,
    slots: unsafe { RANGE_SLOTS.as_ptr() },
};

static INIT: std::sync::OnceLock<()> = std::sync::OnceLock::new();

fn init_range_type() {
    INIT.get_or_init(|| {
        unsafe {
            let mut idx = 6;
            if mpconfig::PY_BUILTINS_RANGE_BINOP {
                RANGE_SLOTS[6] = range_binary_op as *const ();
                idx = 7;
            }
            if mpconfig::PY_BUILTINS_RANGE_ATTRS {
                RANGE_SLOTS[if mpconfig::PY_BUILTINS_RANGE_BINOP { 7 } else { 6 }] =
                    range_attr as *const ();
            }
        }
    });
}

pub fn type_range() -> &'static ObjType {
    init_range_type();
    &TYPE
}

fn type_range_it() -> &'static ObjType {
    &TYPE_RANGE_IT
}

fn range_len(self_: &ObjRange) -> Int {
    let mut len = self_.stop - self_.start + self_.step;
    if self_.step > 0 {
        len -= 1;
    } else {
        len += 1;
    }
    len /= self_.step;
    if len < 0 { 0 } else { len }
}

fn range_it_iternext(self_in: Obj) -> Obj {
    let o = unsafe { &mut *(obj::as_ptr(self_in) as *mut ObjRangeIt) };
    if (o.step > 0 && o.cur < o.stop) || (o.step < 0 && o.cur > o.stop) {
        let cur = o.cur;
        o.cur += o.step;
        obj::new_small_int(cur)
    } else {
        obj::OBJ_STOP_ITERATION
    }
}

fn new_range_iterator(cur: Int, stop: Int, step: Int, iter_buf: *mut ObjIterBuf) -> Obj {
    debug_assert!(size_of::<ObjRangeIt>() <= size_of::<ObjIterBuf>());
    let o = unsafe { &mut *(iter_buf as *mut ObjRangeIt) };
    o.base.type_ = type_range_it() as *const ObjType;
    o.cur = cur;
    o.stop = stop;
    o.step = step;
    obj::from_ptr(iter_buf as *const ObjRangeIt as *const ())
}

fn range_print(print: &Print, self_in: Obj, _kind: PrintKind) {
    let self_ = unsafe { &*(obj::as_ptr(self_in) as *const ObjRange) };
    mpprint::print_str(print, "range(");
    mpprint::print_str(print, &self_.start.to_string());
    mpprint::print_str(print, ", ");
    mpprint::print_str(print, &self_.stop.to_string());
    if self_.step == 1 {
        mpprint::print_str(print, ")");
    } else {
        mpprint::print_str(print, ", ");
        mpprint::print_str(print, &self_.step.to_string());
        mpprint::print_str(print, ")");
    }
}

fn range_make_new(type_in: &ObjType, n_args: usize, n_kw: usize, args: &[Obj]) -> Obj {
    argcheck::check_num(n_args, n_kw, 1, 3, false);
    let o = malloc::new_obj::<ObjRange>().expect("range alloc");
    unsafe {
        (*o).base.type_ = type_in as *const ObjType;
        (*o).start = 0;
        (*o).step = 1;
        if n_args == 1 {
            (*o).stop = obj::get_int(args[0]);
        } else {
            (*o).start = obj::get_int(args[0]);
            (*o).stop = obj::get_int(args[1]);
            if n_args == 3 {
                (*o).step = obj::get_int(args[2]);
                if (*o).step == 0 {
                    raise::raise(MpRaise::ValueError("zero step"));
                }
            }
        }
        obj::from_ptr(o as *const ObjRange as *const ())
    }
}

fn range_unary_op(op: UnaryOp, self_in: Obj) -> Obj {
    let self_ = unsafe { &*(obj::as_ptr(self_in) as *const ObjRange) };
    let len = range_len(self_);
    match op {
        UnaryOp::Bool => obj::new_bool(len > 0),
        UnaryOp::Len => obj::new_int(len),
        _ => obj::OBJ_NULL,
    }
}

fn range_binary_op(op: BinaryOp, lhs_in: Obj, rhs_in: Obj) -> Obj {
    if !obj::is_exact_type(rhs_in, type_range()) || op != BinaryOp::Equal {
        return obj::OBJ_NULL;
    }
    let lhs = unsafe { &*(obj::as_ptr(lhs_in) as *const ObjRange) };
    let rhs = unsafe { &*(obj::as_ptr(rhs_in) as *const ObjRange) };
    let lhs_len = range_len(lhs);
    let rhs_len = range_len(rhs);
    obj::new_bool(
        lhs_len == rhs_len
            && (lhs_len == 0
                || (lhs.start == rhs.start && (lhs_len == 1 || lhs.step == rhs.step))),
    )
}

fn range_subscr(self_in: Obj, index: Obj, value: Obj) -> Obj {
    if value == obj::OBJ_NULL {
        raise::raise(MpRaise::TypeError("range object does not support item deletion"));
    }
    if value != OBJ_SENTINEL {
        return obj::OBJ_NULL;
    }
    let self_ = unsafe { &*(obj::as_ptr(self_in) as *const ObjRange) };
    let len = range_len(self_);
    if mpconfig::PY_BUILTINS_SLICE && obj::is_exact_type(index, objslice::type_slice()) {
        let mut slice = BoundSlice { start: 0, stop: 0, step: 1 };
        objslice::slice_indices(index, len, &mut slice);
        let o = malloc::new_obj::<ObjRange>().expect("range slice alloc");
        unsafe {
            (*o).base.type_ = type_range() as *const ObjType;
            (*o).start = self_.start + slice.start * self_.step;
            (*o).stop = self_.start + slice.stop * self_.step;
            (*o).step = slice.step * self_.step;
            obj::from_ptr(o as *const ObjRange as *const ())
        }
    } else {
        let index_val = obj::get_index(type_range(), len as usize, index, false);
        obj::new_int(self_.start + index_val as Int * self_.step)
    }
}

fn range_getiter(o_in: Obj, iter_buf: *mut ObjIterBuf) -> Obj {
    let o = unsafe { &*(obj::as_ptr(o_in) as *const ObjRange) };
    new_range_iterator(o.start, o.stop, o.step, iter_buf)
}

fn range_attr(o_in: Obj, attr: Qstr, dest: &mut [Obj; 2]) {
    if dest[0] != obj::OBJ_NULL {
        return;
    }
    let o = unsafe { &*(obj::as_ptr(o_in) as *const ObjRange) };
    if attr == qstr::from_str("start") {
        dest[0] = obj::new_int(o.start);
    } else if attr == qstr::from_str("stop") {
        dest[0] = obj::new_int(o.stop);
    } else if attr == qstr::from_str("step") {
        dest[0] = obj::new_int(o.step);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gc;

    fn setup() {
        let _ = gc::init();
    }

    #[test]
    fn range_len_and_index() {
        setup();
        let r = range_make_new(type_range(), 2, 0, &[obj::new_small_int(0), obj::new_small_int(10)]);
        assert_eq!(obj::get_int(range_unary_op(UnaryOp::Len, r)), 10);
        let v = range_subscr(r, obj::new_small_int(3), OBJ_SENTINEL);
        assert_eq!(obj::small_int_value(v), 3);
    }

    #[test]
    fn range_iter_yields_values() {
        setup();
        let r = range_make_new(type_range(), 2, 0, &[obj::new_small_int(1), obj::new_small_int(4)]);
        let mut buf = obj::ObjIterBuf {
            base: ObjBase { type_: core::ptr::null() },
            buf: [obj::OBJ_NULL; 3],
        };
        let it = range_getiter(r, &mut buf as *mut _);
        assert_eq!(obj::small_int_value(range_it_iternext(it)), 1);
        assert_eq!(obj::small_int_value(range_it_iternext(it)), 2);
        assert_eq!(obj::small_int_value(range_it_iternext(it)), 3);
        assert_eq!(range_it_iternext(it), obj::OBJ_STOP_ITERATION);
    }
}
