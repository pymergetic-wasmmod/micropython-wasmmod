//! rewrite of mpy-cross/gccollect.c
// symmetry: done

use py_rs::gc;
use py_rs::mpconfig;
use shared_rs::runtime::gchelper;

/// Port GC collection hook (`gc_collect`).
pub fn gc_collect() {
    if !mpconfig::ENABLE_GC {
        return;
    }
    gc::collect_start();
    gchelper::collect_regs_and_stack();
    gc::collect_end();
}
