//! rewrite of ports/unix/gccollect.c
// symmetry: done

use py_rs::gc;
use py_rs::mpconfig;
use py_rs::mpstate;

/// Scan register/stack roots (host setjmp-style path).
#[inline(never)]
fn gc_collect_regs_and_stack() {
    let mut regs = [0u8; 200];
    let start = regs.as_ptr() as usize;
    mpstate::with_thread(|t| {
        let stack_top = t.stack_top as usize;
        if stack_top > start {
            let count = (stack_top - start) / core::mem::size_of::<usize>();
            let mut ptrs = Vec::with_capacity(count);
            for i in 0..count {
                ptrs.push(unsafe { (start as *mut u8).add(i * core::mem::size_of::<usize>()) });
            }
            gc::collect_root(&ptrs);
        }
    });
}

pub fn gc_collect() {
    if !mpconfig::ENABLE_GC { return; }
    gc::collect_start();
    gc_collect_regs_and_stack();
    if mpconfig::PY_THREAD {
        super::mpthreadport::gc_others();
    }
    gc::collect_end();
}
