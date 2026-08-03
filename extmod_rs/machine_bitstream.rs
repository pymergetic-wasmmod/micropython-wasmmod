//! rewrite of extmod/machine_bitstream.c
// symmetry: done

use py_rs::mpconfig;
use py_rs::mphal;
use py_rs::obj::{self, BufferInfo, Obj, ObjBase, ObjType, TYPE_FLAG_BUILTIN_FUN};
use py_rs::raise::{self, MpRaise};

use crate::hal_pin;
use crate::virtpin;

type BuiltinFnVar = fn(usize, &[Obj]) -> Obj;

/// Timing is a 4-tuple of (high_time_0, low_time_0, high_time_1, low_time_1).
pub const MACHINE_BITSTREAM_TYPE_HIGH_LOW: i32 = 0;

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
    let o = py_rs::malloc::new_obj::<ObjFunBuiltinVar>().expect("machine_bitstream fn");
    unsafe {
        (*o).base.type_ = &TV;
        (*o).min_args = min;
        (*o).max_args = max;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltinVar as *const ())
    }
}

#[inline]
fn ns_to_us(ns: u32) -> usize {
    ((ns / 1000).max(1)) as usize
}

/// Soft host `machine_bitstream_high_low` — virtpin + `delay_us` (timing not cycle-accurate).
pub fn bitstream_high_low(pin: Obj, timing_ns: &[u32; 4], buf: &[u8]) {
    hal_pin::pin_output(pin);
    let _state = mphal::begin_atomic_section();
    for &byte in buf {
        for bit in (0..8).rev() {
            let one = (byte >> bit) & 1 != 0;
            let (high_ns, low_ns) = if one {
                (timing_ns[2], timing_ns[3])
            } else {
                (timing_ns[0], timing_ns[1])
            };
            virtpin::virtual_pin_write(pin, 1);
            mphal::delay_us(ns_to_us(high_ns));
            virtpin::virtual_pin_write(pin, 0);
            mphal::delay_us(ns_to_us(low_ns));
        }
    }
    mphal::end_atomic_section(_state);
}

fn machine_bitstream(n: usize, args: &[Obj]) -> Obj {
    let pin = hal_pin::get_pin_obj(args[0]);
    let encoding = obj::get_int(args[1]) as i32;
    let mut bufinfo = BufferInfo::default();
    obj::get_buffer_raise(args[3], &mut bufinfo, obj::BUFFER_READ);
    let buf = bufinfo.as_bytes();

    match encoding {
        MACHINE_BITSTREAM_TYPE_HIGH_LOW => {
            let (timing_len, timing) = obj::get_array(args[2]);
            if timing_len != 4 {
                raise::raise(MpRaise::ValueError("encoding"));
            }
            let mut timing_ns = [0u32; 4];
            for (i, t) in timing.iter().enumerate() {
                timing_ns[i] = obj::get_int(*t) as u32;
            }
            bitstream_high_low(pin, &timing_ns, buf);
        }
        _ => raise::raise(MpRaise::ValueError("encoding")),
    }
    obj::CONST_NONE
}

/// Built-in `machine.bitstream` object.
pub fn bitstream_obj() -> Obj {
    mkv(4, 4, machine_bitstream)
}

/// Board-specific `machine_bitstream` helpers — enabled when `PY_MACHINE_BITSTREAM != 0`.
pub fn enabled() -> bool {
    mpconfig::PY_MACHINE && mpconfig::PY_MACHINE_BITSTREAM != 0
}
