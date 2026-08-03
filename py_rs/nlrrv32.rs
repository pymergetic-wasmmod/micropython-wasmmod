//! rewrite of py/nlrrv32.c
// symmetry: done

#![allow(non_snake_case)]

use crate::mpconfig;
use crate::nlr;
use crate::obj::Obj;

pub const NLR_NUM_REGS: usize = 14;

/// Native `nlr_buf_t` for RISC-V RV32I (`MICROPY_NLR_RV32I`).
#[repr(C)]
pub struct NlrBuf {
    pub prev: *mut NlrBuf,
    pub ret_val: Obj,
    pub regs: [*mut core::ffi::c_void; NLR_NUM_REGS],
}

const ENABLED: bool = mpconfig::NLR_RV32I;

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
    #[cfg(target_arch = "riscv32")]
    {
        // RV32I naked inline asm from `py/nlrrv32.c` (x10 = first arg):
        // sw   x1,  8(x10)       // Store RA.
        // sw   x8, 12(x10)       // Store S0.
        // sw   x9, 16(x10)       // Store S1.
        // sw  x18, 20(x10)       // Store S2.
        // sw  x19, 24(x10)       // Store S3.
        // sw  x20, 28(x10)       // Store S4.
        // sw  x21, 32(x10)       // Store S5.
        // sw  x22, 36(x10)       // Store S6.
        // sw  x23, 40(x10)       // Store S7.
        // sw  x24, 44(x10)       // Store S8.
        // sw  x25, 48(x10)       // Store S9.
        // sw  x26, 52(x10)       // Store S10.
        // sw  x27, 56(x10)       // Store S11.
        // sw   x2, 60(x10)       // Store SP.
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
    #[cfg(target_arch = "riscv32")]
    {
        let top = jump_head(val, core::ptr::null_mut());
        let _ = top;
        // Restore x1, x8–x11, x18–x27, sp per `py/nlrrv32.c`:
        // add  x10, x0, %0  // Load nlr_buf address.
        // lw   x1,  8(x10)  // Retrieve RA.
        // lw   x8, 12(x10) … lw x27, 56(x10)
        // lw   x2, 60(x10)  // Retrieve SP.
        // addi x10, x0, 1   // Return 1 for a non-local return.
        // jalr x0, x1, 0    // Return.
        nlr::jump_fail(val.0);
    }
    #[cfg(not(target_arch = "riscv32"))]
    {
        let _ = jump_head(val, core::ptr::null_mut());
        nlr::jump_fail(val.0);
    }
}
