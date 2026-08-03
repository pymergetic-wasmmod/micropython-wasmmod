//! rewrite of shared/runtime/gchelper_loong64.s
// symmetry: done

use super::gchelper::GcHelperRegs;

/// LOONG64 register capture (`gc_helper_get_regs_and_sp`).
#[cfg(target_arch = "loongarch64")]
pub fn get_regs_and_sp(regs: &mut GcHelperRegs) -> usize {
    let mut sp: usize;
    unsafe {
        core::arch::asm!(
            "st.d $r23, $r4, 0",
            "st.d $r24, $r4, 8",
            "st.d $r25, $r4, 16",
            "st.d $r26, $r4, 24",
            "st.d $r27, $r4, 32",
            "st.d $r28, $r4, 40",
            "st.d $r29, $r4, 48",
            "st.d $r30, $r4, 56",
            "st.d $r31, $r4, 64",
            "st.d $r22, $r4, 72",
            "add.d {sp_out}, $r0, $r3",
            regs_ptr = in(reg) regs.as_mut_ptr(),
            sp_out = out(reg) sp,
            options(nomem)
        );
    }
    sp
}

#[cfg(not(target_arch = "loongarch64"))]
pub fn get_regs_and_sp(_regs: &mut GcHelperRegs) -> usize {
    0
}
