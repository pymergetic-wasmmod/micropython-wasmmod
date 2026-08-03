//! rewrite of extmod/modonewire.c
// symmetry: done

use py_rs::bc::ModuleContext;
use py_rs::malloc;
use py_rs::map::{self, MapElem};
use py_rs::mpconfig;
use py_rs::mphal;
use py_rs::obj::{self, BufferInfo, Obj, ObjBase, ObjType, TYPE_FLAG_BUILTIN_FUN};
use py_rs::objdict;
use py_rs::objmodule;
use py_rs::qstr;
use py_rs::raise::{self, MpRaise};

use crate::virtpin::{self, has_pin_protocol};

type BuiltinFn1 = fn(Obj) -> Obj;
type BuiltinFn2 = fn(Obj, Obj) -> Obj;

#[repr(C)]
struct ObjFunBuiltin1 {
    base: ObjBase,
    fun: BuiltinFn1,
}
#[repr(C)]
struct ObjFunBuiltin2 {
    base: ObjBase,
    fun: BuiltinFn2,
}

static mut F1: [*const (); 1] = [call1 as *const ()];
static mut F2: [*const (); 1] = [call2 as *const ()];
static T1: ObjType = ObjType {
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
    slots: unsafe { F1.as_ptr() },
};
static T2: ObjType = ObjType {
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
    slots: unsafe { F2.as_ptr() },
};

fn call1(s: Obj, n: usize, k: usize, a: &[Obj]) -> Obj {
    py_rs::argcheck::check_num(n, k, 1, 1, false);
    unsafe { ((*(obj::as_ptr(s) as *const ObjFunBuiltin1)).fun)(a[0]) }
}
fn call2(s: Obj, n: usize, k: usize, a: &[Obj]) -> Obj {
    py_rs::argcheck::check_num(n, k, 2, 2, false);
    unsafe { ((*(obj::as_ptr(s) as *const ObjFunBuiltin2)).fun)(a[0], a[1]) }
}
fn mk1(f: BuiltinFn1) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltin1>().expect("onewire fn1");
    unsafe {
        (*o).base.type_ = &T1;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltin1 as *const ())
    }
}
fn mk2(f: BuiltinFn2) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltin2>().expect("onewire fn2");
    unsafe {
        (*o).base.type_ = &T2;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltin2 as *const ())
    }
}

const TIMING_RESET1: usize = 480;
const TIMING_RESET2: usize = 70;
const TIMING_RESET3: usize = 410;
const TIMING_READ1: usize = 6;
const TIMING_READ2: usize = 9;
const TIMING_READ3: usize = 55;
const TIMING_WRITE1: usize = 6;
const TIMING_WRITE2: usize = 54;
const TIMING_WRITE3: usize = 10;

fn get_pin_obj(pin_in: Obj) -> Obj {
    if !has_pin_protocol(pin_in) {
        raise::raise(MpRaise::TypeError("pin"));
    }
    pin_in
}

fn pin_od_low(pin: Obj) {
    virtpin::virtual_pin_write(pin, 0);
}

fn pin_od_high(pin: Obj) {
    virtpin::virtual_pin_write(pin, 1);
}

fn pin_read(pin: Obj) -> i32 {
    virtpin::virtual_pin_read(pin)
}

fn onewire_bus_reset(pin: Obj) -> bool {
    pin_od_low(pin);
    mphal::delay_us(TIMING_RESET1);
    let state = mphal::begin_atomic_section();
    pin_od_high(pin);
    mphal::delay_us(TIMING_RESET2);
    let status = pin_read(pin) == 0;
    mphal::end_atomic_section(state);
    mphal::delay_us(TIMING_RESET3);
    status
}

fn onewire_bus_readbit(pin: Obj) -> i32 {
    pin_od_high(pin);
    let state = mphal::begin_atomic_section();
    pin_od_low(pin);
    mphal::delay_us(TIMING_READ1);
    pin_od_high(pin);
    mphal::delay_us(TIMING_READ2);
    let value = pin_read(pin);
    mphal::end_atomic_section(state);
    mphal::delay_us(TIMING_READ3);
    value
}

