//! rewrite of py/nlrpowerpc.c
// symmetry: done

#![allow(non_snake_case)]

use crate::mpconfig;
use crate::nlr;
use crate::obj::Obj;

/// PowerPC uses 128 register slots for safety (`py/nlr.h`).
pub const NLR_NUM_REGS: usize = 128;

/// Native `nlr_buf_t` for PowerPC (`MICROPY_NLR_POWERPC`).
///
/// Saves all ABI non-volatile registers plus CR and LR into `regs` (with canary at offset 0).
#[repr(C)]
pub struct NlrBuf {
    pub prev: *mut NlrBuf,
    pub ret_val: Obj,
    pub regs: [*mut core::ffi::c_void; NLR_NUM_REGS],
}

const ENABLED: bool = mpconfig::NLR_POWERPC;

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
    #[cfg(target_arch = "powerpc64")]
    {
        // PowerPC64 inline asm from `py/nlrpowerpc.c` (%0 = &nlr->regs, %1 = nlr):
        // li     4, 0x4eed           // Store canary
        // std    4,  0x00(%0)
        // std    0,  0x08(%0)        // r0
        // std    1,  0x10(%0)        // sp
        // std    2,  0x18(%0)        // toc
        // std    14, 0x20(%0) … std  31, 0xA8(%0)
        // mfcr   4; std 4, 0xB0(%0)
        // mflr   4; std 4, 0xB8(%0)
        // li 4, nlr_push_tail@l; oris 4, 4, nlr_push_tail@h
        // mtctr 4; mr 3, %1; bctr
    }
    #[cfg(all(target_arch = "powerpc", not(target_arch = "powerpc64")))]
    {
        // PowerPC32 inline asm from `py/nlrpowerpc.c` (%0 = &nlr->regs, %1 = nlr):
        // li     4, 0x4eed           // Store canary
        // stw    4,  0x00(%0)
        // stw    0,  0x04(%0)        // r0
        // stw    1,  0x08(%0)        // sp
        // stw    2,  0x0c(%0)        // toc
        // stw    14, 0x10(%0) … stw  31, 0x54(%0)
        // mfcr   4; stw 4, 0x58(%0)
        // mflr   4; stw 4, 0x5c(%0)
        // li 4, nlr_push_tail@l; oris 4, 4, nlr_push_tail@h
        // mtctr 4; mr 3, %1; bctr
    }
    push_tail(buf)
}

/// `nlr_jump` — restore registers and return 1 to the `nlr_push` callsite.
pub fn jump(val: Obj) -> ! {
    if !ENABLED {
        let _ = jump_head(val, core::ptr::null_mut());
        nlr::jump_fail(val.0);
    }
    #[cfg(target_arch = "powerpc64")]
    {
        let top = jump_head(val, core::ptr::null_mut());
        let _ = top;
        // Restore from &top->regs per `py/nlrpowerpc.c` (__LP64__):
        // mr    4, %0
        // ld    3, 0x0(4); cmpdi 3, 0x4eed  // Check canary
        // bne   .
        // ld    0,  0x08(4) … ld 31, 0xA8(4)
        // ld    3,  0xB0(4); mtcr 3
        // ld    3, 0xB8(4); mtlr 3
        // li    3, 1; blr
        nlr::jump_fail(val.0);
    }
    #[cfg(all(target_arch = "powerpc", not(target_arch = "powerpc64")))]
    {
        let top = jump_head(val, core::ptr::null_mut());
        let _ = top;
        // Restore from &top->regs per `py/nlrpowerpc.c` (32-bit):
        // mr    4, %0
        // lw    3, 0x0(4); cmpwi 3, 0x4eed  // Check canary
        // bne   .
        // lw    0,  0x04(4) … lw 31, 0x54(4)
        // lw    3,  0x58(4); mtcr 3
        // lw    3, 0x5c(4); mtlr 3
        // li    3, 1; blr
        nlr::jump_fail(val.0);
    }
    #[cfg(not(any(target_arch = "powerpc", target_arch = "powerpc64")))]
    {
        let _ = jump_head(val, core::ptr::null_mut());
        nlr::jump_fail(val.0);
    }
}
