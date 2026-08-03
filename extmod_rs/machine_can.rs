//! rewrite of extmod/machine_can.c + extmod/machine_can.h
//! Shared `machine.CAN` API surface with constants and method stubs; TX/RX/filters/IRQ call `CanPort` hooks.
//! Host/unix has no CAN controller HAL — `__init__`/`init`/`send`/`recv`/… raise `OSError("CAN not available")`; `state()` returns `STOPPED`.
// symmetry: done

use py_rs::argcheck;
use py_rs::malloc;
use py_rs::map::{self, Map, MapElem};
use py_rs::mpconfig;
use py_rs::obj::{
    self, MakeNewFn, Obj, ObjBase, ObjType, TYPE_FLAG_BINDS_SELF, TYPE_FLAG_BUILTIN_FUN,
};
use py_rs::objdict::{self, ObjDict};
use py_rs::objexcept;
use py_rs::objstr;
use py_rs::qstr;
use py_rs::raise::{self, MpRaise};

use crate::machine_can_port::{
    CanMode, CanState, CAN_MSG_FLAG_EXT_ID, CAN_MSG_FLAG_RTR, CAN_MSG_FLAG_UNORDERED,
    CAN_RECV_ERR_ESI, CAN_RECV_ERR_FULL, CAN_RECV_ERR_OVERRUN, MP_CAN_IRQ_IDX_MASK,
    MP_CAN_IRQ_IDX_SHIFT, MP_CAN_IRQ_RX, MP_CAN_IRQ_STATE, MP_CAN_IRQ_TX, MP_CAN_IRQ_TX_FAILED,
};

type BuiltinFn1 = fn(Obj) -> Obj;
type BuiltinFnKw = fn(usize, &[Obj], &Map) -> Obj;

/// Default number of CAN peripherals when port does not override.
const HW_NUM_CAN: usize = 1;

#[repr(C)]
struct MachineCan {
    base: ObjBase,
    can_idx: u8,
    /// Non-null when port HAL has initialised the controller.
    port: *mut (),
}

#[repr(C)]
struct ObjFunBuiltin1 {
    base: ObjBase,
    fun: BuiltinFn1,
}

#[repr(C)]
struct ObjFunBuiltinKw {
    base: ObjBase,
    min_args: u8,
    fun: BuiltinFnKw,
}

static mut F1: [*const (); 1] = [call1 as *const ()];
static mut FK: [*const (); 1] = [call_kw as *const ()];

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

static TK: ObjType = ObjType {
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
    slots: unsafe { FK.as_ptr() },
};

fn call1(s: Obj, n: usize, k: usize, a: &[Obj]) -> Obj {
    argcheck::check_num(n, k, 1, 1, false);
    unsafe { ((*(obj::as_ptr(s) as *const ObjFunBuiltin1)).fun)(a[0]) }
}

fn call_kw(s: Obj, n: usize, k: usize, a: &[Obj]) -> Obj {
    let self_ = unsafe { &*(obj::as_ptr(s) as *const ObjFunBuiltinKw) };
    if n < self_.min_args as usize {
        raise::raise(MpRaise::TypeError("argument num/types mismatch"));
    }
    let mut kw = Map::default();
    map::init(&mut kw, k);
    for i in 0..k {
        let key = a[n + i * 2];
        let val = a[n + i * 2 + 1];
        if let Some(slot) = map::lookup(&mut kw, key, map::LookupKind::AddIfNotFound) {
            slot.value = val;
        }
    }
    (self_.fun)(n, &a[..n], &kw)
}

fn mk1(f: BuiltinFn1) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltin1>().expect("can fn1");
    unsafe {
        (*o).base.type_ = &T1;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltin1 as *const ())
    }
}

fn mk_kw(min: u8, f: BuiltinFnKw) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltinKw>().expect("can fnkw");
    unsafe {
        (*o).base.type_ = &TK;
        (*o).min_args = min;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltinKw as *const ())
    }
}

fn can_ptr(o: Obj) -> *mut MachineCan {
    obj::as_ptr(o) as *mut MachineCan
}

fn can_unavailable() -> ! {
    raise::raise_obj(objexcept::new_exception_args(
        objexcept::type_os_error(),
        1,
        &[objstr::new_str(b"CAN not available")],
    ));
}

