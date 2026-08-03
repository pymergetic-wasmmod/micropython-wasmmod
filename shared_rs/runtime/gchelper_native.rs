//! rewrite of shared/runtime/gchelper_native.c
// symmetry: done

use py_rs::gc;
use py_rs::mpconfig;
use py_rs::mpstate;

use super::gchelper::GcHelperRegs;

/// Assembly helper: store callee-saved registers and return the stack pointer.
#[cfg(all(target_arch = "arm", target_feature = "thumb-mode"))]
pub fn get_regs_and_sp(regs: &mut GcHelperRegs) -> usize {
    super::gchelper_thumb1::get_regs_and_sp(regs)
}

#[cfg(all(target_arch = "arm", not(target_feature = "thumb-mode")))]
pub fn get_regs_and_sp(regs: &mut GcHelperRegs) -> usize {
    super::gchelper_thumb2::get_regs_and_sp(regs)
}

#[cfg(target_arch = "riscv32")]
pub fn get_regs_and_sp(regs: &mut GcHelperRegs) -> usize {
    super::gchelper_rv32i::get_regs_and_sp(regs)
}

#[cfg(target_arch = "riscv64")]
pub fn get_regs_and_sp(regs: &mut GcHelperRegs) -> usize {
    super::gchelper_rv64i::get_regs_and_sp(regs)
}

#[cfg(target_arch = "loongarch64")]
pub fn get_regs_and_sp(regs: &mut GcHelperRegs) -> usize {
    super::gchelper_loong64::get_regs_and_sp(regs)
}

#[cfg(not(any(
    all(
        target_arch = "arm",
        any(target_feature = "thumb-mode", not(target_feature = "thumb-mode"))
    ),
    target_arch = "riscv32",
    target_arch = "riscv64",
    target_arch = "loongarch64"
)))]
pub fn get_regs_and_sp(_regs: &mut GcHelperRegs) -> usize {
    let mut anchor = 0usize;
    &mut anchor as *mut usize as usize
}

/// `gc_helper_collect_regs_and_stack` — native register capture path.
#[inline(never)]
pub fn collect_regs_and_stack() {
    if !mpconfig::ENABLE_GC {
        return;
    }

    let mut regs = [0u8; 200];
    let sp = get_regs_and_sp(&mut regs);
    mpstate::with_thread(|t| {
        let stack_top = t.stack_top as usize;
        if stack_top > sp {
            let count = (stack_top - sp) / core::mem::size_of::<usize>();
            // `collect_root_words` reads the *contents* of each slot (the saved
            // register / stack values) as a candidate pointer.
            gc::collect_root_words(sp as *const u8, count);
        }
    });
}
