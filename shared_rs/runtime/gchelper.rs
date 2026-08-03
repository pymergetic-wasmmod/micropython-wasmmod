//! rewrite of shared/runtime/gchelper.h
// symmetry: done

pub use super::gchelper_generic::collect_regs_and_stack;

/// Callee-saved register capture buffer (`gc_helper_regs_t`).
pub type GcHelperRegs = [u8; 200];
