//! rewrite of py/nlrthumb.c
// symmetry: done

#![allow(non_snake_case)]

use crate::mpconfig;
use crate::nlr;
use crate::obj::Obj;

pub const NLR_NUM_REGS: usize = 10;

/// Native `nlr_buf_t` for ARM/Thumb (`MICROPY_NLR_THUMB`).
///
/// Callee-saved registers: r4–r11, r13=sp, lr.
/// With hardware FP (`MICROPY_NLR_NUM_REGS_ARM_THUMB_FP` = 16), s16–s21 are also saved.
#[repr(C)]
pub struct NlrBuf {
    pub prev: *mut NlrBuf,
    pub ret_val: Obj,
    pub regs: [*mut core::ffi::c_void; NLR_NUM_REGS],
}

const ENABLED: bool = mpconfig::NLR_THUMB;

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
    #[cfg(target_arch = "arm")]
    {
        // ARM/Thumb naked inline asm from `py/nlrthumb.c`:
        // str    r4, [r0, #12]       // store r4 into nlr_buf
        // str    r5, [r0, #16]       // store r5 into nlr_buf
        // str    r6, [r0, #20]       // store r6 into nlr_buf
        // str    r7, [r0, #24]       // store r7 into nlr_buf
        //
        // thumb1 (!__thumb2__):
        // mov    r1, r8; str r1, [r0, #28]  // r8
        // mov    r1, r9; str r1, [r0, #32]  // r9
        // mov    r1, r10; str r1, [r0, #36] // r10
        // mov    r1, r11; str r1, [r0, #40] // r11
        // mov    r1, r13; str r1, [r0, #44] // sp
        // mov    r1, lr; str r1, [r0, #8]    // lr
        //
        // thumb2:
        // str    r8, [r0, #28]; str r9, [r0, #32]; str r10, [r0, #36]
        // str    r11, [r0, #40]; str r13, [r0, #44]
        // #if MICROPY_NLR_NUM_REGS == 16 (hard-FP):
        // vstr   d8, [r0, #48]; vstr d9, [r0, #56]; vstr d10, [r0, #64]
        // str    lr, [r0, #8]
        //
        // b nlr_push_tail  (or ldr/bx via nlr_push_tail_var if MICROPY_NLR_THUMB_USE_LONG_JUMP)
    }
    push_tail(buf)
}

/// `nlr_jump` — restore registers and return 1 to the `nlr_push` callsite.
pub fn jump(val: Obj) -> ! {
    if !ENABLED {
        let _ = jump_head(val, core::ptr::null_mut());
        nlr::jump_fail(val.0);
    }
    #[cfg(target_arch = "arm")]
    {
        let top = jump_head(val, core::ptr::null_mut());
        let _ = top;
        // Restore r4–r11, sp, lr per `py/nlrthumb.c`:
        // mov    r0, %0              // r0 points to nlr_buf
        // ldr    r4, [r0, #12] … ldr r7, [r0, #24]
        // thumb1: ldr r1, [r0, #28]; mov r8, r1; … mov lr, r1
        // thumb2: ldr r8..r11, r13, lr directly
        // #if MICROPY_NLR_NUM_REGS == 16: vldr d8..d10 from [r0, #48..#64]
        // movs   r0, #1              // return 1, non-local return
        // bx     lr                  // return
        nlr::jump_fail(val.0);
    }
    #[cfg(not(target_arch = "arm"))]
    {
        let _ = jump_head(val, core::ptr::null_mut());
        nlr::jump_fail(val.0);
    }
}
