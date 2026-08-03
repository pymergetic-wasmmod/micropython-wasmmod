//! rewrite of shared/libc/abort_.c
// symmetry: done

use py_rs::raise::{self, MpRaise};

/// `abort_` — MicroPython abort hook.
pub fn abort_() -> ! {
    raise::raise(MpRaise::RuntimeError("abort() called"));
}
