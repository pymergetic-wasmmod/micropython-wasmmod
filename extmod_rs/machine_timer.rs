//! rewrite of extmod/machine_timer.c
// symmetry: done

use py_rs::argcheck::{self, Arg, ArgFlag, ArgVal};
use py_rs::map::{self, Map, MapElem};
use py_rs::malloc;
use py_rs::mpconfig;
use py_rs::mpprint::{self, Print, PrintKind, VaArg};
use py_rs::obj::{self, MakeNewFn, Obj, ObjBase, ObjType, TYPE_FLAG_BINDS_SELF, TYPE_FLAG_BUILTIN_FUN};
use py_rs::objdict::{self, ObjDict};
use py_rs::objfloat;
use py_rs::qstr;
use py_rs::raise::{self, MpRaise};

use shared_rs::runtime::softtimer::{
    self, SoftTimerEntry, FLAG_GC_ALLOCATED, FLAG_HARD_CALLBACK, FLAG_PY_CALLBACK, MODE_ONE_SHOT,
    MODE_PERIODIC,
};

type BuiltinFn1 = fn(Obj) -> Obj;
type BuiltinFnKw = fn(usize, &[Obj], &Map) -> Obj;

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
    let o = malloc::new_obj::<ObjFunBuiltin1>().expect("timer fn1");
    unsafe {
        (*o).base.type_ = &T1;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltin1 as *const ())
    }
}

fn mk_kw(min: u8, f: BuiltinFnKw) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltinKw>().expect("timer fnkw");
    unsafe {
        (*o).base.type_ = &TK;
        (*o).min_args = min;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltinKw as *const ())
    }
}

fn timer_ptr(o: Obj) -> *mut SoftTimerEntry {
    obj::as_ptr(o) as *mut SoftTimerEntry
}

fn compute_delta_ms(freq: Option<Obj>, period: u32, tick_hz: u32, current: u32) -> u32 {
    let mut delta_ms = u64::from(current);
    if let Some(freq_obj) = freq {
        if freq_obj != obj::CONST_NONE {
            delta_ms = if mpconfig::PY_BUILTINS_FLOAT {
                (1000.0 / objfloat::get_float(freq_obj)) as u64
            } else {
                1000 / obj::get_int(freq_obj) as u64
            };
        }
    } else if period != u32::MAX {
        delta_ms = u64::from(period) * 1000 / u64::from(tick_hz);
    }
    if delta_ms < 1 {
        1
    } else if delta_ms >= 0x4000_0000 {
        raise::raise(MpRaise::ValueError("period too large"));
    } else {
        delta_ms as u32
    }
}

fn timer_init_helper(self_: *mut SoftTimerEntry, n_pos: usize, pos: &[Obj], kw: &Map) -> Obj {
    let allowed = [
        Arg {
            qst: qstr::from_str("mode"),
            flags: ArgFlag::KwOnly as u16 | ArgFlag::Int as u16,
            defval: ArgVal::Int(MODE_PERIODIC as isize),
        },
        Arg {
            qst: qstr::from_str("callback"),
            flags: ArgFlag::KwOnly as u16 | ArgFlag::Obj as u16,
            defval: ArgVal::Obj(obj::OBJ_NULL),
        },
        Arg {
            qst: qstr::from_str("period"),
            flags: ArgFlag::KwOnly as u16 | ArgFlag::Int as u16,
            defval: ArgVal::Int(u32::MAX as isize),
        },
        Arg {
            qst: qstr::from_str("tick_hz"),
            flags: ArgFlag::KwOnly as u16 | ArgFlag::Int as u16,
            defval: ArgVal::Int(1000),
        },
        Arg {
            qst: qstr::from_str("freq"),
            flags: ArgFlag::KwOnly as u16 | ArgFlag::Obj as u16,
            defval: ArgVal::Obj(obj::CONST_NONE),
        },
        Arg {
            qst: qstr::from_str("hard"),
            flags: ArgFlag::KwOnly as u16 | ArgFlag::Bool as u16,
            defval: ArgVal::Bool(false),
        },
    ];
    let mut vals = [ArgVal::default(); 6];
    let mut kw_copy = kw.clone();
    argcheck::parse_all(n_pos, pos, &mut kw_copy, allowed.len(), &allowed, &mut vals);

    let mode = match vals[0] {
        ArgVal::Int(v) => v as u16,
        _ => MODE_PERIODIC,
    };
    let callback = match vals[1] {
        ArgVal::Obj(v) => v,
        _ => obj::OBJ_NULL,
    };
    let period = match vals[2] {
        ArgVal::Int(v) => v as u32,
        _ => u32::MAX,
    };
    let tick_hz = match vals[3] {
        ArgVal::Int(v) => v as u32,
        _ => 1000,
    };
    let freq = match vals[4] {
        ArgVal::Obj(v) => Some(v),
        _ => None,
    };
    let hard = matches!(vals[5], ArgVal::Bool(true));

    unsafe {
        (*self_).mode = mode;
        let freq_obj = freq.filter(|f| *f != obj::CONST_NONE);
        (*self_).delta_ms = compute_delta_ms(freq_obj, period, tick_hz, (*self_).delta_ms);
        if callback != obj::OBJ_NULL {
            (*self_).py_callback = callback;
        }
        if hard {
            (*self_).flags |= FLAG_HARD_CALLBACK;
        } else {
            (*self_).flags &= !FLAG_HARD_CALLBACK;
        }
        if (*self_).py_callback != obj::CONST_NONE {
            softtimer::insert(&mut *self_, (*self_).delta_ms);
        }
    }
    obj::CONST_NONE
}

