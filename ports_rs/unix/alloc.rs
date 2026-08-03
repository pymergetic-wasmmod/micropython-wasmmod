//! rewrite of ports/unix/alloc.c
// symmetry: done

use py_rs::gc;
use py_rs::mpconfig;
use std::ptr;
use std::sync::Mutex;

/// Linked list node tracking an mmap'd executable region.
struct MmapRegion {
    ptr: usize,
    len: usize,
    next: Option<Box<MmapRegion>>,
}

static MMAP_REGIONS: Mutex<Option<Box<MmapRegion>>> = Mutex::new(None);

/// `mp_unix_alloc_exec` — allocate RWX mmap region for native code.
pub fn unix_alloc_exec(min_size: usize, ptr: &mut *mut u8, size: &mut usize) {
    if !mpconfig::ENABLE_NATIVE_CODE {
        *ptr = ptr::null_mut();
        *size = 0;
        return;
    }
    *size = (min_size + 0xfff) & !0xfff;
    let p = unsafe {
        libc::mmap(
            ptr::null_mut(),
            *size,
            libc::PROT_READ | libc::PROT_WRITE | libc::PROT_EXEC,
            libc::MAP_PRIVATE | libc::MAP_ANONYMOUS,
            -1,
            0,
        )
    };
    if p == libc::MAP_FAILED {
        *ptr = ptr::null_mut();
        return;
    }
    *ptr = p as *mut u8;
    let node = Box::new(MmapRegion {
        ptr: *ptr as usize,
        len: min_size,
        next: MMAP_REGIONS.lock().unwrap().take(),
    });
    *MMAP_REGIONS.lock().unwrap() = Some(node);
}

/// `mp_unix_free_exec` — unmap and unlink executable region.
pub fn unix_free_exec(ptr: *mut u8, size: usize) {
    if ptr.is_null() {
        return;
    }
    unsafe {
        libc::munmap(ptr as *mut _, size);
    }
    let mut guard = MMAP_REGIONS.lock().unwrap();
    unlink_region(&mut *guard, ptr);
}

fn unlink_region(list: &mut Option<Box<MmapRegion>>, ptr: *mut u8) {
    let mut cur = list;
    while let Some(node) = cur {
        if node.ptr == ptr as usize {
            let rest = node.next.take();
            *cur = rest;
            return;
        }
        cur = &mut cur.as_mut().unwrap().next;
    }
}

/// Trace mmap region list heads during GC.
pub fn register_gc_roots() {
    let guard = MMAP_REGIONS.lock().unwrap();
    let mut node = guard.as_ref();
    while let Some(n) = node {
        let ptrs = [n.ptr as *mut u8];
        gc::collect_root(&ptrs);
        node = n.next.as_ref();
    }
}
