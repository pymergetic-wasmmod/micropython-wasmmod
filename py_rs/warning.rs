//! rewrite of py/warning.c
// symmetry: done

use crate::mpconfig;
use crate::mpprint::{self, Print, VaArg};

/// Compiler pass kind used by emitter warnings (`pass_kind_t` subset).
#[repr(u8)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum PassKind {
    Scope = 1,
    StackSize = 2,
    CodeSize = 3,
    Emit = 4,
}

/// `mp_warning` — print to the error printer when `MICROPY_WARNINGS` is enabled.
pub fn warning(category: Option<&str>, msg: &str, args: &[VaArg<'_>]) {
    if !mpconfig::WARNINGS {
        return;
    }
    let category = category.unwrap_or("Warning");
    let print: &Print = &mpprint::PLAT_PRINT;
    mpprint::print_str(print, category);
    mpprint::print_str(print, ": ");
    mpprint::printf(print, msg, args.iter().cloned());
    mpprint::print_str(print, "\n");
}

/// `mp_emitter_warning` — only emit during the code-size pass.
pub fn emitter_warning(pass: PassKind, msg: &str) {
    if !mpconfig::WARNINGS {
        return;
    }
    if pass == PassKind::CodeSize {
        warning(None, msg, &[]);
    }
}
