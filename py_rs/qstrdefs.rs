//! rewrite of py/qstrdefs.h
// symmetry: done

use crate::mpconfig;
use crate::qstr::{self, Qstr};

/// Qstr configuration metadata consumed by the qstr code generator.
pub const QCFG_BYTES_IN_LEN: usize = mpconfig::QSTR_BYTES_IN_LEN;
pub const QCFG_BYTES_IN_HASH: usize = mpconfig::QSTR_BYTES_IN_HASH;

/// Static qstr entries from `qstrdefs.h` (feature-gated like the C header).
pub fn register_static_qstrdefs() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let mut table: &[&[u8]] = &[
            b"*",
            b"_",
            b"/",
            b" ",
            b"\n",
            b"<module>",
            b"<lambda>",
            b"<listcomp>",
            b"<dictcomp>",
            b"<setcomp>",
            b"<genexpr>",
            b"<string>",
            b"<stdin>",
            b"utf-8",
        ];
        if mpconfig::PY_SYS_PS1_PS2 {
            table = &[
                b"*", b"_", b"/", b">>> ", b"... ",
                b" ", b"\n", b"<module>", b"<lambda>", b"<listcomp>",
                b"<dictcomp>", b"<setcomp>", b"<genexpr>", b"<string>", b"<stdin>", b"utf-8",
            ];
        }
        if mpconfig::PY_BUILTINS_STR_OP_MODULO {
            let _ = qstr::from_strn(b"%#o");
            let _ = qstr::from_strn(b"%#x");
        } else {
            let _ = qstr::from_strn(b"{:#o}");
            let _ = qstr::from_strn(b"{:#x}");
        }
        let _ = qstr::from_strn(b"{:#b}");
        if mpconfig::STACK_CHECK {
            let _ = qstr::from_strn(b"maximum recursion depth exceeded");
        }
        if mpconfig::MODULE_FROZEN {
            let _ = qstr::from_strn(b".frozen");
        }
        if mpconfig::ENABLE_PYSTACK {
            let _ = qstr::from_strn(b"pystack exhausted");
        }
        if mpconfig::PY_TSTRINGS {
            let _ = qstr::from_strn(b"string.templatelib");
        }
        for &s in table {
            let _ = qstr::from_strn(s);
        }
    });
}

/// Last static qstr id after registering defs (host mirror of generated tables).
pub fn qstr_last_static() -> Qstr {
    register_static_qstrdefs();
    qstr::total()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn static_defs_intern() {
        qstr::init();
        register_static_qstrdefs();
        assert!(qstr::find_strn(b"<module>") != qstr::QSTR_NULL);
    }
}
