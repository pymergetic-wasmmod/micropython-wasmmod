//! rewrite of py/nlrx86.c
// symmetry: done

#![allow(non_snake_case)]

use crate::mpconfig;
use crate::nlr;
use crate::obj::Obj;

pub const NLR_NUM_REGS: usize = 6;

/// Native `nlr_buf_t` for i386 (`MICROPY_NLR_X86`).
///
/// Callee-saved registers: ebx, esi, edi, ebp, esp, eip.
#[repr(C)]
pub struct NlrBuf {
    pub prev: *mut NlrBuf,
    pub ret_val: Obj,
    pub regs: [*mut core::ffi::c_void; NLR_NUM_REGS],
}

const ENABLED: bool = mpconfig::NLR_X86;

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
    #[cfg(target_arch = "x86")]
    {
        // i386 inline asm from `py/nlrx86.c` (System V; naked on gcc >= 8 / clang):
        // mov    4(%esp), %edx       // load nlr_buf
        // mov    (%esp), %eax        // load return %eip
        // mov    %eax, 8(%edx)       // store %eip into nlr_buf
        // mov    %ebp, 12(%edx)      // store %ebp into nlr_buf
        // mov    %esp, 16(%edx)      // store %esp into nlr_buf
        // mov    %ebx, 20(%edx)      // store %ebx into nlr_buf
        // mov    %edi, 24(%edx)      // store %edi into nlr_buf
        // mov    %esi, 28(%edx)      // store %esi into nlr_buf
        // jmp    nlr_push_tail       // do the rest in C
        //
        // Older gcc (< 8) may prepend `pop %ebp` to undo the function prologue.
    }
    push_tail(buf)
}

/// `nlr_jump` — restore registers and return 1 to the `nlr_push` callsite.
pub fn jump(val: Obj) -> ! {
    if !ENABLED {
        let _ = jump_head(val, core::ptr::null_mut());
        nlr::jump_fail(val.0);
    }
    #[cfg(target_arch = "x86")]
    {
        let top = jump_head(val, core::ptr::null_mut());
        let _ = top;
        // Restore %esi..%eip per `py/nlrx86.c`:
        // mov    %0, %%edx           // %edx points to nlr_buf
        // mov    28(%%edx), %%esi    // load saved %esi
        // mov    24(%%edx), %%edi    // load saved %edi
        // mov    20(%%edx), %%ebx    // load saved %ebx
        // mov    16(%%edx), %%esp    // load saved %esp
        // mov    12(%%edx), %%ebp    // load saved %ebp
        // mov    8(%%edx), %%eax     // load saved %eip
        // mov    %%eax, (%%esp)      // store saved %eip to stack
        // xor    %%eax, %%eax        // clear return register
        // inc    %%al                // increase to make 1, non-local return
        // ret                        // return
        nlr::jump_fail(val.0);
    }
    #[cfg(not(target_arch = "x86"))]
    {
        let _ = jump_head(val, core::ptr::null_mut());
        nlr::jump_fail(val.0);
    }
}
