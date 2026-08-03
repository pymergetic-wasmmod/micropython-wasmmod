//! rewrite of py/mphal.h (+ host portable HAL for unix-class ports)
// symmetry: done

use std::io::{self, Read, Write};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::obj::Uint;

/// Poll flags matching common MicroPython HAL usage.
pub const HAL_POLLIN: Uint = 0x0001;
pub const HAL_POLLOUT: Uint = 0x0004;
pub const HAL_POLLERR: Uint = 0x0008;
pub const HAL_POLLHUP: Uint = 0x0010;

/// Port stdio backend (`mp_hal_stdin_rx_chr` / `stdout_tx` / termios). Registered by unix/qemu.
pub struct StdioPort {
    pub stdin_rx_chr: fn() -> i32,
    pub stdout_tx_strn: fn(&[u8]) -> usize,
    pub stdio_mode_raw: fn(),
    pub stdio_mode_orig: fn(),
}

static STDIO_PORT: OnceLock<StdioPort> = OnceLock::new();

/// Install host/port stdio implementations (call once from the port `main`).
pub fn register_stdio_port(port: StdioPort) {
    let _ = STDIO_PORT.set(port);
}

fn stdio_port() -> Option<&'static StdioPort> {
    STDIO_PORT.get()
}

static START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
static CPU_TICKS: AtomicU64 = AtomicU64::new(0);

fn start() -> Instant {
    *START.get_or_init(Instant::now)
}

/// `MICROPY_BEGIN_ATOMIC_SECTION` — host no-op.
#[inline]
pub fn begin_atomic_section() -> u32 {
    0
}

/// `MICROPY_END_ATOMIC_SECTION` — host no-op.
#[inline]
pub fn end_atomic_section(_state: u32) {}

/// `mp_hal_stdio_poll` — host stdin/stdout readiness.
pub fn stdio_poll(poll_flags: Uint) -> Uint {
    let mut ret = 0;
    if poll_flags & HAL_POLLIN != 0 {
        // Best-effort: assume stdin may have data when requested.
        ret |= HAL_POLLIN;
    }
    if poll_flags & HAL_POLLOUT != 0 {
        ret |= HAL_POLLOUT;
    }
    ret
}

/// Switch stdin to raw mode for interactive readline (`mp_hal_stdio_mode_raw`).
pub fn stdio_mode_raw() {
    if let Some(p) = stdio_port() {
        (p.stdio_mode_raw)();
    }
}

/// Restore cooked terminal mode (`mp_hal_stdio_mode_orig`).
pub fn stdio_mode_orig() {
    if let Some(p) = stdio_port() {
        (p.stdio_mode_orig)();
    }
}

/// `mp_hal_stdin_rx_chr` — blocking read of one byte from stdin.
pub fn stdin_rx_chr() -> i32 {
    if let Some(p) = stdio_port() {
        return (p.stdin_rx_chr)();
    }
    let mut buf = [0u8; 1];
    loop {
        match io::stdin().read_exact(&mut buf) {
            Ok(()) => return i32::from(buf[0]),
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(_) => return -1,
        }
    }
}

/// `mp_hal_stdout_tx_str`.
pub fn stdout_tx_str(str: &str) {
    stdout_tx_strn(str, str.len());
}

/// `mp_hal_stdout_tx_strn`.
pub fn stdout_tx_strn(str: &str, len: usize) -> Uint {
    let end = len.min(str.len());
    let bytes = &str.as_bytes()[..end];
    if let Some(p) = stdio_port() {
        return (p.stdout_tx_strn)(bytes);
    }
    let mut out = io::stdout();
    let _ = out.write_all(bytes);
    // Prompts like `>>> ` have no newline; flush so interactive REPL is usable.
    let _ = out.flush();
    end
}

/// `mp_hal_stdout_tx_strn_cooked` — translate `\n` to `\r\n`.
pub fn stdout_tx_strn_cooked(str: &str, len: usize) {
    let end = len.min(str.len());
    if let Some(p) = stdio_port() {
        for &byte in &str.as_bytes()[..end] {
            if byte == b'\n' {
                (p.stdout_tx_strn)(b"\r\n");
            } else {
                (p.stdout_tx_strn)(&[byte]);
            }
        }
        return;
    }
    let mut out = io::stdout();
    for &byte in &str.as_bytes()[..end] {
        if byte == b'\n' {
            let _ = out.write_all(b"\r\n");
        } else {
            let _ = out.write_all(&[byte]);
        }
    }
    let _ = out.flush();
}

/// `MP_PLAT_PRINT_STRN` used by mpprint.
pub fn plat_print_strn(str: &str) {
    let _ = stdout_tx_strn(str, str.len());
}

/// `mp_hal_delay_ms`.
pub fn delay_ms(ms: Uint) {
    std::thread::sleep(Duration::from_millis(ms as u64));
}

/// `mp_hal_delay_us`.
pub fn delay_us(us: Uint) {
    std::thread::sleep(Duration::from_micros(us as u64));
}

/// `mp_hal_ticks_ms`.
pub fn ticks_ms() -> Uint {
    start().elapsed().as_millis() as Uint
}

/// `mp_hal_ticks_us`.
pub fn ticks_us() -> Uint {
    start().elapsed().as_micros() as Uint
}

/// `mp_hal_ticks_cpu` — monotonic host counter.
pub fn ticks_cpu() -> Uint {
    CPU_TICKS.fetch_add(1, Ordering::Relaxed) as Uint
}

/// `mp_hal_time_ns` — nanoseconds since the Unix epoch.
pub fn time_ns() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0)
}

/// `MICROPY_INTERNAL_WFE` — host no-op.
#[inline]
pub fn internal_wfe(_timeout_ms: Uint) {}

/// `MICROPY_INTERNAL_EVENT_HOOK` — host no-op.
#[inline]
pub fn internal_event_hook() {}

/// `mp_hal_set_interrupt_char` — enable/disable Ctrl-C style interruption.
pub fn set_interrupt_char(c: i32) {
    if !crate::mpconfig::KBD_EXCEPTION {
        return;
    }
    unsafe {
        let handler = if c == 3 {
            // CHAR_CTRL_C
            extern "C" fn sighandler(_: libc::c_int) {
                crate::scheduler::sched_keyboard_interrupt();
            }
            sighandler as *const () as usize
        } else {
            libc::SIG_DFL
        };
        libc::signal(libc::SIGINT, handler);
    }
}
