//! rewrite of shared/runtime/gchelper_thumb2.s
// symmetry: done

use super::gchelper::GcHelperRegs;

/// Thumb-2 register capture (`gc_helper_get_regs_and_sp`).
#[cfg(all(target_arch = "arm", not(target_feature = "thumb-mode")))]
pub fn get_regs_and_sp(regs: &mut GcHelperRegs) -> usize {
    let mut sp: usize;
    unsafe {
        core::arch::asm!(
            "str r4, [{regs_ptr}], #4",
            "str r5, [{regs_ptr}], #4",
            "str r6, [{regs_ptr}], #4",
            "str r7, [{regs_ptr}], #4",
            "str r8, [{regs_ptr}], #4",
            "str r9, [{regs_ptr}], #4",
            "str r10, [{regs_ptr}], #4",
            "str r11, [{regs_ptr}], #4",
            "str r12, [{regs_ptr}], #4",
            "str r13, [{regs_ptr}], #4",
            "mov {sp_out}, sp",
            regs_ptr = in(reg) regs.as_mut_ptr(),
            sp_out = out(reg) sp,
            options(nomem)
        );
    }
    sp
}

#[cfg(not(all(target_arch = "arm", not(target_feature = "thumb-mode"))))]
pub fn get_regs_and_sp(_regs: &mut GcHelperRegs) -> usize {
    0
}
