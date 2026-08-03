//! rewrite of shared/runtime/gchelper_rv64i.s
// symmetry: done

use super::gchelper::GcHelperRegs;

/// RV64I register capture (`gc_helper_get_regs_and_sp`).
#[cfg(target_arch = "riscv64")]
pub fn get_regs_and_sp(regs: &mut GcHelperRegs) -> usize {
    let mut sp: usize;
    unsafe {
        core::arch::asm!(
            "sd x8,  0({regs_ptr})",
            "sd x9,  8({regs_ptr})",
            "sd x18, 16({regs_ptr})",
            "sd x19, 24({regs_ptr})",
            "sd x20, 32({regs_ptr})",
            "sd x21, 40({regs_ptr})",
            "sd x22, 48({regs_ptr})",
            "sd x23, 56({regs_ptr})",
            "sd x24, 64({regs_ptr})",
            "sd x25, 72({regs_ptr})",
            "sd x26, 80({regs_ptr})",
            "sd x27, 88({regs_ptr})",
            "mv {sp_out}, sp",
            regs_ptr = in(reg) regs.as_mut_ptr(),
            sp_out = out(reg) sp,
            options(nomem)
        );
    }
    sp
}

#[cfg(not(target_arch = "riscv64"))]
pub fn get_regs_and_sp(_regs: &mut GcHelperRegs) -> usize {
    0
}