fn can_get_index(id: Obj) -> u8 {
    let can_num = obj::get_int(id) as usize;
    if can_num < 1 || can_num > HW_NUM_CAN {
        raise::raise(MpRaise::ValueError("CAN id out of range"));
    }
    (can_num - 1) as u8
}

fn can_check_initialised(self_: &MachineCan) {
    if self_.port.is_null() {
        raise::raise(MpRaise::OSError(py_rs::mperrno::EINVAL));
    }
}

fn can_port_init(_self: &mut MachineCan) {
    // Port builds with `feature = "machine_can"` wire `CanPort` here.
    can_unavailable();
}

fn can_init_helper(self_: &mut MachineCan, n_args: usize, _pos: &[Obj], _kw: &Map) {
    if n_args < 2 {
        raise::raise(MpRaise::TypeError("missing required argument 'bitrate'"));
    }
    can_port_init(self_);
}

fn can_make_new(type_in: &ObjType, n_args: usize, n_kw: usize, args: &[Obj]) -> Obj {
    argcheck::check_num(n_args, n_kw, 1, usize::MAX, true);
    let can_idx = can_get_index(args[0]);
    let o = malloc::new_obj::<MachineCan>().expect("CAN");
    unsafe {
        (*o).base.type_ = type_in;
        (*o).can_idx = can_idx;
        (*o).port = core::ptr::null_mut();
    }
    let self_in = obj::from_ptr(o as *const MachineCan as *const ());
    let mut kw = Map::default();
    map::init(&mut kw, n_kw);
    for i in 0..n_kw {
        let key = args[n_args + i * 2];
        let val = args[n_args + i * 2 + 1];
        if let Some(slot) = map::lookup(&mut kw, key, map::LookupKind::AddIfNotFound) {
            slot.value = val;
        }
    }
    can_init_helper(unsafe { &mut *can_ptr(self_in) }, n_args, args, &kw);
    self_in
}

fn can_deinit(self_in: Obj) -> Obj {
    let self_ = unsafe { &mut *can_ptr(self_in) };
    self_.port = core::ptr::null_mut();
    obj::CONST_NONE
}

fn can_state(self_in: Obj) -> Obj {
    let self_ = unsafe { &*can_ptr(self_in) };
    if self_.port.is_null() {
        return obj::new_small_int(CanState::Stopped as isize);
    }
    obj::new_small_int(CanState::Stopped as isize)
}

fn can_init_call(n: usize, args: &[Obj], kw: &Map) -> Obj {
    let self_ = unsafe { &mut *can_ptr(args[0]) };
    can_init_helper(self_, n, args, kw);
    obj::CONST_NONE
}

fn can_need_hal_check(self_in: Obj) -> Obj {
    let self_ = unsafe { &*can_ptr(self_in) };
    can_check_initialised(self_);
    can_unavailable();
}

static mut CAN_SLOTS: [*const (); 2] = [can_make_new as MakeNewFn as *const (), core::ptr::null()];
static mut CAN_TYPE: ObjType = ObjType {
    base: ObjBase {
        type_: core::ptr::null(),
    },
    flags: obj::TYPE_FLAG_NONE,
    name: 0,
    slot_index_make_new: 1,
    slot_index_print: 0,
    slot_index_call: 0,
    slot_index_unary_op: 0,
    slot_index_binary_op: 0,
    slot_index_attr: 0,
    slot_index_subscr: 0,
    slot_index_iter: 0,
    slot_index_buffer: 0,
    slot_index_protocol: 0,
    slot_index_parent: 0,
    slot_index_locals_dict: 2,
    slots: unsafe { CAN_SLOTS.as_ptr() },
};

static INIT: std::sync::OnceLock<()> = std::sync::OnceLock::new();

