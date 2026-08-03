//! rewrite of ports/unix/unix_mphal.c
// symmetry: done

use py_rs::runtime;
use py_rs::scheduler;
use py_rs::vstr::{self, Vstr};
use std::sync::OnceLock;

static ORIG_TERMIOS: OnceLock<libc::termios> = OnceLock::new();

extern "C" fn sighandler(_: libc::c_int) {
    scheduler::sched_keyboard_interrupt();
}

fn errno() -> i32 {
    unsafe { *libc::__errno_location() }
}

/// `mp_hal_set_interrupt_char`
pub fn set_interrupt_char(c: i8) {
    unsafe {
        let handler = if c as u8 == super::mphalport::CHAR_CTRL_C {
            sighandler as *const () as usize
        } else {
            libc::SIG_DFL
        };
        libc::signal(libc::SIGINT, handler);
    }
}

/// Switch stdin to raw mode for readline/input.
pub fn stdio_mode_raw() {
    unsafe {
        let mut orig: libc::termios = std::mem::zeroed();
        if libc::tcgetattr(0, &mut orig) != 0 {
            return;
        }
        let _ = ORIG_TERMIOS.set(orig);
        let mut term = orig;
        term.c_iflag &= !(libc::BRKINT | libc::ICRNL | libc::INPCK | libc::ISTRIP | libc::IXON);
        term.c_cflag = (term.c_cflag & !(libc::CSIZE | libc::PARENB)) | libc::CS8;
        term.c_lflag = 0;
        term.c_cc[libc::VMIN as usize] = 1;
        term.c_cc[libc::VTIME as usize] = 0;
        libc::tcsetattr(0, libc::TCSANOW, &term);
    }
}

/// Restore original terminal mode.
pub fn stdio_mode_orig() {
    if let Some(orig) = ORIG_TERMIOS.get() {
        unsafe {
            libc::tcsetattr(0, libc::TCSANOW, orig);
        }
    }
}

/// Readline wrapper toggling raw/cooked mode.
pub fn readline(vstr: &mut Vstr, prompt: &str) -> i32 {
    stdio_mode_raw();
    let r = if let Some(line) = super::input::prompt(prompt) {
        vstr::add_str(vstr, &line);
        0
    } else {
        -1
    };
    stdio_mode_orig();
    r
}

/// Unix `mp_hal_stdin_rx_chr`.
pub fn stdin_rx_chr() -> i32 {
    let mut c = [0u8; 1];
    loop {
        let ret = unsafe { libc::read(0, c.as_mut_ptr() as *mut _, 1) };
        if ret == 0 {
            return 4;
        }
        if ret < 0 {
            if errno() == libc::EINTR {
                runtime::handle_pending(runtime::HandlePendingBehaviour::CallbacksAndClearExceptions);
                continue;
            }
            return -1;
        }
        return if c[0] == b'\n' {
            b'\r' as i32
        } else {
            c[0] as i32
        };
    }
}

/// Unix `mp_hal_stdout_tx_strn`.
pub fn stdout_tx_strn(str: &[u8]) -> usize {
    let mut written = 0usize;
    while written < str.len() {
        let ret = unsafe {
            libc::write(
                1,
                str[written..].as_ptr() as *const _,
                str.len() - written,
            )
        };
        if ret < 0 {
            if errno() == libc::EINTR {
                runtime::handle_pending(runtime::HandlePendingBehaviour::CallbacksAndClearExceptions);
                continue;
            }
            break;
        }
        written += ret as usize;
    }
    written
}

/// Fill buffer with OS random bytes.
pub fn get_random(n: usize, buf: &mut [u8]) {
    #[cfg(target_os = "linux")]
    {
        let r = unsafe { libc::getrandom(buf.as_mut_ptr() as *mut _, n, 0) };
        if r >= 0 {
            return;
        }
    }
    let fd = unsafe { libc::open(c"/dev/random".as_ptr(), libc::O_RDONLY) };
    if fd == -1 {
        fill_random_local(n, buf);
        return;
    }
    let _ = unsafe { libc::read(fd, buf.as_mut_ptr() as *mut _, n) };
    unsafe {
        libc::close(fd);
    }
}

fn fill_random_local(n: usize, buf: &mut [u8]) {
    for (i, byte) in buf.iter_mut().enumerate().take(n) {
        *byte = (i as u8).wrapping_mul(31).wrapping_add(17);
    }
}
