//! rewrite of extmod/modmachine.c + extmod/modmachine.h
//! Host soft exports (mem*, idle, soft_reset, PinBase, Signal, SoftSPI/SoftI2C, Timer, PWM, UART, WDT, …) are wired here.
//! Port HAL (`Pin`, HW SPI/I2C, reset/freq/sleep/bootloader/unique_id, mem_backup) lives in per-port `modmachine` hooks.
// symmetry: done

use std::sync::OnceLock;

use py_rs::bc::ModuleContext;
use py_rs::map::{self, MapElem};
use py_rs::malloc;
use py_rs::mpconfig;
use py_rs::obj::{self, Obj, ObjBase, ObjType, TYPE_FLAG_BUILTIN_FUN};
use py_rs::objdict;
use py_rs::objexcept;
use py_rs::objmodule;
use py_rs::qstr;
use py_rs::raise::{self, MpRaise};

use crate::machine_bitstream;
use crate::machine_i2c;
use crate::machine_mem;
use crate::machine_pinbase;
use crate::machine_pulse;
use crate::machine_pwm;
use crate::machine_signal;
use crate::machine_spi;
use crate::machine_timer;
use crate::machine_uart;
use crate::machine_wdt;

type MachineIdleFn = fn();
type BuiltinFn0 = fn() -> Obj;
type BuiltinFnVar = fn(usize, &[Obj]) -> Obj;

static MACHINE_IDLE: OnceLock<MachineIdleFn> = OnceLock::new();

fn default_machine_idle() {}

/// Port hook for `mp_machine_idle`.
pub fn set_machine_idle_hook(hook: MachineIdleFn) {
    let _ = MACHINE_IDLE.set(hook);
}

fn machine_idle() {
    MACHINE_IDLE
        .get()
        .copied()
        .unwrap_or(default_machine_idle)();
}

#[repr(C)]
struct ObjFunBuiltin0 {
    base: ObjBase,
    fun: BuiltinFn0,
}
#[repr(C)]
struct ObjFunBuiltinVar {
    base: ObjBase,
    min_args: u8,
    max_args: u8,
    fun: BuiltinFnVar,
}

static mut F0: [*const (); 1] = [call0 as *const ()];
static mut FV: [*const (); 1] = [callv as *const ()];
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

fn call0(s: Obj, n: usize, k: usize, _a: &[Obj]) -> Obj {
    py_rs::argcheck::check_num(n, k, 0, 0, false);
    unsafe { ((*(obj::as_ptr(s) as *const ObjFunBuiltin0)).fun)() }
}
fn callv(s: Obj, n: usize, k: usize, a: &[Obj]) -> Obj {
    let self_ = unsafe { &*(obj::as_ptr(s) as *const ObjFunBuiltinVar) };
    py_rs::argcheck::check_num(n, k, self_.min_args as usize, self_.max_args as usize, false);
    (self_.fun)(n, a)
}
fn mk0(f: BuiltinFn0) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltin0>().expect("machine fn0");
    unsafe {
        (*o).base.type_ = &T0;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltin0 as *const ())
    }
}
fn mkv(min: u8, max: u8, f: BuiltinFnVar) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltinVar>().expect("machine fnv");
    unsafe {
        (*o).base.type_ = &TV;
        (*o).min_args = min;
        (*o).max_args = max;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltinVar as *const ())
    }
}

fn machine_idle_fn() -> Obj {
    machine_idle();
    obj::CONST_NONE
}

fn soft_reset(n: usize, args: &[Obj]) -> Obj {
    if n == 0 {
        raise::raise_obj(objexcept::new_exception(objexcept::type_system_exit()));
    }
    raise::raise_obj(objexcept::new_exception_args(
        objexcept::type_system_exit(),
        1,
        &[args[0]],
    ));
}

