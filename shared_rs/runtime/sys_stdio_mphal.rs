//! rewrite of shared/runtime/sys_stdio_mphal.c
// symmetry: done

use py_rs::mphal;
use py_rs::mperrno::{self, EPERM, EINVAL};
use py_rs::obj::{self, Obj, ObjBase};
use py_rs::stream::{STREAM_CLOSE, STREAM_POLL, STREAM_POLL_RD, STREAM_POLL_WR};

const STDIO_FD_IN: i32 = 0;
const STDIO_FD_OUT: i32 = 1;
const STDIO_FD_ERR: i32 = 2;

#[repr(C)]
pub struct SysStdioObj {
    pub base: ObjBase,
    pub fd: i32,
}

pub static MP_SYS_STDIN_OBJ: SysStdioObj = SysStdioObj {
    base: ObjBase { type_: core::ptr::null() },
    fd: STDIO_FD_IN,
};
pub static MP_SYS_STDOUT_OBJ: SysStdioObj = SysStdioObj {
    base: ObjBase { type_: core::ptr::null() },
    fd: STDIO_FD_OUT,
};
pub static MP_SYS_STDERR_OBJ: SysStdioObj = SysStdioObj {
    base: ObjBase { type_: core::ptr::null() },
    fd: STDIO_FD_ERR,
};

/// `stdio_read`.
pub fn read(self_in: Obj, buf: &mut [u8]) -> Result<usize, i32> {
    let self_ = unsafe { &*(obj::as_ptr(self_in) as *const SysStdioObj) };
    if self_.fd != STDIO_FD_IN {
        return Err(EPERM);
    }
    for byte in buf.iter_mut() {
        let c = mphal::stdin_rx_chr();
        *byte = if c == b'\r' as i32 { b'\n' } else { c as u8 };
    }
    Ok(buf.len())
}

/// `stdio_write`.
pub fn write(self_in: Obj, buf: &[u8]) -> Result<usize, i32> {
    let self_ = unsafe { &*(obj::as_ptr(self_in) as *const SysStdioObj) };
    if self_.fd == STDIO_FD_OUT || self_.fd == STDIO_FD_ERR {
        super::stdout_helpers::stdout_tx_strn_cooked(
            unsafe { std::str::from_utf8_unchecked(buf) },
            buf.len(),
        );
        Ok(buf.len())
    } else {
        Err(EPERM)
    }
}

/// `stdio_ioctl`.
pub fn ioctl(_self_in: Obj, request: u32, arg: usize) -> Result<usize, i32> {
    if request == STREAM_POLL {
        Ok(mphal::stdio_poll(arg))
    } else if request == STREAM_CLOSE {
        Ok(0)
    } else {
        Err(EINVAL)
    }
}

pub fn stdin_obj() -> Obj {
    obj::from_ptr(&MP_SYS_STDIN_OBJ as *const SysStdioObj as *const ())
}

pub fn stdout_obj() -> Obj {
    obj::from_ptr(&MP_SYS_STDOUT_OBJ as *const SysStdioObj as *const ())
}

pub fn stderr_obj() -> Obj {
    obj::from_ptr(&MP_SYS_STDERR_OBJ as *const SysStdioObj as *const ())
}

pub fn poll_stdin() -> usize {
    mphal::stdio_poll(STREAM_POLL_RD as usize)
}

pub fn poll_stdout() -> usize {
    mphal::stdio_poll(STREAM_POLL_WR as usize)
}

pub fn mperrno_stream_error(code: i32) -> i32 {
    let _ = mperrno::EPERM;
    code
}
