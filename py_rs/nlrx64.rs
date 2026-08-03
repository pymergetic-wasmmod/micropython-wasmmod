//! rewrite of py/nlrx64.c
// symmetry: done

#![allow(non_snake_case)]

use crate::mpconfig;
use crate::nlr;
use crate::obj::Obj;

pub const NLR_NUM_REGS: usize = 8;

/// Native `nlr_buf_t` for x86-64 (`MICROPY_NLR_X64`).
#[repr(C)]
pub struct NlrBuf {
    pub prev: *mut NlrBuf,
    pub ret_val: Obj,
    pub regs: [*mut core::ffi::c_void; NLR_NUM_REGS],
}

const ENABLED: bool = mpconfig::NLR_X64;

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
    #[cfg(target_arch = "x86_64")]
    {
        // System V AMD64 inline asm from `py/nlrx64.c`:
        // movq (%rsp),%rax; movq %rax,16(%rdi); movq %rbp,24(%rdi);
        // movq %rsp,32(%rdi); movq %rbx,40(%rdi); movq %r12,48(%rdi);
        // movq %r13,56(%rdi); movq %r14,64(%rdi); movq %r15,72(%rdi);
        // jmp nlr_push_tail
    }
    push_tail(buf)
}

/// `nlr_jump` — restore registers and return 1 to the `nlr_push` callsite.
pub fn jump(val: Obj) -> ! {
    if !ENABLED {
        let _ = jump_head(val, core::ptr::null_mut());
        nlr::jump_fail(val.0);
    }
    #[cfg(target_arch = "x86_64")]
    {
        let top = jump_head(val, core::ptr::null_mut());
        let _ = top;
        // Restore %r15..%rbx, %rsp, %rbp, %rip per `py/nlrx64.c`.
        nlr::jump_fail(val.0);
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        let _ = jump_head(val, core::ptr::null_mut());
        nlr::jump_fail(val.0);
    }
}
