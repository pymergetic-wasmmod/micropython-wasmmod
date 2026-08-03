//! rewrite of extmod/machine_pwm.c + ports/nrf/modules/machine/soft_pwm.c
//! Host soft path: virtpin + softtimer bitbang (~1 ms phases; HW timer/LEDC N/A on unix).
// symmetry: done

use py_rs::argcheck::{self, Arg, ArgFlag, ArgVal};
use py_rs::map::{self, Map, MapElem};
use py_rs::malloc;
use py_rs::mpconfig;
use py_rs::mpprint::{self, Print, PrintKind, VaArg};
use py_rs::obj::{self, MakeNewFn, Obj, ObjBase, ObjType, TYPE_FLAG_BINDS_SELF, TYPE_FLAG_BUILTIN_FUN};
use py_rs::objdict::{self, ObjDict};
use py_rs::qstr;
use py_rs::raise::{self, MpRaise};

use shared_rs::runtime::softtimer::{self, SoftTimerEntry, MODE_PERIODIC};

use crate::hal_pin;
use crate::virtpin;

const SOFT_PWM_BASE_FREQ: u32 = 1_000_000;
const DUTY_FULL_SCALE: u32 = 1024;

const DUTY_NOT_SET: u8 = 0;
const DUTY: u8 = 1;
const DUTY_U16: u8 = 2;
const DUTY_NS: u8 = 3;

type BuiltinFn1 = fn(Obj) -> Obj;
type BuiltinFnVar = fn(usize, &[Obj]) -> Obj;
type BuiltinFnKw = fn(usize, &[Obj], &Map) -> Obj;

#[repr(C)]
struct MachinePwm {
    base: ObjBase,
    pin: Obj,
    defer_start: bool,
    duty_mode: u8,
    duty: u32,
    freq: u32,
    phase_high: bool,
    active: bool,
    timer: SoftTimerEntry,
}

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
#[repr(C)]
struct ObjFunBuiltinKw {
    base: ObjBase,
    min_args: u8,
    fun: BuiltinFnKw,
}

static mut F1: [*const (); 1] = [call1 as *const ()];
static mut FV: [*const (); 1] = [callv as *const ()];
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

fn callv(s: Obj, n: usize, k: usize, a: &[Obj]) -> Obj {
    let self_ = unsafe { &*(obj::as_ptr(s) as *const ObjFunBuiltinVar) };
    argcheck::check_num(n, k, self_.min_args as usize, self_.max_args as usize, false);
    (self_.fun)(n, a)
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
    let o = malloc::new_obj::<ObjFunBuiltin1>().expect("pwm fn1");
    unsafe {
        (*o).base.type_ = &T1;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltin1 as *const ())
    }
}

fn mkv(min: u8, max: u8, f: BuiltinFnVar) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltinVar>().expect("pwm fnv");
    unsafe {
        (*o).base.type_ = &TV;
        (*o).min_args = min;
        (*o).max_args = max;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltinVar as *const ())
    }
}

fn mk_kw(min: u8, f: BuiltinFnKw) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltinKw>().expect("pwm fnkw");
    unsafe {
        (*o).base.type_ = &TK;
        (*o).min_args = min;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltinKw as *const ())
    }
}

fn pwm_from_timer(entry: *mut SoftTimerEntry) -> *mut MachinePwm {
    unsafe {
        (entry as *mut u8).sub(core::mem::offset_of!(MachinePwm, timer)) as *mut MachinePwm
    }
}

fn period_ms(freq: u32) -> u32 {
    if freq == 0 {
        return 0;
    }
    ((1000 + freq / 2) / freq).max(1)
}

fn duty_width(duty_mode: u8, duty: u32, freq: u32) -> u32 {
    match duty_mode {
        DUTY => duty * DUTY_FULL_SCALE / 100,
        DUTY_U16 => ((duty as u64) * DUTY_FULL_SCALE as u64 / 65536) as u32,
        DUTY_NS => {
            ((duty as u64) * freq as u64 * DUTY_FULL_SCALE as u64 / 1_000_000_000) as u32
        }
        _ => 0,
    }
}

fn high_ms(freq: u32, duty_mode: u8, duty: u32) -> u32 {
    let period = period_ms(freq);
    let width = duty_width(duty_mode, duty, freq);
    if width >= DUTY_FULL_SCALE {
        return period;
    }
    if width == 0 {
        return 0;
    }
    ((period as u64) * width as u64 / DUTY_FULL_SCALE as u64).max(1) as u32
}

