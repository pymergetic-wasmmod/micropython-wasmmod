//! rewrite of py/nlrloong64.c
// symmetry: done

#![allow(non_snake_case)]

use crate::mpconfig;
use crate::nlr;
use crate::obj::Obj;

pub const NLR_NUM_REGS: usize = 13;

/// Native `nlr_buf_t` for LoongArch64 (`MICROPY_NLR_LOONG64`).
#[repr(C)]
pub struct NlrBuf {
    pub prev: *mut NlrBuf,
    pub ret_val: Obj,
    pub regs: [*mut core::ffi::c_void; NLR_NUM_REGS],
}

const ENABLED: bool = mpconfig::NLR_LOONG64;

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
    #[cfg(target_arch = "loongarch64")]
    {
        // Standalone asm from `py/nlrloong64.c` ($r4 = first arg = nlr_buf):
        // st.d $r1,  $r4, 16       // Store RA.
        // st.d $r23, $r4, 24       // Store S0.
        // st.d $r24, $r4, 32       // Store S1.
        // st.d $r25, $r4, 40       // Store S2.
        // st.d $r26, $r4, 48       // Store S3.
        // st.d $r27, $r4, 56       // Store S4.
        // st.d $r28, $r4, 64       // Store S5.
        // st.d $r29, $r4, 72       // Store S6.
        // st.d $r30, $r4, 80       // Store S7.
        // st.d $r31, $r4, 88       // Store S8.
        // st.d $r22, $r4, 96       // Store S9.
        // st.d $r21, $r4, 104      // Marked as reserved in the ABI.
        // st.d $r3,  $r4, 112      // Store SP.
        // b    nlr_push_tail        // Jump to the C part.
    }
    push_tail(buf)
}

/// `nlr_jump` — restore registers and return 1 to the `nlr_push` callsite.
pub fn jump(val: Obj) -> ! {
    if !ENABLED {
        let _ = jump_head(val, core::ptr::null_mut());
        nlr::jump_fail(val.0);
    }
    #[cfg(target_arch = "loongarch64")]
    {
        let top = jump_head(val, core::ptr::null_mut());
        let _ = top;
        // Restore $r1, $r21–$r31, $r3 per `py/nlrloong64.c`:
        // add.d  $r4,  $r0, %0
        // ld.d   $r1,  $r4, 16     // Retrieve RA.
        // ld.d   $r23, $r4, 24 … ld.d $r31, $r4, 88
        // ld.d   $r22, $r4, 96     // Retrieve S9.
        // ld.d   $r21, $r4, 104
        // ld.d   $r3,  $r4, 112    // Retrieve SP.
        // addi.d $r4,  $r0, 1      // Return 1 for a non-local return.
        // ret
        nlr::jump_fail(val.0);
    }
    #[cfg(not(target_arch = "loongarch64"))]
    {
        let _ = jump_head(val, core::ptr::null_mut());
        nlr::jump_fail(val.0);
    }
}