/// Register built-in `machine` module (`MP_REGISTER_EXTENSIBLE_MODULE`).
pub fn init_module() -> Obj {
    if !mpconfig::PY_MACHINE {
        return obj::OBJ_NULL;
    }
    let mut table = vec![MapElem {
        key: obj::new_qstr(qstr::from_str("__name__")),
        value: obj::new_qstr(qstr::from_str("machine")),
    }];
    if mpconfig::PY_MACHINE_MEMX {
        table.push(MapElem {
            key: obj::new_qstr(qstr::from_str("mem8")),
            value: machine_mem::mem8_obj(),
        });
        table.push(MapElem {
            key: obj::new_qstr(qstr::from_str("mem16")),
            value: machine_mem::mem16_obj(),
        });
        table.push(MapElem {
            key: obj::new_qstr(qstr::from_str("mem32")),
            value: machine_mem::mem32_obj(),
        });
    }
    if mpconfig::PY_SYS_EXIT {
        table.push(MapElem {
            key: obj::new_qstr(qstr::from_str("soft_reset")),
            value: mkv(0, 1, soft_reset),
        });
    }
    table.push(MapElem {
        key: obj::new_qstr(qstr::from_str("idle")),
        value: mk0(machine_idle_fn),
    });
    if mpconfig::PY_MACHINE_PIN_BASE {
        let ty = machine_pinbase::pinbase_type();
        table.push(MapElem {
            key: obj::new_qstr(qstr::from_str("PinBase")),
            value: obj::from_ptr(ty as *const ObjType as *const ()),
        });
    }
    if mpconfig::PY_MACHINE_SIGNAL {
        let ty = machine_signal::signal_type();
        table.push(MapElem {
            key: obj::new_qstr(qstr::from_str("Signal")),
            value: obj::from_ptr(ty as *const ObjType as *const ()),
        });
    }
    if mpconfig::PY_MACHINE_PULSE {
        table.push(MapElem {
            key: obj::new_qstr(qstr::from_str("time_pulse_us")),
            value: machine_pulse::time_pulse_us_obj(),
        });
    }
    if machine_bitstream::enabled() {
        table.push(MapElem {
            key: obj::new_qstr(qstr::from_str("bitstream")),
            value: machine_bitstream::bitstream_obj(),
        });
    }
    if mpconfig::PY_MACHINE_TIMER {
        let ty = machine_timer::timer_type();
        table.push(MapElem {
            key: obj::new_qstr(qstr::from_str("Timer")),
            value: obj::from_ptr(ty as *const ObjType as *const ()),
        });
    }
    if mpconfig::PY_MACHINE_SOFTI2C {
        let ty = machine_i2c::soft_i2c_type();
        table.push(MapElem {
            key: obj::new_qstr(qstr::from_str("SoftI2C")),
            value: obj::from_ptr(ty as *const ObjType as *const ()),
        });
    }
    if mpconfig::PY_MACHINE_SOFTSPI {
        let ty = machine_spi::soft_spi_type();
        table.push(MapElem {
            key: obj::new_qstr(qstr::from_str("SoftSPI")),
            value: obj::from_ptr(ty as *const ObjType as *const ()),
        });
    }
    if mpconfig::PY_MACHINE_UART {
        let ty = machine_uart::uart_type();
        table.push(MapElem {
            key: obj::new_qstr(qstr::from_str("UART")),
            value: obj::from_ptr(ty as *const ObjType as *const ()),
        });
    }
    if machine_wdt::enabled() {
        let ty = machine_wdt::wdt_type();
        table.push(MapElem {
            key: obj::new_qstr(qstr::from_str("WDT")),
            value: obj::from_ptr(ty as *const ObjType as *const ()),
        });
    }
    if machine_pwm::enabled() {
        let ty = machine_pwm::pwm_type();
        table.push(MapElem {
            key: obj::new_qstr(qstr::from_str("PWM")),
            value: obj::from_ptr(ty as *const ObjType as *const ()),
        });
    }
    let ctx = malloc::new_obj::<ModuleContext>().expect("machine module");
    let dict = objdict::new_dict(table.len());
    unsafe {
        map::init_fixed_table(&mut (*objdict::dict_ptr(dict)).map, table);
        (*ctx).module.base.type_ = objmodule::type_module();
        (*ctx).module.globals = objdict::dict_ptr(dict);
        (*ctx).constants = Default::default();
    }
    let module = obj::from_ptr(ctx as *const ModuleContext as *const ());
    objmodule::register_builtin_module(qstr::from_str("machine"), module);
    module
}

/// Register port-specific machine hooks (mem addr mapping, idle).
pub fn register_port_hooks(mem_get_addr: fn(Obj, usize) -> usize, idle: fn()) {
    machine_mem::set_mem_get_addr_hook(mem_get_addr);
    let _ = MACHINE_IDLE.set(idle);
}
