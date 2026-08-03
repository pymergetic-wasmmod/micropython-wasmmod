//! rewrite of py/stackctrl.c + py/stackctrl.h
// symmetry: done

use crate::mpconfig;
use crate::mpstate;
use crate::obj::Uint;
use crate::raise::{self, MpRaise};

/// Initialise C stack top marker (`mp_stack_ctrl_init`).
pub fn stack_ctrl_init() {
    if mpconfig::PREVIEW_VERSION_2 {
        return;
    }
    let mut dummy: u32 = 0;
    let top = &mut dummy as *mut u32 as *mut u8;
    mpstate::with_thread(|st| {
        st.stack_top = top;
    });
}

/// Set C stack top (`mp_stack_set_top`).
pub fn stack_set_top(top: *mut u8) {
    if mpconfig::PREVIEW_VERSION_2 {
        return;
    }
    mpstate::with_thread(|st| {
        st.stack_top = top;
    });
}

/// Bytes of C stack used (descending stack) (`mp_stack_usage`).
pub fn stack_usage() -> Uint {
    if mpconfig::PREVIEW_VERSION_2 {
        return 0;
    }
    let mut dummy: u32 = 0;
    let cur = &mut dummy as *mut u32 as *mut u8;
    mpstate::with_thread(|st| {
        if st.stack_top.is_null() {
            0
        } else {
            (st.stack_top as usize).abs_diff(cur as usize)
        }
    })
}

/// Set stack limit (`mp_stack_set_limit`).
pub fn stack_set_limit(limit: Uint) {
    if mpconfig::PREVIEW_VERSION_2 || !mpconfig::STACK_CHECK {
        return;
    }
    mpstate::with_thread(|st| {
        st.stack_limit = limit;
    });
}

/// Check stack depth and raise on overflow (`mp_stack_check`).
pub fn stack_check() {
    if mpconfig::PREVIEW_VERSION_2 || !mpconfig::STACK_CHECK {
        return;
    }
    mpstate::with_thread(|st| {
        if stack_usage() >= st.stack_limit {
            raise::raise(MpRaise::RecursionDepth);
        }
    });
}

/// Macro wrapper (`MP_STACK_CHECK()`).
#[inline]
pub fn stack_check_macro() {
    stack_check();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stack_usage_is_nonzero_after_init() {
        stack_ctrl_init();
        fn inner() {
            assert!(stack_usage() > 0);
        }
        inner();
    }
}
