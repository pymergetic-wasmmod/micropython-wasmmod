//! rewrite of extmod/machine_pulse.c
// symmetry: done

use py_rs::mpconfig;
use py_rs::mphal;
use py_rs::obj::{self, Obj, ObjBase, ObjType, TYPE_FLAG_BUILTIN_FUN};
use py_rs::raise::{self, MpRaise};

use crate::virtpin::{self, has_pin_protocol};

type BuiltinFnVar = fn(usize, &[Obj]) -> Obj;

#[repr(C)]
struct ObjFunBuiltinVar {
    base: ObjBase,
    min_args: u8,
    max_args: u8,
    fun: BuiltinFnVar,
}

static mut FV: [*const (); 1] = [callv as *const ()];
static TV: ObjType = ObjType {
    base: ObjBase {
        type_: core::ptr::null(),
    },
    flags: TYPE_FLAG_BUILTIN_FUN,
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
    slots: unsafe { FV.as_ptr() },
};

fn callv(s: Obj, n: usize, k: usize, a: &[Obj]) -> Obj {
    let self_ = unsafe { &*(obj::as_ptr(s) as *const ObjFunBuiltinVar) };
    py_rs::argcheck::check_num(
        n,
        k,
        self_.min_args as usize,
        self_.max_args as usize,
        false,
    );
    (self_.fun)(n, a)
}

fn mkv(min: u8, max: u8, f: BuiltinFnVar) -> Obj {
    let o = py_rs::malloc::new_obj::<ObjFunBuiltinVar>().expect("machine_pulse fn");
    unsafe {
        (*o).base.type_ = &TV;
        (*o).min_args = min;
        (*o).max_args = max;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltinVar as *const ())
    }
}

/// `machine_time_pulse_us`
pub fn time_pulse_us(pin: Obj, mut pulse_level: i32, timeout_us: u32) -> isize {
    let mut nchanges = 2u32;
    let mut start = mphal::ticks_us();
    loop {
        let t = mphal::ticks_us();
        let pin_value = virtpin::virtual_pin_read(pin);

        if pin_value == pulse_level {
            pulse_level = 1 - pulse_level;
            nchanges -= 1;
            if nchanges == 0 {
                return (t - start) as isize;
            }
            start = t;
        } else if t.wrapping_sub(start) >= timeout_us as usize {
            return -(nchanges as isize);
        }
    }
}

fn time_pulse_us_py(n: usize, args: &[Obj]) -> Obj {
    if !has_pin_protocol(args[0]) {
        raise::raise(MpRaise::TypeError("pin"));
    }
    let level = if obj::is_true(args[1]) { 1 } else { 0 };
    let timeout_us = if n > 2 {
        obj::get_int(args[2]) as u32
    } else {
        1_000_000
    };
    obj::new_small_int(time_pulse_us(args[0], level, timeout_us))
}

/// Built-in `machine.time_pulse_us` object.
pub fn time_pulse_us_obj() -> Obj {
    mkv(2, 3, time_pulse_us_py)
}

pub fn enabled() -> bool {
    mpconfig::PY_MACHINE && mpconfig::PY_MACHINE_PULSE
}
