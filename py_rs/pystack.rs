//! rewrite of py/pystack.c + py/pystack.h
// symmetry: done

use crate::malloc;
use crate::mpconfig;
use crate::mpstate;
use crate::raise::{self, MpRaise};

const PYSTACK_ALIGN: usize = 8;
const PYSTACK_DEBUG: bool = false;

/// Initialise pystack region (`mp_pystack_init`).
pub fn pystack_init(start: *mut u8, end: *mut u8) {
    if !mpconfig::ENABLE_PYSTACK {
        return;
    }
    mpstate::with_thread(|st| {
        st.pystack_start = start;
        st.pystack_end = end;
        st.pystack_cur = start;
    });
}

/// Allocate from pystack (`mp_pystack_alloc`).
pub fn pystack_alloc(n_bytes: usize) -> *mut u8 {
    if !mpconfig::ENABLE_PYSTACK {
        return local_alloc(n_bytes);
    }
    let mut n_bytes = (n_bytes + PYSTACK_ALIGN - 1) & !(PYSTACK_ALIGN - 1);
    if PYSTACK_DEBUG {
        n_bytes += PYSTACK_ALIGN;
    }
    mpstate::with_thread(|st| {
        let cur = st.pystack_cur as usize;
        let end = st.pystack_end as usize;
        if cur + n_bytes > end {
            raise::raise(MpRaise::RuntimeError("pystack_space_exhausted"));
        }
        let ptr = st.pystack_cur;
        st.pystack_cur = unsafe { st.pystack_cur.add(n_bytes) };
        if PYSTACK_DEBUG {
            unsafe {
                *(st.pystack_cur.sub(PYSTACK_ALIGN) as *mut usize) = n_bytes;
            }
        }
        ptr
    })
}

/// Free back to marker (`mp_pystack_free`).
pub fn pystack_free(ptr: *mut u8) {
    if !mpconfig::ENABLE_PYSTACK {
        local_free(ptr);
        return;
    }
    mpstate::with_thread(|st| {
        debug_assert!(ptr >= st.pystack_start);
        debug_assert!(ptr <= st.pystack_cur);
        #[cfg(debug_assertions)]
        if PYSTACK_DEBUG {
            let n_bytes_to_free = st.pystack_cur as usize - ptr as usize;
            let mut n_bytes = unsafe { *(st.pystack_cur.sub(PYSTACK_ALIGN) as *const usize) };
            while n_bytes < n_bytes_to_free {
                n_bytes +=
                    unsafe { *(st.pystack_cur.sub(n_bytes + PYSTACK_ALIGN) as *const usize) };
            }
            assert_eq!(n_bytes, n_bytes_to_free);
        }
        st.pystack_cur = ptr;
    });
}

/// Realloc by free+alloc (`mp_pystack_realloc`).
pub fn pystack_realloc(ptr: *mut u8, n_bytes: usize) {
    pystack_free(ptr);
    let _ = pystack_alloc(n_bytes);
}

/// Current usage (`mp_pystack_usage`).
pub fn pystack_usage() -> usize {
    if !mpconfig::ENABLE_PYSTACK {
        return 0;
    }
    mpstate::with_thread(|st| st.pystack_cur as usize - st.pystack_start as usize)
}

/// Total limit (`mp_pystack_limit`).
pub fn pystack_limit() -> usize {
    if !mpconfig::ENABLE_PYSTACK {
        return 0;
    }
    mpstate::with_thread(|st| st.pystack_end as usize - st.pystack_start as usize)
}

/// Stack-local allocation when pystack disabled (`mp_local_alloc`).
pub fn local_alloc(n_bytes: usize) -> *mut u8 {
    if mpconfig::ENABLE_PYSTACK {
        return pystack_alloc(n_bytes);
    }
    malloc::new::<u8>(n_bytes)
        .unwrap_or_else(|| raise::raise(MpRaise::RuntimeError("alloc failed")))
}

/// Stack-local free when pystack disabled (`mp_local_free`).
pub fn local_free(_ptr: *mut u8) {}

/// Non-local allocation (`mp_nonlocal_alloc`).
pub fn nonlocal_alloc(n_bytes: usize) -> *mut u8 {
    if mpconfig::ENABLE_PYSTACK {
        pystack_alloc(n_bytes)
    } else {
        malloc::new::<u8>(n_bytes)
            .unwrap_or_else(|| raise::raise(MpRaise::RuntimeError("alloc failed")))
    }
}

/// Non-local realloc (`mp_nonlocal_realloc`).
pub fn nonlocal_realloc(ptr: *mut u8, old_n_bytes: usize, new_n_bytes: usize) -> *mut u8 {
    if mpconfig::ENABLE_PYSTACK {
        let _ = old_n_bytes;
        pystack_realloc(ptr, new_n_bytes);
        ptr
    } else {
        malloc::renew::<u8>(ptr, old_n_bytes, new_n_bytes)
            .unwrap_or_else(|| raise::raise(MpRaise::RuntimeError("realloc failed")))
    }
}

/// Non-local free (`mp_nonlocal_free`).
pub fn nonlocal_free(ptr: *mut u8, n_bytes: usize) {
    if mpconfig::ENABLE_PYSTACK {
        let _ = n_bytes;
        pystack_free(ptr);
    } else {
        malloc::del(ptr, n_bytes);
    }
}