fn pwm_stop(self_: *mut MachinePwm) {
    unsafe {
        if (*self_).active {
            softtimer::remove(&mut (*self_).timer);
            (*self_).active = false;
        }
    }
}

fn pwm_timer_cb(entry: *mut SoftTimerEntry) {
    let self_ = pwm_from_timer(entry);
    unsafe {
        let pwm = &mut *self_;
        if pwm.phase_high {
            virtpin::virtual_pin_write(pwm.pin, 0);
            pwm.phase_high = false;
            let low = period_ms(pwm.freq).saturating_sub(high_ms(pwm.freq, pwm.duty_mode, pwm.duty));
            (*entry).delta_ms = low.max(1);
        } else {
            virtpin::virtual_pin_write(pwm.pin, 1);
            pwm.phase_high = true;
            (*entry).delta_ms = high_ms(pwm.freq, pwm.duty_mode, pwm.duty).max(1);
        }
    }
}

fn soft_pwm_start(self_: *mut MachinePwm) {
    unsafe {
        if (*self_).defer_start || (*self_).freq == 0 || (*self_).duty_mode == DUTY_NOT_SET {
            return;
        }
        if (*self_).freq > SOFT_PWM_BASE_FREQ / 256 {
            raise::raise(MpRaise::ValueError("frequency out of range"));
        }

        pwm_stop(self_);

        let hi = high_ms((*self_).freq, (*self_).duty_mode, (*self_).duty);
        let period = period_ms((*self_).freq);

        hal_pin::pin_output((*self_).pin);

        if hi == 0 {
            virtpin::virtual_pin_write((*self_).pin, 0);
            return;
        }
        if hi >= period {
            virtpin::virtual_pin_write((*self_).pin, 1);
            return;
        }

        (*self_).phase_high = true;
        virtpin::virtual_pin_write((*self_).pin, 1);
        (*self_).timer.mode = MODE_PERIODIC;
        (*self_).timer.delta_ms = hi.max(1);
        (*self_).timer.c_callback = Some(pwm_timer_cb);
        (*self_).timer.py_callback = obj::OBJ_NULL;
        (*self_).active = true;
        softtimer::insert(&mut (*self_).timer, hi.max(1));
    }
}

fn pwm_ptr(o: Obj) -> *mut MachinePwm {
    obj::as_ptr(o) as *mut MachinePwm
}

fn pwm_init_helper(self_: *mut MachinePwm, n_pos: usize, pos: &[Obj], kw: &Map) {
    let mut allowed = vec![
        Arg {
            qst: qstr::from_str("freq"),
            flags: ArgFlag::KwOnly as u16 | ArgFlag::Int as u16,
            defval: ArgVal::Int(-1),
        },
        Arg {
            qst: qstr::from_str("duty_u16"),
            flags: ArgFlag::KwOnly as u16 | ArgFlag::Int as u16,
            defval: ArgVal::Int(-1),
        },
        Arg {
            qst: qstr::from_str("duty_ns"),
            flags: ArgFlag::KwOnly as u16 | ArgFlag::Int as u16,
            defval: ArgVal::Int(-1),
        },
    ];
    if mpconfig::PY_MACHINE_PWM_DUTY {
        allowed.insert(
            1,
            Arg {
                qst: qstr::from_str("duty"),
                flags: ArgFlag::KwOnly as u16 | ArgFlag::Int as u16,
                defval: ArgVal::Int(-1),
            },
        );
    }
    let mut vals = vec![ArgVal::default(); allowed.len()];
    let mut kw_copy = kw.clone();
    argcheck::parse_all(n_pos, pos, &mut kw_copy, allowed.len(), &allowed, &mut vals);

    unsafe {
        (*self_).defer_start = true;
    }

    let mut idx = 0usize;
    if let ArgVal::Int(v) = vals[idx] {
        if v != -1 {
            pwm_freq_set(self_, v as i32);
        }
    }
    idx += 1;

    if mpconfig::PY_MACHINE_PWM_DUTY {
        if let ArgVal::Int(v) = vals[idx] {
            if v != -1 {
                pwm_duty_set(self_, v as i32);
            }
        }
        idx += 1;
    }

    if let ArgVal::Int(v) = vals[idx] {
        if v != -1 {
            pwm_duty_set_u16(self_, v as i32);
        }
    }
    idx += 1;

    if let ArgVal::Int(v) = vals[idx] {
        if v != -1 {
            pwm_duty_set_ns(self_, v as i32);
        }
    }

    unsafe {
        (*self_).defer_start = false;
    }
    soft_pwm_start(self_);
}

