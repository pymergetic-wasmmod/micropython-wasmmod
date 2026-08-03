//! rewrite of extmod/machine_signal.c
// symmetry: done

use py_rs::argcheck;
use py_rs::malloc;
use py_rs::map::{self, MapElem};
use py_rs::mpconfig;
use py_rs::obj::{self, Obj, ObjBase, ObjType, TYPE_FLAG_BINDS_SELF, TYPE_FLAG_BUILTIN_FUN};
use py_rs::objdict::{self, ObjDict};
use py_rs::qstr;
use py_rs::raise::{self, MpRaise};
use py_rs::runtime;

use crate::virtpin::{self, has_pin_protocol, PinProtocol, MP_PIN_READ, MP_PIN_WRITE};

#[repr(C)]
struct MachineSignal {
    base: ObjBase,
    pin: Obj,
    invert: bool,
}

fn signal_ptr(o: Obj) -> *mut MachineSignal {
    obj::as_ptr(o) as *mut MachineSignal
}

fn signal_make_new(type_in: &ObjType, n_args: usize, n_kw: usize, args: &[Obj]) -> Obj {
    if n_args != 1 {
        raise::raise(MpRaise::TypeError(""));
    }
    let pin = args[0];
    if !has_pin_protocol(pin) {
        raise::raise(MpRaise::TypeError(""));
    }
    let mut invert = false;
    let invert_q = obj::new_qstr(qstr::from_str("invert"));
    let mut i = n_args;
    for _ in 0..n_kw {
        if args[i] == invert_q {
            invert = obj::is_true(args[i + 1]);
        } else {
            raise::raise(MpRaise::TypeError(""));
        }
        i += 2;
    }
    let o = malloc::new_obj::<MachineSignal>().expect("Signal");
    unsafe {
        (*o).base.type_ = type_in as *const ObjType;
        (*o).pin = pin;
        (*o).invert = invert;
        obj::from_ptr(o as *const MachineSignal as *const ())
    }
}

fn signal_ioctl(self_in: Obj, request: u32, arg: usize, _errcode: *mut i32) -> usize {
    let self_ = unsafe { &*signal_ptr(self_in) };
    match request {
        MP_PIN_READ => (virtpin::virtual_pin_read(self_.pin) as usize) ^ (self_.invert as usize),
        MP_PIN_WRITE => {
            virtpin::virtual_pin_write(self_.pin, (arg ^ (self_.invert as usize)) as i32);
            0
        }
        _ => usize::MAX,
    }
}

fn signal_call(self_in: Obj, n_args: usize, n_kw: usize, args: &[Obj]) -> Obj {
    argcheck::check_num(n_args, n_kw, 0, 1, false);
    if n_args == 0 {
        obj::new_small_int(virtpin::virtual_pin_read(self_in) as isize)
    } else {
        virtpin::virtual_pin_write(self_in, i32::from(obj::is_true(args[0])));
        obj::CONST_NONE
    }
}

static SIGNAL_PIN_P: PinProtocol = PinProtocol {
    ioctl: signal_ioctl,
};

type BuiltinFn1 = fn(Obj) -> Obj;
type BuiltinFnVar = fn(usize, &[Obj]) -> Obj;

#[repr(C)]
struct ObjFunBuiltin1 {
    base: ObjBase,
    fun: BuiltinFn1,
}
#[repr(C)]
struct ObjFunBuiltinVar {
    base: ObjBase,
    min_args: u8,
    max_args: u8,
    fun: BuiltinFnVar,
}

static mut F1: [*const (); 1] = [call1 as *const ()];
static mut FV: [*const (); 1] = [callv as *const ()];
static T1: ObjType = ObjType {
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
    slots: unsafe { F1.as_ptr() },
};
static TV: ObjType = ObjType {
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
    slots: unsafe { FV.as_ptr() },
};

fn call1(s: Obj, n: usize, k: usize, a: &[Obj]) -> Obj {
    argcheck::check_num(n, k, 1, 1, false);
    unsafe { ((*(obj::as_ptr(s) as *const ObjFunBuiltin1)).fun)(a[0]) }
}
fn callv(s: Obj, n: usize, k: usize, a: &[Obj]) -> Obj {
    let self_ = unsafe { &*(obj::as_ptr(s) as *const ObjFunBuiltinVar) };
    argcheck::check_num(
        n,
        k,
        self_.min_args as usize,
        self_.max_args as usize,
        false,
    );
    (self_.fun)(n, a)
}
fn mk1(f: BuiltinFn1) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltin1>().expect("signal fn1");
    unsafe {
        (*o).base.type_ = &T1;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltin1 as *const ())
    }
}
fn mkv(min: u8, max: u8, f: BuiltinFnVar) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltinVar>().expect("signal fnv");
    unsafe {
        (*o).base.type_ = &TV;
        (*o).min_args = min;
        (*o).max_args = max;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltinVar as *const ())
    }
}

fn signal_value(n: usize, args: &[Obj]) -> Obj {
    signal_call(args[0], n - 1, 0, &args[1..])
}

fn signal_on(self_in: Obj) -> Obj {
    virtpin::virtual_pin_write(self_in, 1);
    obj::CONST_NONE
}

fn signal_off(self_in: Obj) -> Obj {
    virtpin::virtual_pin_write(self_in, 0);
    obj::CONST_NONE
}

static mut SIGNAL_SLOTS: [*const (); 4] = [
    signal_make_new as *const (),
    signal_call as *const (),
    &raw const SIGNAL_PIN_P as *const (),
    core::ptr::null(),
];
static mut SIGNAL_TYPE: ObjType = ObjType {
    base: ObjBase {
        type_: core::ptr::null(),
    },
    flags: obj::TYPE_FLAG_NONE,
    name: 0,
    slot_index_make_new: 1,
    slot_index_print: 0,
    slot_index_call: 2,
    slot_index_unary_op: 0,
    slot_index_binary_op: 0,
    slot_index_attr: 0,
    slot_index_subscr: 0,
    slot_index_iter: 0,
    slot_index_buffer: 0,
    slot_index_protocol: 3,
    slot_index_parent: 0,
    slot_index_locals_dict: 4,
    slots: unsafe { SIGNAL_SLOTS.as_ptr() },
};

static INIT: std::sync::OnceLock<()> = std::sync::OnceLock::new();

fn init_signal_type() -> &'static ObjType {
    INIT.get_or_init(|| {
        let table = vec![
            MapElem {
                key: obj::new_qstr(qstr::from_str("value")),
                value: mkv(1, 2, signal_value),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("on")),
                value: mk1(signal_on),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("off")),
                value: mk1(signal_off),
            },
        ];
        let ptr = obj::malloc_helper(core::mem::size_of::<ObjDict>(), objdict::type_dict())
            as *mut ObjDict;
        unsafe {
            map::init_fixed_table(&mut (*ptr).map, table);
            SIGNAL_SLOTS[3] = obj::from_ptr(ptr as *const ObjDict as *const ()).0 as *const ();
            SIGNAL_TYPE.name = qstr::from_str("Signal");
        }
    });
    unsafe { &SIGNAL_TYPE }
}

/// `machine_signal_type`
pub fn signal_type() -> &'static ObjType {
    if !mpconfig::PY_MACHINE_SIGNAL {
        panic!("Signal disabled");
    }
    init_signal_type()
}