fn timer_make_new(type_in: &ObjType, n_args: usize, n_kw: usize, args: &[Obj]) -> Obj {
    let mut pos = 0usize;
    let mut id: isize = -1;
    if n_args > 0 {
        id = obj::get_int(args[0]);
        pos = 1;
    }
    if id != -1 {
        raise::raise(MpRaise::ValueError("Timer doesn't exist"));
    }

    let o = malloc::new_obj::<SoftTimerEntry>().expect("Timer");
    unsafe {
        (*o).pairheap.base.type_ = type_in as *const ObjType;
        (*o).flags = FLAG_PY_CALLBACK | FLAG_GC_ALLOCATED;
        (*o).delta_ms = 1000;
        (*o).py_callback = obj::CONST_NONE;
        (*o).c_callback = None;
    }
    let self_obj = obj::from_ptr(o as *const SoftTimerEntry as *const ());

    let n_pos = n_args.saturating_sub(pos);
    if n_pos > 0 || n_kw > 0 {
        let mut kw = Map::default();
        map::init(&mut kw, n_kw);
        for i in 0..n_kw {
            let key = args[n_args + i * 2];
            let val = args[n_args + i * 2 + 1];
            if let Some(slot) = map::lookup(&mut kw, key, map::LookupKind::AddIfNotFound) {
                slot.value = val;
            }
        }
        timer_init_helper(o, n_pos, &args[pos..n_args], &kw);
    }
    self_obj
}

fn timer_print(print: &Print, self_in: Obj, _kind: PrintKind) {
    let self_ = unsafe { &*timer_ptr(self_in) };
    let mode = if self_.mode == MODE_ONE_SHOT {
        "ONE_SHOT"
    } else {
        "PERIODIC"
    };
    mpprint::printf(
        print,
        "Timer(mode=%s, period=%u)",
        [VaArg::Str(mode), VaArg::UInt(self_.delta_ms)],
    );
}

fn timer_init(n: usize, args: &[Obj], kw: &Map) -> Obj {
    let self_ = timer_ptr(args[0]);
    softtimer::remove(unsafe { &mut *self_ });
    timer_init_helper(self_, n - 1, &args[1..n], kw)
}

fn timer_deinit(self_in: Obj) -> Obj {
    softtimer::remove(unsafe { &mut *timer_ptr(self_in) });
    obj::CONST_NONE
}

static mut TIMER_SLOTS: [*const (); 3] = [
    timer_make_new as MakeNewFn as *const (),
    timer_print as *const (),
    core::ptr::null(),
];
static mut TIMER_TYPE: ObjType = ObjType {
    base: ObjBase {
        type_: core::ptr::null(),
    },
    flags: obj::TYPE_FLAG_NONE,
    name: 0,
    slot_index_make_new: 1,
    slot_index_print: 2,
    slot_index_call: 0,
    slot_index_unary_op: 0,
    slot_index_binary_op: 0,
    slot_index_attr: 0,
    slot_index_subscr: 0,
    slot_index_iter: 0,
    slot_index_buffer: 0,
    slot_index_protocol: 0,
    slot_index_parent: 0,
    slot_index_locals_dict: 3,
    slots: unsafe { TIMER_SLOTS.as_ptr() },
};

static INIT: std::sync::OnceLock<()> = std::sync::OnceLock::new();

fn init_timer_type() -> &'static ObjType {
    INIT.get_or_init(|| {
        let table = vec![
            MapElem {
                key: obj::new_qstr(qstr::from_str("init")),
                value: mk_kw(1, timer_init),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("deinit")),
                value: mk1(timer_deinit),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("ONE_SHOT")),
                value: obj::new_small_int(MODE_ONE_SHOT as isize),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("PERIODIC")),
                value: obj::new_small_int(MODE_PERIODIC as isize),
            },
        ];
        let ptr =
            obj::malloc_helper(core::mem::size_of::<ObjDict>(), objdict::type_dict()) as *mut ObjDict;
        unsafe {
            map::init_fixed_table(&mut (*ptr).map, table);
            TIMER_SLOTS[2] = obj::from_ptr(ptr as *const ObjDict as *const ()).0 as *const ();
            TIMER_TYPE.name = qstr::from_str("Timer");
        }
    });
    unsafe { &TIMER_TYPE }
}

/// `machine_timer_type`
pub fn timer_type() -> &'static ObjType {
    if !mpconfig::PY_MACHINE_TIMER {
        panic!("Timer disabled");
    }
    init_timer_type()
}

/// Start host soft-timer dispatch (std thread + poll helpers).
pub fn init_host_service() {
    if mpconfig::PY_MACHINE_TIMER {
        softtimer::init_host();
    }
}

/// Service due soft timers from port idle hooks.
pub fn host_service_poll() {
    if mpconfig::PY_MACHINE_TIMER {
        softtimer::poll();
    }
}

pub fn enabled() -> bool {
    mpconfig::PY_MACHINE && mpconfig::PY_MACHINE_TIMER
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delta_from_freq() {
        assert_eq!(compute_delta_ms(Some(obj::new_small_int(50)), u32::MAX, 1000, 1000), 20);
        assert_eq!(compute_delta_ms(None, 40, 1000, 1000), 40);
        assert_eq!(compute_delta_ms(None, u32::MAX, 1000, 500), 500);
    }
}
