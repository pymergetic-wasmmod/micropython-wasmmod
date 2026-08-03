//! rewrite of ports/unix/modtime.c
// symmetry: done

use py_rs::obj::Obj;
use py_rs::objfloat;
use py_rs::objtuple;
use py_rs::raise::{self, MpRaise};
use py_rs::runtime::{self, HandlePendingBehaviour};

const CLOCK_DIV: f64 = 1000.0;

fn errno() -> i32 {
    unsafe { *libc::__errno_location() }
}

/// Unix `time.time()`.
pub fn time_get() -> Obj {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    if py_rs::mpconfig::PY_BUILTINS_FLOAT {
        objfloat::new_float(now.as_secs_f64())
    } else {
        py_rs::obj::new_small_int(now.as_secs() as isize)
    }
}

/// Deprecated `time.clock()`.
pub fn clock() -> Obj {
    static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    let elapsed = START.get_or_init(std::time::Instant::now).elapsed();
    let ticks = (elapsed.as_secs_f64() * 1000.0) as u64;
    if py_rs::mpconfig::PY_BUILTINS_FLOAT {
        objfloat::new_float((ticks as f64 / 1000.0) / CLOCK_DIV)
    } else {
        py_rs::obj::new_small_int(ticks as isize)
    }
}

/// `time.sleep` with EINTR retry.
pub fn sleep(seconds: f64) -> Obj {
    let whole = seconds.floor() as i64;
    let frac = ((seconds - whole as f64) * 1_000_000.0).round() as i64;
    let mut tv = libc::timeval {
        tv_sec: whole,
        tv_usec: frac,
    };
    loop {
        runtime::handle_pending(HandlePendingBehaviour::CallbacksAndClearExceptions);
        let res = unsafe {
            libc::select(
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                &mut tv,
            )
        };
        if res != -1 || errno() != libc::EINTR {
            if res == -1 {
                raise::raise(MpRaise::OSError(errno()));
            }
            break;
        }
    }
    py_rs::obj::CONST_NONE
}

fn break_down_time(t: libc::time_t, local: bool) -> Obj {
    let tm = unsafe {
        let tp = &t as *const libc::time_t;
        if local {
            *libc::localtime(tp)
        } else {
            *libc::gmtime(tp)
        }
    };
    let wday = if tm.tm_wday - 1 < 0 { 6 } else { tm.tm_wday - 1 };
    let items = [
        py_rs::obj::new_small_int((tm.tm_year + 1900) as isize),
        py_rs::obj::new_small_int((tm.tm_mon + 1) as isize),
        py_rs::obj::new_small_int(tm.tm_mday as isize),
        py_rs::obj::new_small_int(tm.tm_hour as isize),
        py_rs::obj::new_small_int(tm.tm_min as isize),
        py_rs::obj::new_small_int(tm.tm_sec as isize),
        py_rs::obj::new_small_int(wday as isize),
        py_rs::obj::new_small_int((tm.tm_yday + 1) as isize),
        py_rs::obj::new_small_int(tm.tm_isdst as isize),
    ];
    objtuple::new_tuple(items.len(), Some(&items))
}

pub fn gmtime(secs: Option<i64>) -> Obj {
    let t = secs.unwrap_or_else(current_unix_time);
    break_down_time(t as libc::time_t, false)
}

pub fn localtime(secs: Option<i64>) -> Obj {
    let t = secs.unwrap_or_else(current_unix_time);
    break_down_time(t as libc::time_t, true)
}

fn current_unix_time() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

/// `time.mktime` from 8/9-tuple.
pub fn mktime(tuple: Obj) -> Obj {
    let (len, items) = objtuple::tuple_get(tuple);
    if !(8..=9).contains(&len) {
        raise::raise(MpRaise::TypeError("mktime needs a tuple of length 8 or 9"));
    }
    let int_at = |i: usize| py_rs::obj::get_int(items[i]) as i32;
    let mut tm = libc::tm {
        tm_year: int_at(0) - 1900,
        tm_mon: int_at(1) - 1,
        tm_mday: int_at(2),
        tm_hour: int_at(3),
        tm_min: int_at(4),
        tm_sec: int_at(5),
        tm_isdst: if len == 9 { int_at(8) } else { -1 },
        ..unsafe { std::mem::zeroed() }
    };
    let ret = unsafe { libc::mktime(&mut tm) };
    if ret == -1 {
        raise::raise(MpRaise::OverflowError("invalid mktime usage"));
    }
    py_rs::obj::new_small_int(ret as isize)
}

pub const EXTRA_GLOBAL_NAMES: &[&str] = &["clock", "gmtime", "localtime", "mktime"];
