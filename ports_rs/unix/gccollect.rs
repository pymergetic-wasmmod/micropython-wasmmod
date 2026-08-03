use py_rs::gc;
use py_rs::mpconfig;
use py_rs::mpstate;
use shared_rs::runtime::gchelper;

/// Scan register/stack roots (host setjmp-style path).
#[inline(never)]
pub fn gc_collect_regs_and_stack() {
    gchelper::collect_regs_and_stack();
}

pub fn gc_collect() {
    if !mpconfig::ENABLE_GC {
        return;
    }
    gc::collect_start();
    gc_collect_regs_and_stack();
    if mpconfig::PY_THREAD {
        super::mpthreadport::gc_others();
    }
    gc::collect_end();
}
