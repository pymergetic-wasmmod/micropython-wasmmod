//! rewrite of py/malloc.c + alloc helpers from py/misc.h
// symmetry: done

use std::mem::{align_of, size_of};

use crate::gc;

/// Allocate `count` elements of `T` (`m_new`).
pub fn new<T>(count: usize) -> Option<*mut T> {
    let size = count.checked_mul(size_of::<T>())?;
    let align = align_of::<T>().max(1);
    let ptr = gc::alloc(size, align)?;
    Some(ptr as *mut T)
}

/// Allocate one object (`m_new_obj`).
pub fn new_obj<T>() -> Option<*mut T> {
    new::<T>(1)
}

/// Free a GC allocation (`m_del`).
pub fn del<T>(ptr: *mut T, _count: usize) {
    gc::free(ptr.cast());
}

/// Reallocate (`m_renew`), preserving the existing allocation where possible.
pub fn renew<T: Copy>(ptr: *mut T, old_count: usize, new_count: usize) -> Option<*mut T> {
    let new_bytes = new_count.checked_mul(size_of::<T>())?;
    if ptr.is_null() {
        return gc::alloc(new_bytes, align_of::<T>().max(1)).map(|ptr| ptr.cast());
    }
    gc::realloc(ptr.cast(), new_bytes, true).map(|ptr| ptr.cast())
}

/// Free object allocation (`m_del_obj`).
pub fn del_obj<T>(ptr: *mut T) {
    gc::free(ptr.cast());
}

/// Grow or shrink a GC block in place when possible (`m_renew_maybe`).
pub fn renew_maybe<T>(ptr: *mut T, old_bytes: usize, new_bytes: usize, allow_move: bool) -> Option<*mut T> {
    if new_bytes <= old_bytes {
        return Some(ptr);
    }
    gc::realloc(ptr.cast(), new_bytes, allow_move).map(|p| p.cast())
}