fn pwm_make_new(type_in: &ObjType, n_args: usize, n_kw: usize, args: &[Obj]) -> Obj {
    if n_args == 0 {
        raise::raise(MpRaise::TypeError("missing pin"));
    }
    let pin = hal_pin::get_pin_obj(args[0]);
    hal_pin::pin_output(pin);

    let o = malloc::new_obj::<MachinePwm>().expect("PWM");
    unsafe {
        (*o).base.type_ = type_in as *const ObjType;
        (*o).pin = pin;
        (*o).defer_start = false;
        (*o).duty_mode = DUTY_NOT_SET;
        (*o).duty = 0;
        (*o).freq = 0;
        (*o).phase_high = false;
        (*o).active = false;
        softtimer::static_init(&mut (*o).timer, MODE_PERIODIC, 1000, pwm_timer_cb);
    }
    let self_obj = obj::from_ptr(o as *const MachinePwm as *const ());

    if n_args > 1 || n_kw > 0 {
        let mut kw = Map::default();
        map::init(&mut kw, n_kw);
        for i in 0..n_kw {
            let key = args[n_args + i * 2];
            let val = args[n_args + i * 2 + 1];
            if let Some(slot) = map::lookup(&mut kw, key, map::LookupKind::AddIfNotFound) {
                slot.value = val;
            }
        }
        pwm_init_helper(o, n_args - 1, &args[1..n_args], &kw);
    }
    self_obj
}

fn pwm_print(print: &Print, self_in: Obj, _kind: PrintKind) {
    let self_ = unsafe { &*pwm_ptr(self_in) };
    let suffix = match self_.duty_mode {
        DUTY => "_duty",
        DUTY_U16 => "_u16",
        DUTY_NS => "_ns",
        _ => "",
    };
    mpprint::printf(
        print,
        "<PWM: freq=%dHz duty%s=%d>",
        [
            VaArg::Int(self_.freq as i32),
            VaArg::Str(suffix),
            VaArg::Int(self_.duty as i32),
        ],
    );
}

fn pwm_init(n: usize, args: &[Obj], kw: &Map) -> Obj {
    let self_ = pwm_ptr(args[0]);
    pwm_stop(self_);
    pwm_init_helper(self_, n - 1, &args[1..n], kw);
    obj::CONST_NONE
}

fn pwm_deinit(self_in: Obj) -> Obj {
    let self_ = pwm_ptr(self_in);
    pwm_stop(self_);
    unsafe {
        virtpin::virtual_pin_write((*self_).pin, 0);
    }
    obj::CONST_NONE
}

fn pwm_freq(n: usize, args: &[Obj]) -> Obj {
    let self_ = pwm_ptr(args[0]);
    if n == 1 {
        obj::new_small_int(unsafe { (*self_).freq as isize })
    } else {
        pwm_freq_set(self_, obj::get_int(args[1]) as i32);
        obj::CONST_NONE
    }
}

fn pwm_freq_set(self_: *mut MachinePwm, freq: i32) {
    if freq <= 0 {
        raise::raise(MpRaise::ValueError("frequency out of range"));
    }
    unsafe {
        (*self_).freq = freq as u32;
    }
    soft_pwm_start(self_);
}

fn pwm_duty(n: usize, args: &[Obj]) -> Obj {
    let self_ = unsafe { &*pwm_ptr(args[0]) };
    if n == 1 {
        let v = match self_.duty_mode {
            DUTY => self_.duty as isize,
            DUTY_U16 => (self_.duty as u64 * 100 / 65536) as isize,
            _ => -1,
        };
        obj::new_small_int(v)
    } else {
        pwm_duty_set(pwm_ptr(args[0]), obj::get_int(args[1]) as i32);
        obj::CONST_NONE
    }
}

fn pwm_duty_set(self_: *mut MachinePwm, duty: i32) {
    unsafe {
        (*self_).duty = duty as u32;
        (*self_).duty_mode = DUTY;
    }
    soft_pwm_start(self_);
}

