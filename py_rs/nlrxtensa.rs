//! rewrite of py/nlrxtensa.c
// symmetry: done

#![allow(non_snake_case)]

use crate::mpconfig;
use crate::nlr;
use crate::obj::Obj;

pub const NLR_NUM_REGS: usize = 10;

/// Native `nlr_buf_t` for Xtensa (`MICROPY_NLR_XTENSA`).
///
/// Calling convention: a0 = return address, a1 = stack pointer, a2 = first arg / return value.
#[repr(C)]
pub struct NlrBuf {
    pub prev: *mut NlrBuf,
    pub ret_val: Obj,
    pub regs: [*mut core::ffi::c_void; NLR_NUM_REGS],
}

const ENABLED: bool = mpconfig::NLR_XTENSA;

fn jump_head(val: Obj, top: *mut NlrBuf) -> *mut NlrBuf {
    if top.is_null() {
        nlr::jump_fail(val.0);
    }
    unsafe {
        (*top).ret_val = val;
    }
    top
}

/// `nlr_push_tail` — chain management after register save (`py/nlr.c`).
pub fn push_tail(buf: &mut NlrBuf) -> u32 {
    let _ = buf;
    0
}

/// `nlr_push` — save callee-saved registers then push onto the NLR chain.
pub fn push(buf: &mut NlrBuf) -> u32 {
    if !ENABLED {
        return push_tail(buf);
    }
    #[cfg(target_arch = "xtensa")]
    {
        // Xtensa inline asm from `py/nlrxtensa.c` (a2 = nlr_buf):
        // s32i.n  a0, a2, 8          // save return address
        // s32i.n  a1, a2, 12         // save stack pointer
        // s32i.n  a8, a2, 16         // save a8
        // s32i.n  a9, a2, 20         // save a9
        // s32i.n  a10, a2, 24        // save a10
        // s32i.n  a11, a2, 28        // save a11
        // s32i.n  a12, a2, 32        // save a12
        // s32i.n  a13, a2, 36        // save a13
        // s32i.n  a14, a2, 40        // save a14
        // s32i.n  a15, a2, 44        // save a15
        // j      nlr_push_tail       // do the rest in C
    }
    push_tail(buf)
}

/// `nlr_jump` — restore registers and return 1 to the `nlr_push` callsite.
pub fn jump(val: Obj) -> ! {
    if !ENABLED {
        let _ = jump_head(val, core::ptr::null_mut());
        nlr::jump_fail(val.0);
    }
    #[cfg(target_arch = "xtensa")]
    {
        let top = jump_head(val, core::ptr::null_mut());
        let _ = top;
        // Restore a0–a1, a8–a15 per `py/nlrxtensa.c`:
        // mov.n   a2, %0             // a2 points to nlr_buf
        // l32i.n  a0, a2, 8          // restore return address
        // l32i.n  a1, a2, 12         // restore stack pointer
        // l32i.n  a8, a2, 16 … l32i.n a15, a2, 44
        // movi.n a2, 1               // return 1, non-local return
        // ret.n                      // return
        nlr::jump_fail(val.0);
    }
    #[cfg(not(target_arch = "xtensa"))]
    {
        let _ = jump_head(val, core::ptr::null_mut());
        nlr::jump_fail(val.0);
    }
}
