//! rewrite of shared/runtime/gchelper_thumb1.s
// symmetry: done

use super::gchelper::GcHelperRegs;

/// Thumb-1 register capture (`gc_helper_get_regs_and_sp`).
#[cfg(all(target_arch = "arm", target_feature = "thumb-mode"))]
pub fn get_regs_and_sp(regs: &mut GcHelperRegs) -> usize {
    let mut sp: usize;
    unsafe {
        core::arch::asm!(
            "str r4, [{regs_ptr}, #0]",
            "str r5, [{regs_ptr}, #4]",
            "str r6, [{regs_ptr}, #8]",
            "str r7, [{regs_ptr}, #12]",
            "mov r1, r8",
            "str r1, [{regs_ptr}, #16]",
            "mov r1, r9",
            "str r1, [{regs_ptr}, #20]",
            "mov r1, r10",
            "str r1, [{regs_ptr}, #24]",
            "mov r1, r11",
            "str r1, [{regs_ptr}, #28]",
            "mov r1, r12",
            "str r1, [{regs_ptr}, #32]",
            "mov r1, r13",
            "str r1, [{regs_ptr}, #36]",
            "mov {sp_out}, sp",
            regs_ptr = in(reg) regs.as_mut_ptr(),
            sp_out = out(reg) sp,
            options(nomem)
        );
    }
    sp
}

#[cfg(not(all(target_arch = "arm", target_feature = "thumb-mode")))]
pub fn get_regs_and_sp(_regs: &mut GcHelperRegs) -> usize {
    0
}
