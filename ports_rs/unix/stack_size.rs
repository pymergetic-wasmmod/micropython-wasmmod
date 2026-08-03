//! rewrite of ports/unix/stack_size.h
// symmetry: done

use py_rs::mpconfig;

/// ARM (non-Thumb) architectures require more stack.
#[cfg(all(target_arch = "arm", not(target_arch = "thumb")))]
pub const STACK_MUL_ARM: usize = 2;

#[cfg(not(all(target_arch = "arm", not(target_arch = "thumb"))))]
pub const STACK_MUL_ARM: usize = 1;

/// Sanitizer builds consume significant stack.
pub const STACK_MUL_SANITIZERS: usize = if cfg!(feature = "sanitizer") { 4 } else { 1 };

#[cfg(target_env = "msvc")]
pub const STACK_MUL_WINDOWS: usize = 2;

#[cfg(not(target_env = "msvc"))]
pub const STACK_MUL_WINDOWS: usize = 1;

/// `UNIX_STACK_MULTIPLIER` — scale default thread / main stack sizes.
pub const STACK_MULTIPLIER: usize =
    (core::mem::size_of::<*const ()>() / 4) * STACK_MUL_ARM * STACK_MUL_SANITIZERS * STACK_MUL_WINDOWS;

/// Default MicroPython stack size for unix (bytes), scaled by [`STACK_MULTIPLIER`].
pub const DEFAULT_STACK_SIZE: usize = 32768 * STACK_MULTIPLIER;

/// Host smoke builds use a smaller cstack when threading is disabled.
pub fn cstack_size() -> usize {
    if mpconfig::PY_THREAD {
        DEFAULT_STACK_SIZE
    } else {
        16 * 1024
    }
}
