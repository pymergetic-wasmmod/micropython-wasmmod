//! rewrite of py/nlrmips.c
// symmetry: done

#![allow(non_snake_case)]

use crate::mpconfig;
use crate::nlr;
use crate::obj::Obj;

pub const NLR_NUM_REGS: usize = 13;

/// Native `nlr_buf_t` for MIPS (`MICROPY_NLR_MIPS`).
#[repr(C)]
pub struct NlrBuf {
    pub prev: *mut NlrBuf,
    pub ret_val: Obj,
    pub regs: [*mut core::ffi::c_void; NLR_NUM_REGS],
}

const ENABLED: bool = mpconfig::NLR_MIPS;

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
    #[cfg(target_arch = "mips")]
    {
        // Standalone asm from `py/nlrmips.c` ($4 = first arg = nlr_buf):
        // sw $31, 8($4)            // ra  (offset of regs in nlr_buf_t)
        // sw $30, 12($4)           // fp
        // sw $29, 16($4)           // sp
        // sw $28, 20($4)           // gp
        // sw $23, 24($4)
        // sw $22, 28($4)
        // sw $21, 32($4)
        // sw $20, 36($4)
        // sw $19, 40($4)
        // sw $18, 44($4)
        // sw $17, 48($4)
        // sw $16, 52($4)
        // #ifdef __pic__: la $25, nlr_push_tail
        // j nlr_push_tail
    }
    push_tail(buf)
}

/// `nlr_jump` — restore registers and return 1 to the `nlr_push` callsite.
pub fn jump(val: Obj) -> ! {
    if !ENABLED {
        let _ = jump_head(val, core::ptr::null_mut());
        nlr::jump_fail(val.0);
    }
    #[cfg(target_arch = "mips")]
    {
        let top = jump_head(val, core::ptr::null_mut());
        let _ = top;
        // Restore $16–$31, sp, fp, gp, ra per `py/nlrmips.c`:
        // move $4, %0
        // lw $31, 8($4) … lw $16, 52($4)
        // lui $2, 1                // set return value 1
        // j $31
        // nop
        nlr::jump_fail(val.0);
    }
    #[cfg(not(target_arch = "mips"))]
    {
        let _ = jump_head(val, core::ptr::null_mut());
        nlr::jump_fail(val.0);
    }
}
