//! rewrite of shared/runtime/gchelper_rv32i.s
// symmetry: done

use super::gchelper::GcHelperRegs;

/// RV32I register capture (`gc_helper_get_regs_and_sp`).
#[cfg(target_arch = "riscv32")]
pub fn get_regs_and_sp(regs: &mut GcHelperRegs) -> usize {
    let mut sp: usize;
    unsafe {
        core::arch::asm!(
            "sw x8,  0({regs_ptr})",
            "sw x9,  4({regs_ptr})",
            "sw x18, 8({regs_ptr})",
            "sw x19, 12({regs_ptr})",
            "sw x20, 16({regs_ptr})",
            "sw x21, 20({regs_ptr})",
            "sw x22, 24({regs_ptr})",
            "sw x23, 28({regs_ptr})",
            "sw x24, 32({regs_ptr})",
            "sw x25, 36({regs_ptr})",
            "sw x26, 40({regs_ptr})",
            "sw x27, 44({regs_ptr})",
            "mv {sp_out}, sp",
            regs_ptr = in(reg) regs.as_mut_ptr(),
            sp_out = out(reg) sp,
            options(nomem)
        );
    }
    sp
}

#[cfg(not(target_arch = "riscv32"))]
pub fn get_regs_and_sp(_regs: &mut GcHelperRegs) -> usize {
    0
}
