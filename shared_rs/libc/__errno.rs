//! rewrite of shared/libc/__errno.c
// symmetry: done

static mut EMBED_ERRNO: i32 = 0;

/// `__errno` / `__errno_location` — embedded errno for `&errno`.
#[cfg(target_os = "linux")]
pub fn errno_location() -> *mut i32 {
    unsafe { core::ptr::addr_of_mut!(EMBED_ERRNO) }
}

#[cfg(not(target_os = "linux"))]
pub fn errno() -> *mut i32 {
    unsafe { core::ptr::addr_of_mut!(EMBED_ERRNO) }
}
