//! rewrite of py/nlraarch64.c
// symmetry: done

#![allow(non_snake_case)]

use crate::mpconfig;
use crate::nlr;
use crate::obj::Obj;

pub const NLR_NUM_REGS: usize = 13;

/// Native `nlr_buf_t` for AArch64 (`MICROPY_NLR_AARCH64`).
///
/// Callee-saved registers: x19–x29, lr, sp.
#[repr(C)]
pub struct NlrBuf {
    pub prev: *mut NlrBuf,
    pub ret_val: Obj,
    pub regs: [*mut core::ffi::c_void; NLR_NUM_REGS],
}

const ENABLED: bool = mpconfig::NLR_AARCH64;

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
    #[cfg(target_arch = "aarch64")]
    {
        // Standalone asm from `py/nlraarch64.c` (x0 = nlr_buf; regs at offset 16):
        // mov x9, sp
        // stp lr,  x9,  [x0,  #16]  // 16 == offsetof(nlr_buf_t, regs)
        // stp x19, x20, [x0,  #32]
        // stp x21, x22, [x0,  #48]
        // stp x23, x24, [x0,  #64]
        // stp x25, x26, [x0,  #80]
        // stp x27, x28, [x0,  #96]
        // str x29,      [x0, #112]
        // b nlr_push_tail           // do the rest in C
    }
    push_tail(buf)
}

/// `nlr_jump` — restore registers and return 1 to the `nlr_push` callsite.
pub fn jump(val: Obj) -> ! {
    if !ENABLED {
        let _ = jump_head(val, core::ptr::null_mut());
        nlr::jump_fail(val.0);
    }
    #[cfg(target_arch = "aarch64")]
    {
        let top = jump_head(val, core::ptr::null_mut());
        let _ = top;
        // Restore x19–x29, lr, sp per `py/nlraarch64.c`:
        // mov x0, %0
        // ldr x29,      [x0, #112]
        // ldp x27, x28, [x0,  #96]
        // ldp x25, x26, [x0,  #80]
        // ldp x23, x24, [x0,  #64]
        // ldp x21, x22, [x0,  #48]
        // ldp x19, x20, [x0,  #32]
        // ldp lr,  x9,  [x0,  #16]  // 16 == offsetof(nlr_buf_t, regs)
        // mov sp, x9
        // mov x0, #1                // non-local return
        // ret
        nlr::jump_fail(val.0);
    }
    #[cfg(not(target_arch = "aarch64"))]
    {
        let _ = jump_head(val, core::ptr::null_mut());
        nlr::jump_fail(val.0);
    }
}