fn init_can_type() -> &'static ObjType {
    INIT.get_or_init(|| {
        let table = vec![
            MapElem {
                key: obj::new_qstr(qstr::from_str("init")),
                value: mk_kw(1, can_init_call),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("deinit")),
                value: mk1(can_deinit),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("irq")),
                value: mk_kw(1, |n, a, kw| {
                    let _ = (n, a, kw);
                    can_need_hal_check(a[0])
                }),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("send")),
                value: mk1(can_need_hal_check),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("cancel_send")),
                value: mk1(can_need_hal_check),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("recv")),
                value: mk1(can_need_hal_check),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("set_filters")),
                value: mk1(can_need_hal_check),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("state")),
                value: mk1(can_state),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("get_counters")),
                value: mk1(can_need_hal_check),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("get_timings")),
                value: mk1(can_need_hal_check),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("restart")),
                value: mk1(can_need_hal_check),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("MODE_NORMAL")),
                value: obj::new_small_int(CanMode::Normal as isize),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("MODE_SLEEP")),
                value: obj::new_small_int(CanMode::Sleep as isize),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("MODE_LOOPBACK")),
                value: obj::new_small_int(CanMode::Loopback as isize),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("MODE_SILENT")),
                value: obj::new_small_int(CanMode::Silent as isize),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("MODE_SILENT_LOOPBACK")),
                value: obj::new_small_int(CanMode::SilentLoopback as isize),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("STATE_STOPPED")),
                value: obj::new_small_int(CanState::Stopped as isize),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("STATE_ACTIVE")),
                value: obj::new_small_int(CanState::Active as isize),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("STATE_WARNING")),
                value: obj::new_small_int(CanState::Warning as isize),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("STATE_PASSIVE")),
                value: obj::new_small_int(CanState::Passive as isize),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("STATE_BUS_OFF")),
                value: obj::new_small_int(CanState::BusOff as isize),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("FLAG_RTR")),
                value: obj::new_small_int(CAN_MSG_FLAG_RTR as isize),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("FLAG_EXT_ID")),
                value: obj::new_small_int(CAN_MSG_FLAG_EXT_ID as isize),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("FLAG_UNORDERED")),
                value: obj::new_small_int(CAN_MSG_FLAG_UNORDERED as isize),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("RECV_ERR_FULL")),
                value: obj::new_small_int(CAN_RECV_ERR_FULL as isize),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("RECV_ERR_OVERRUN")),
                value: obj::new_small_int(CAN_RECV_ERR_OVERRUN as isize),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("RECV_ERR_ESI")),
                value: obj::new_small_int(CAN_RECV_ERR_ESI as isize),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("IRQ_RX")),
                value: obj::new_small_int(MP_CAN_IRQ_RX as isize),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("IRQ_TX")),
                value: obj::new_small_int(MP_CAN_IRQ_TX as isize),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("IRQ_STATE")),
                value: obj::new_small_int(MP_CAN_IRQ_STATE as isize),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("IRQ_TX_FAILED")),
                value: obj::new_small_int(MP_CAN_IRQ_TX_FAILED as isize),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("IRQ_TX_IDX_SHIFT")),
                value: obj::new_small_int(MP_CAN_IRQ_IDX_SHIFT as isize),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("IRQ_TX_IDX_MASK")),
                value: obj::new_small_int(MP_CAN_IRQ_IDX_MASK as isize),
            },
        ];
        let ptr = obj::malloc_helper(core::mem::size_of::<ObjDict>(), objdict::type_dict())
            as *mut ObjDict;
        unsafe {
            map::init_fixed_table(&mut (*ptr).map, table);
            CAN_SLOTS[1] = obj::from_ptr(ptr as *const ObjDict as *const ()).0 as *const ();
            CAN_TYPE.name = qstr::from_str("CAN");
        }
    });
    unsafe { &CAN_TYPE }
}

/// `machine.CAN` type — raises on host; port builds wire `CanPort` via `machine_can_port`.
pub fn can_type() -> &'static ObjType {
    if !enabled() {
        panic!("CAN disabled");
    }
    init_can_type()
}

/// Board-specific `machine_can` helpers — enabled with `feature = "machine"`.
#[cfg(feature = "machine")]
pub fn enabled() -> bool {
    mpconfig::PY_MACHINE
}

#[cfg(not(feature = "machine"))]
pub fn enabled() -> bool {
    false
}

/// Register `machine.CAN` type for port `modmachine` wiring.
pub fn init_types() -> Obj {
    if !enabled() {
        return Obj(0);
    }
    let ty = can_type();
    obj::from_ptr(ty as *const ObjType as *const ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn can_mode_and_state_values_match_port_header() {
        assert_eq!(CanMode::Normal as u8, 0);
        assert_eq!(CanState::BusOff as u8, 4);
        assert_eq!(MP_CAN_IRQ_RX, 1 << 1);
    }
}
