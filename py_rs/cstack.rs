//! rewrite of py/cstack.h + py/cstack.c
// symmetry: done

use crate::mpconfig;
use crate::mpstate;
use crate::obj::Uint;
use crate::raise::{self, MpRaise};

/// Initialise stack bounds using the current stack pointer as the top.
pub fn init_with_sp_here(stack_size: usize) {
    let stack_dummy = 0u8;
    let top = &stack_dummy as *const u8 as *mut u8;
    init_with_top(top, stack_size);
}

/// Initialise stack bounds with an explicit top pointer (`mp_cstack_init_with_top`).
pub fn init_with_top(top: *mut u8, stack_size: usize) {
    mpstate::with_thread(|t| {
        t.stack_top = top;
        if mpconfig::STACK_CHECK {
            assert!(stack_size > mpconfig::STACK_CHECK_MARGIN as usize);
            t.stack_limit = (stack_size - mpconfig::STACK_CHECK_MARGIN as usize) as Uint;
        }
    });
}

/// Bytes of stack used since `stack_top` (descending stacks).
pub fn usage() -> Uint {
    let stack_dummy = 0u8;
    let cur = &stack_dummy as *const u8 as usize;
    mpstate::with_thread(|t| {
        let top = t.stack_top as usize;
        if top == 0 {
            0
        } else {
            top.saturating_sub(cur) as Uint
        }
    })
}

/// Raise recursion depth if usage exceeds the configured limit.
pub fn check() {
    if !mpconfig::STACK_CHECK {
        return;
    }
    let (used, limit) = mpstate::with_thread(|t| (usage_with_top(t.stack_top), t.stack_limit));
    if used >= limit && limit != 0 {
        raise::raise(MpRaise::RecursionDepth);
    }
}

fn usage_with_top(top: *mut u8) -> Uint {
    let stack_dummy = 0u8;
    let cur = &stack_dummy as *const u8 as usize;
    let top = top as usize;
    if top == 0 {
        0
    } else {
        top.saturating_sub(cur) as Uint
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usage_after_init() {
        init_with_sp_here(64 * 1024);
        let u = usage();
        assert!(u < 64 * 1024);
    }
}
