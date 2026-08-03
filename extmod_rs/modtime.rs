//! rewrite of extmod/modtime.c + extmod/modtime.h
// symmetry: done

use py_rs::bc::ModuleContext;
use py_rs::map::{self, MapElem};
use py_rs::malloc;
use py_rs::mpconfig;
use py_rs::mphal;
use py_rs::obj::{self, Obj, ObjBase, ObjType, TYPE_FLAG_BUILTIN_FUN};
use py_rs::objdict;
use py_rs::objfloat::{self, MpFloat};
use py_rs::objint;
use py_rs::objmodule;
use py_rs::qstr;
use py_rs::raise::{self, MpRaise};

type BuiltinFn0 = fn() -> Obj;
type BuiltinFn1 = fn(Obj) -> Obj;
type BuiltinFn2 = fn(Obj, Obj) -> Obj;

#[repr(C)]
struct ObjFunBuiltin0 {
    base: ObjBase,
    fun: BuiltinFn0,
}
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

static mut F0: [*const (); 1] = [call0 as *const ()];
static mut F1: [*const (); 1] = [call1 as *const ()];
static mut F2: [*const (); 1] = [call2 as *const ()];
static T0: ObjType = ObjType {
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
    slots: unsafe { F0.as_ptr() },
};
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

fn call0(s: Obj, n: usize, k: usize, a: &[Obj]) -> Obj {
    py_rs::argcheck::check_num(n, k, 0, 0, false);
    unsafe { ((*(obj::as_ptr(s) as *const ObjFunBuiltin0)).fun)() }
}
fn call1(s: Obj, n: usize, k: usize, a: &[Obj]) -> Obj {
    py_rs::argcheck::check_num(n, k, 1, 1, false);
    unsafe { ((*(obj::as_ptr(s) as *const ObjFunBuiltin1)).fun)(a[0]) }
}
fn call2(s: Obj, n: usize, k: usize, a: &[Obj]) -> Obj {
    py_rs::argcheck::check_num(n, k, 2, 2, false);
    unsafe { ((*(obj::as_ptr(s) as *const ObjFunBuiltin2)).fun)(a[0], a[1]) }
}
fn mk0(f: BuiltinFn0) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltin0>().expect("time fn0");
    unsafe {
        (*o).base.type_ = &T0;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltin0 as *const ())
    }
}
fn mk1(f: BuiltinFn1) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltin1>().expect("time fn1");
    unsafe {
        (*o).base.type_ = &T1;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltin1 as *const ())
    }
}
fn mk2(f: BuiltinFn2) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltin2>().expect("time fn2");
    unsafe {
        (*o).base.type_ = &T2;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltin2 as *const ())
    }
}

const TICKS_PERIOD: u64 = mpconfig::PY_TIME_TICKS_PERIOD;

fn ticks_mask(v: u64) -> Obj {
    obj::new_small_int((v & (TICKS_PERIOD - 1)) as isize)
}

fn time_time() -> Obj {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    if mpconfig::PY_BUILTINS_FLOAT {
        objfloat::new_float(secs as MpFloat)
    } else {
        obj::new_small_int(secs as isize)
    }
}

fn time_time_ns() -> Obj {
    objint::new_int_from_ull(mphal::time_ns())
}

fn time_sleep(seconds_o: Obj) -> Obj {
    let ms = if mpconfig::PY_BUILTINS_FLOAT {
        (1000.0 * objfloat::get_float(seconds_o)) as u32
    } else {
        1000 * obj::get_int(seconds_o) as u32
    };
    mphal::delay_ms(ms as usize);
    obj::CONST_NONE
}

fn time_sleep_ms(arg: Obj) -> Obj {
    let ms = obj::get_int(arg);
    if ms >= 0 {
        if mpconfig::PY_MACHINE_TIMER {
            shared_rs::runtime::softtimer::delay_ms(ms as u32);
        } else {
            mphal::delay_ms(ms as usize);
        }
    }
    obj::CONST_NONE
}

fn time_sleep_us(arg: Obj) -> Obj {
    let us = obj::get_int(arg);
    if us > 0 {
        mphal::delay_us(us as usize);
    }
    obj::CONST_NONE
}

fn time_ticks_ms() -> Obj {
    ticks_mask(mphal::ticks_ms() as u64)
}

fn time_ticks_us() -> Obj {
    ticks_mask(mphal::ticks_us() as u64)
}

fn time_ticks_cpu() -> Obj {
    ticks_mask(mphal::ticks_cpu() as u64)
}

fn time_ticks_diff(end_in: Obj, start_in: Obj) -> Obj {
    let start = obj::small_int_value(start_in) as u64;
    let end = obj::small_int_value(end_in) as u64;
    let half = TICKS_PERIOD / 2;
    let diff = ((end.wrapping_sub(start).wrapping_add(half)) & (TICKS_PERIOD - 1))
        .wrapping_sub(half) as i64;
    obj::new_small_int(diff as isize)
}

fn time_ticks_add(ticks_in: Obj, delta_in: Obj) -> Obj {
    let ticks = obj::small_int_value(ticks_in) as u64;
    let delta = obj::get_int(delta_in) as u64;
    let half = TICKS_PERIOD / 2;
    if delta + half - 1 >= TICKS_PERIOD - 1 {
        raise::raise(MpRaise::OverflowError("ticks interval overflow"));
    }
    ticks_mask(ticks.wrapping_add(delta))
}

/// Register built-in `time` module (`MP_REGISTER_EXTENSIBLE_MODULE`).
pub fn init_module() -> Obj {
    if !mpconfig::PY_TIME {
        return obj::OBJ_NULL;
    }
    let mut table = vec![
        MapElem {
            key: obj::new_qstr(qstr::from_str("__name__")),
            value: obj::new_qstr(qstr::from_str("time")),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("sleep")),
            value: mk1(time_sleep),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("sleep_ms")),
            value: mk1(time_sleep_ms),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("sleep_us")),
            value: mk1(time_sleep_us),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("ticks_ms")),
            value: mk0(time_ticks_ms),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("ticks_us")),
            value: mk0(time_ticks_us),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("ticks_cpu")),
            value: mk0(time_ticks_cpu),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("ticks_add")),
            value: mk2(time_ticks_add),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("ticks_diff")),
            value: mk2(time_ticks_diff),
        },
    ];
    if mpconfig::PY_TIME_TIME_TIME_NS {
        table.push(MapElem {
            key: obj::new_qstr(qstr::from_str("time")),
            value: mk0(time_time),
        });
        table.push(MapElem {
            key: obj::new_qstr(qstr::from_str("time_ns")),
            value: mk0(time_time_ns),
        });
    }
    let ctx = malloc::new_obj::<ModuleContext>().expect("time module");
    let dict = objdict::new_dict(table.len());
    unsafe {
        map::init_fixed_table(&mut (*objdict::dict_ptr(dict)).map, table);
        (*ctx).module.base.type_ = objmodule::type_module();
        (*ctx).module.globals = objdict::dict_ptr(dict);
        (*ctx).constants = Default::default();
    }
    let module = obj::from_ptr(ctx as *const ModuleContext as *const ());
    objmodule::register_builtin_module(qstr::from_str("time"), module);
    module
}
