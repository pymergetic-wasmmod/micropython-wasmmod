//! rewrite of ports/unix/mphalport.h
// symmetry: done

use py_rs::mpconfig;
use py_rs::mphal;
use py_rs::vstr::{self, Vstr};
use std::sync::Mutex;

pub const CHAR_CTRL_C: u8 = 3;

pub static COMPILE_ONLY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

static HISTORY: Mutex<Vec<String>> = Mutex::new(Vec::new());

pub fn load_history_lines(lines: Vec<String>) {
    *HISTORY.lock().unwrap() = lines;
}

pub fn take_history_lines() -> Vec<String> {
    std::mem::take(&mut *HISTORY.lock().unwrap())
}

/// `MICROPY_BEGIN_ATOMIC_SECTION` when threading is enabled.
pub fn begin_atomic_section() -> u32 {
    if mpconfig::PY_THREAD {
        super::mpthreadport::begin_atomic_section();
    }
    0xffff_ffff
}

/// `MICROPY_END_ATOMIC_SECTION`.
pub fn end_atomic_section(_state: u32) {
    if mpconfig::PY_THREAD {
        super::mpthreadport::end_atomic_section();
    }
}

/// `MICROPY_INTERNAL_WFE`.
pub fn internal_wfe(_timeout_ms: u32) {
    mphal::delay_us(500);
}

pub const HAL_HAS_STDIO_MODE_SWITCH: bool = true;

pub fn set_interrupt_char(c: i8) {
    super::unix_mphal::set_interrupt_char(c);
}

pub fn stdio_mode_raw() {
    super::unix_mphal::stdio_mode_raw();
}

pub fn stdio_mode_orig() {
    super::unix_mphal::stdio_mode_orig();
}

/// `mp_hal_readline` — prompt/read path for builtins.input().
pub fn hal_readline(vstr: &mut Vstr, prompt: &str) -> i32 {
    if mpconfig::PY_BUILTINS_INPUT {
        if let Some(line) = super::input::prompt(prompt) {
            vstr::add_str(vstr, &line);
            return 0;
        }
        return -1;
    }
    super::unix_mphal::readline(vstr, prompt)
}

pub fn hal_delay_us(us: u32) {
    mphal::delay_us(us as usize);
}

pub fn hal_ticks_cpu() -> u32 {
    0
}

pub fn hal_get_random(n: usize, buf: &mut [u8]) {
    super::unix_mphal::get_random(n, buf);
}
