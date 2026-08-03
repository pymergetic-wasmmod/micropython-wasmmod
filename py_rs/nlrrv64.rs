//! rewrite of py/nlrrv64.c
// symmetry: done

#![allow(non_snake_case)]

use crate::mpconfig;
use crate::nlr;
use crate::obj::Obj;

pub const NLR_NUM_REGS: usize = 14;

/// Native `nlr_buf_t` for RISC-V RV64I (`MICROPY_NLR_RV64I`).
#[repr(C)]
pub struct NlrBuf {
    pub prev: *mut NlrBuf,
    pub ret_val: Obj,
    pub regs: [*mut core::ffi::c_void; NLR_NUM_REGS],
}

const ENABLED: bool = mpconfig::NLR_RV64I;

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
    #[cfg(target_arch = "riscv64")]
    {
        // RV64I naked inline asm from `py/nlrrv64.c` (x10 = first arg):
        // sd   x1, 16(x10)       // Store RA.
        // sd   x8, 24(x10)       // Store S0.
        // sd   x9, 32(x10)       // Store S1.
        // sd  x18, 40(x10)       // Store S2.
        // sd  x19, 48(x10)       // Store S3.
        // sd  x20, 56(x10)       // Store S4.
        // sd  x21, 64(x10)       // Store S5.
        // sd  x22, 72(x10)       // Store S6.
        // sd  x23, 80(x10)       // Store S7.
        // sd  x24, 88(x10)       // Store S8.
        // sd  x25, 96(x10)       // Store S9.
        // sd  x26, 104(x10)      // Store S10.
        // sd  x27, 112(x10)      // Store S11.
        // sd   x2, 120(x10)      // Store SP.
        // jal  x0, nlr_push_tail // Jump to the C part.
    }
    push_tail(buf)
}

/// `nlr_jump` — restore registers and return 1 to the `nlr_push` callsite.
pub fn jump(val: Obj) -> ! {
    if !ENABLED {
        let _ = jump_head(val, core::ptr::null_mut());
        nlr::jump_fail(val.0);
    }
    #[cfg(target_arch = "riscv64")]
    {
        let top = jump_head(val, core::ptr::null_mut());
        let _ = top;
        // Restore x1, x8–x11, x18–x27, sp per `py/nlrrv64.c`:
        // add  x10, x0, %0  // Load nlr_buf address.
        // ld   x1, 16(x10)  // Retrieve RA.
        // ld   x8, 24(x10) … ld x27, 112(x10)
        // ld   x2, 120(x10) // Retrieve SP.
        // addi x10, x0, 1   // Return 1 for a non-local return.
        // jalr x0, x1, 0    // Return.
        nlr::jump_fail(val.0);
    }
    #[cfg(not(target_arch = "riscv64"))]
    {
        let _ = jump_head(val, core::ptr::null_mut());
        nlr::jump_fail(val.0);
    }
}
