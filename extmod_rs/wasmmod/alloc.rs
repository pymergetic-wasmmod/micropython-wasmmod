//! rewrite of extmod/wasmmod/alloc.h
// symmetry: done

use std::alloc::{self, Layout};

/// `MICROPY_WASM_MALLOC` — host std allocator (override via port macros in C).
pub fn wasm_malloc(n: usize) -> *mut u8 {
    if n == 0 {
        return std::ptr::null_mut();
    }
    let layout =
        Layout::from_size_align(n, 1).unwrap_or_else(|_| Layout::from_size_align(1, 1).unwrap());
    unsafe { alloc::alloc(layout) }
}

/// `MICROPY_WASM_FREE`
pub unsafe fn wasm_free(p: *mut u8, n: usize) {
    if p.is_null() || n == 0 {
        return;
    }
    if let Ok(layout) = Layout::from_size_align(n, 1) {
        alloc::dealloc(p, layout);
    }
}

/// `MICROPY_WASM_REALLOC`
pub unsafe fn wasm_realloc(p: *mut u8, old_n: usize, new_n: usize) -> *mut u8 {
    if new_n == 0 {
        wasm_free(p, old_n);
        return std::ptr::null_mut();
    }
    let new_layout = Layout::from_size_align(new_n, 1)
        .unwrap_or_else(|_| Layout::from_size_align(1, 1).unwrap());
    if p.is_null() {
        return alloc::alloc(new_layout);
    }
    let old_layout = Layout::from_size_align(old_n.max(1), 1).unwrap();
    alloc::realloc(p, old_layout, new_n)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn malloc_free_roundtrip() {
        let p = wasm_malloc(16);
        assert!(!p.is_null());
        unsafe { wasm_free(p, 16) };
    }
}