fn onewire_bus_writebit(pin: Obj, value: i32) {
    let state = mphal::begin_atomic_section();
    pin_od_low(pin);
    mphal::delay_us(TIMING_WRITE1);
    if value != 0 {
        pin_od_high(pin);
    }
    mphal::delay_us(TIMING_WRITE2);
    pin_od_high(pin);
    mphal::delay_us(TIMING_WRITE3);
    mphal::end_atomic_section(state);
}

fn onewire_reset(pin_in: Obj) -> Obj {
    let pin = get_pin_obj(pin_in);
    obj::new_bool(onewire_bus_reset(pin))
}

fn onewire_readbit(pin_in: Obj) -> Obj {
    let pin = get_pin_obj(pin_in);
    obj::new_small_int(onewire_bus_readbit(pin) as isize)
}

fn onewire_readbyte(pin_in: Obj) -> Obj {
    let pin = get_pin_obj(pin_in);
    let mut value = 0u8;
    for i in 0..8 {
        value |= (onewire_bus_readbit(pin) as u8) << i;
    }
    obj::new_small_int(value as isize)
}

fn onewire_writebit(pin_in: Obj, value_in: Obj) -> Obj {
    let pin = get_pin_obj(pin_in);
    onewire_bus_writebit(pin, obj::get_int_truncated(value_in) as i32);
    obj::CONST_NONE
}

fn onewire_writebyte(pin_in: Obj, value_in: Obj) -> Obj {
    let pin = get_pin_obj(pin_in);
    let mut value = obj::get_int_truncated(value_in) as u8;
    for _ in 0..8 {
        onewire_bus_writebit(pin, (value & 1) as i32);
        value >>= 1;
    }
    obj::CONST_NONE
}

fn onewire_crc8(data: Obj) -> Obj {
    let mut bufinfo = BufferInfo::default();
    obj::get_buffer_raise(data, &mut bufinfo, obj::BUFFER_READ);
    let mut crc = 0u8;
    unsafe {
        for i in 0..bufinfo.len {
            let mut byte = *bufinfo.buf.add(i);
            for _ in 0..8 {
                let fb_bit = (crc ^ byte) & 0x01;
                if fb_bit == 0x01 {
                    crc ^= 0x18;
                }
                crc = (crc >> 1) & 0x7f;
                if fb_bit == 0x01 {
                    crc |= 0x80;
                }
                byte >>= 1;
            }
        }
    }
    obj::new_small_int(crc as isize)
}

/// Register built-in `_onewire` module (`MP_REGISTER_MODULE`).
pub fn init_module() -> Obj {
    if !mpconfig::PY_ONEWIRE {
        return obj::OBJ_NULL;
    }
    let table = [
        MapElem {
            key: obj::new_qstr(qstr::from_str("__name__")),
            value: obj::new_qstr(qstr::from_str("onewire")),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("reset")),
            value: mk1(onewire_reset),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("readbit")),
            value: mk1(onewire_readbit),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("readbyte")),
            value: mk1(onewire_readbyte),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("writebit")),
            value: mk2(onewire_writebit),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("writebyte")),
            value: mk2(onewire_writebyte),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("crc8")),
            value: mk1(onewire_crc8),
        },
    ];
    let ctx = malloc::new_obj::<ModuleContext>().expect("onewire module");
    let dict = objdict::new_dict(table.len());
    unsafe {
        map::init_fixed_table(&mut (*objdict::dict_ptr(dict)).map, table.to_vec());
        (*ctx).module.base.type_ = objmodule::type_module();
        (*ctx).module.globals = objdict::dict_ptr(dict);
        (*ctx).constants = Default::default();
    }
    let module = obj::from_ptr(ctx as *const ModuleContext as *const ());
    objmodule::register_builtin_module(qstr::from_str("_onewire"), module);
    module
}

#[cfg(test)]
mod tests {
    use super::*;
    use py_rs::gc;
    use py_rs::mpstate;
    use py_rs::objstr;
    use py_rs::qstr;

    fn setup() {
        let _ = gc::init();
        qstr::init();
        mpstate::init();
    }

    #[test]
    fn crc8_empty() {
        setup();
        let data = objstr::new_bytes(&[]);
        assert_eq!(obj::get_int(onewire_crc8(data)), 0);
    }

    #[test]
    fn crc8_known_vector() {
        setup();
        let data = objstr::new_bytes(b"\x01\x02\x03");
        let crc = obj::get_int(onewire_crc8(data));
        assert_eq!(crc, 0xd8);
    }
}