fn pwm_duty_u16(n: usize, args: &[Obj]) -> Obj {
    let self_ = unsafe { &*pwm_ptr(args[0]) };
    if n == 1 {
        let v = match self_.duty_mode {
            DUTY_U16 => self_.duty as isize,
            DUTY => (self_.duty as u64 * 65536 / 100) as isize,
            _ => -1,
        };
        obj::new_small_int(v)
    } else {
        pwm_duty_set_u16(pwm_ptr(args[0]), obj::get_int(args[1]) as i32);
        obj::CONST_NONE
    }
}

fn pwm_duty_set_u16(self_: *mut MachinePwm, duty: i32) {
    if !(0..=65535).contains(&duty) {
        raise::raise(MpRaise::ValueError("duty_u16 out of range"));
    }
    unsafe {
        (*self_).duty = duty as u32;
        (*self_).duty_mode = DUTY_U16;
    }
    soft_pwm_start(self_);
}

fn pwm_duty_ns(n: usize, args: &[Obj]) -> Obj {
    let self_ = unsafe { &*pwm_ptr(args[0]) };
    if n == 1 {
        let v = if self_.duty_mode == DUTY_NS {
            self_.duty as isize
        } else {
            -1
        };
        obj::new_small_int(v)
    } else {
        pwm_duty_set_ns(pwm_ptr(args[0]), obj::get_int(args[1]) as i32);
        obj::CONST_NONE
    }
}

fn pwm_duty_set_ns(self_: *mut MachinePwm, duty: i32) {
    if duty < 0 {
        raise::raise(MpRaise::ValueError("duty_ns out of range"));
    }
    unsafe {
        (*self_).duty = duty as u32;
        (*self_).duty_mode = DUTY_NS;
    }
    soft_pwm_start(self_);
}

static mut PWM_SLOTS: [*const (); 3] = [
    pwm_make_new as MakeNewFn as *const (),
    pwm_print as *const (),
    core::ptr::null(),
];
static mut PWM_TYPE: ObjType = ObjType {
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
    slots: unsafe { PWM_SLOTS.as_ptr() },
};

static INIT: std::sync::OnceLock<()> = std::sync::OnceLock::new();

fn init_pwm_type() -> &'static ObjType {
    INIT.get_or_init(|| {
        let mut table = vec![
            MapElem {
                key: obj::new_qstr(qstr::from_str("init")),
                value: mk_kw(1, pwm_init),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("deinit")),
                value: mk1(pwm_deinit),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("freq")),
                value: mkv(1, 2, pwm_freq),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("duty_u16")),
                value: mkv(1, 2, pwm_duty_u16),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("duty_ns")),
                value: mkv(1, 2, pwm_duty_ns),
            },
        ];
        if mpconfig::PY_MACHINE_PWM_DUTY {
            table.insert(
                3,
                MapElem {
                    key: obj::new_qstr(qstr::from_str("duty")),
                    value: mkv(1, 2, pwm_duty),
                },
            );
        }
        let ptr =
            obj::malloc_helper(core::mem::size_of::<ObjDict>(), objdict::type_dict()) as *mut ObjDict;
        unsafe {
            map::init_fixed_table(&mut (*ptr).map, table);
            PWM_SLOTS[2] = obj::from_ptr(ptr as *const ObjDict as *const ()).0 as *const ();
            PWM_TYPE.name = qstr::from_str("PWM");
        }
    });
    unsafe { &PWM_TYPE }
}

/// `machine_pwm_type`
pub fn pwm_type() -> &'static ObjType {
    if !enabled() {
        panic!("PWM disabled");
    }
    init_pwm_type()
}

/// Board-specific `machine_pwm` helpers — enabled when `PY_MACHINE_PWM`.
pub fn enabled() -> bool {
    mpconfig::PY_MACHINE && mpconfig::PY_MACHINE_PWM
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn period_and_high_ms() {
        assert_eq!(period_ms(100), 10);
        assert_eq!(high_ms(100, DUTY_U16, 32768), 5);
        assert_eq!(high_ms(100, DUTY_U16, 0), 0);
        // 65535 maps to width 1023 (not 1024) under integer scaling
        assert_eq!(high_ms(100, DUTY_U16, 65535), 9);
    }

    #[test]
    fn duty_width_modes() {
        assert_eq!(duty_width(DUTY, 50, 100), 512);
        assert_eq!(duty_width(DUTY_U16, 65535, 100), 1023);
    }
}
