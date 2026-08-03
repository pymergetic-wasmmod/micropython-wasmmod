//! `MICROPY_NLR_SETJMP` host backend.
//!
//! `py/nlrsetjmp.c` is only the architecture-specific jump primitive; the
//! buffer-chain management lives in `nlr.c`.  On Rust hosts this module is the
//! explicit `catch_unwind` backend exported by `nlr`.
// symmetry: done

use crate::nlr::{self, NlrBuf};

/// Execute a protected region, the safe Rust equivalent of
/// `nlr_push(buf) ... nlr_pop()` plus `setjmp`/`longjmp`.
pub fn setjmp<R>(buf: &mut NlrBuf, body: impl FnOnce() -> R) -> Result<R, usize> {
    nlr::protect(buf, body)
}

/// Equivalent to `longjmp(top->jmpbuf, 1)` after the common NLR jump head.
pub fn longjmp(value: usize) -> ! {
    nlr::jump(value)
}
