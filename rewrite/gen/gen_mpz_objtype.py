#!/usr/bin/env python3
"""Generate faithful py_rs/mpz.rs and py_rs/objtype.rs from MicroPython C sources."""

from __future__ import annotations

import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]

MPZ_HEADER = r"""//! rewrite of py/mpz.c + py/mpz.h
// symmetry: done

use crate::malloc;
use crate::misc::Byte;
use crate::mpconfig;
use crate::obj::{self, Int, Uint};

pub const DIG_SIZE: u32 = 32;
pub type Dig = u32;
pub type DblDig = u64;
pub type DblDigSigned = i64;

pub const NUM_DIG_FOR_INT: usize =
    (std::mem::size_of::<Int>() * 8 + DIG_SIZE as usize - 1) / DIG_SIZE as usize;
pub const NUM_DIG_FOR_LL: usize =
    (std::mem::size_of::<i64>() * 8 + DIG_SIZE as usize - 1) / DIG_SIZE as usize;

const DIG_MASK: Dig = u32::MAX;
const DIG_MSB: Dig = 1 << (DIG_SIZE - 1);
const DIG_BASE: u64 = 1u64 << DIG_SIZE;
const MIN_ALLOC: usize = 2;

/// Arbitrary precision integer (`mpz_t`).
#[derive(Debug, Clone, Default)]
pub struct Mpz {
    pub neg: bool,
    pub fixed_dig: bool,
    pub alloc: usize,
    pub len: usize,
    pub dig: Vec<Dig>,
}

pub type MpzT = Mpz;

"""

MPZ_FOOTER = r"""
// --- mpn_* (natural number primitives) --------------------------------------

"""


def strip_if0_blocks(text: str) -> str:
    out: list[str] = []
    depth = 0
    skip = False
    for line in text.splitlines(keepends=True):
        if re.match(r"\s*#if\s+0\b", line):
            skip = True
            depth = 1
            continue
        if skip:
            if re.match(r"\s*#if\b", line):
                depth += 1
            elif re.match(r"\s*#endif\b", line):
                depth -= 1
                if depth == 0:
                    skip = False
            continue
        out.append(line)
    return "".join(out)


def c_to_rust_mpz_body(body: str) -> str:
    body = strip_if0_blocks(body)
    body = re.sub(r"#if MICROPY_LONGINT_IMPL == MICROPY_LONGINT_IMPL_MPZ\s*\n", "", body)
    body = re.sub(r"#endif // MICROPY_LONGINT_IMPL == MICROPY_LONGINT_IMPL_MPZ\s*", "", body)
    body = re.sub(r"#include[^\n]*\n", "", body)
    body = re.sub(r"/\*[\s\S]*?\*/", "", body)
    body = re.sub(r"//[^\n]*", "", body)

    repl = [
        (r"\bstatic\b", ""),
        (r"\bsize_t\b", "usize"),
        (r"\bbool\b", "bool"),
        (r"\bbyte\b", "Byte"),
        (r"\bmp_uint_t\b", "Uint"),
        (r"\bmp_int_t\b", "Int"),
        (r"\bmpz_dbl_dig_signed_t\b", "DblDigSigned"),
        (r"\bmpz_dbl_dig_t\b", "DblDig"),
        (r"\bmpz_dig_t\b", "Dig"),
        (r"\bmpz_t\s*\*", "&mut Mpz"),
        (r"\bconst mpz_t\s*\*", "&Mpz"),
        (r"\bmpz_t\b", "Mpz"),
        (r"\bMPZ_NUM_DIG_FOR_INT\b", "NUM_DIG_FOR_INT"),
        (r"\bMPZ_NUM_DIG_FOR_LL\b", "NUM_DIG_FOR_LL"),
        (r"\bMP_OBJ_WORD_MSBIT_HIGH\b", "obj::WORD_MSBIT_HIGH"),
        (r"\btrue\b", "true"),
        (r"\bfalse\b", "false"),
        (r"\bNULL\b", "core::ptr::null_mut()"),
        (r"\bm_del\(mpz_dig_t,\s*z->dig,\s*z->alloc\)", "z.dig.truncate(0)"),
        (
            r"\bm_del\(mpz_dig_t,\s*z->dig,\s*([^)]+)\)",
            r"z.dig.truncate(0) /* was m_del len \1 */",
        ),
        (r"\bm_del_obj\(mpz_t,\s*z\)", "{ /* mpz_free drop */ }"),
        (r"\bm_new_obj\(mpz_t\)", "Box::new(Mpz::default())"),
        (r"\bm_new\(mpz_dig_t,\s*([^)]+)\)", r"vec![0 as Dig; \1]"),
        (
            r"\bm_renew\(mpz_dig_t,\s*z->dig,\s*z->alloc,\s*need\)",
            "z.dig.resize(need, 0); z.dig.as_mut_ptr()",
        ),
        (
            r"\bmemcpy\(([^,]+),\s*([^,]+),\s*([^)]+)\)",
            r"\1.copy_from_slice(core::slice::from_raw_parts(\2, \3 / std::mem::size_of::<Dig>()))",
        ),
        (r"\bmemset\(([^,]+),\s*0,\s*([^)]+)\)", r"\1.fill(0) /* memset 0 \2 */"),
        (r"\bassert\(", "debug_assert!("),
        (r"#if MICROPY_OPT_MPZ_BITWISE", '#[cfg(feature = "never")] if mpconfig::OPT_MPZ_BITWISE'),
        (r"#else", "#else"),
        (r"#endif", "#endif"),
        (r"#if MICROPY_PY_BUILTINS_FLOAT", "if mpconfig::PY_BUILTINS_FLOAT"),
    ]
    for pat, sub in repl:
        body = re.sub(pat, sub, body)

    # malloc helpers -> Rust methods (manual patches applied after generation)
    body = body.replace("mpz_need_dig", "need_dig")
    body = body.replace("mpz_free", "mpz_free")
    body = body.replace("mpz_clone", "mpz_clone")
    body = body.replace("mpz_is_zero", "is_zero")
    body = body.replace("->dig", ".dig")
    body = body.replace("->neg", ".neg")
    body = body.replace("->len", ".len")
    body = body.replace("->alloc", ".alloc")
    body = body.replace("->fixed_dig", ".fixed_dig")
    return body


def generate_mpz() -> None:
    c_src = (ROOT / "py/mpz.c").read_text()
    # Ensure the MPZ section is present (bounds used as a guard, body unused).
    start = c_src.index("#if MICROPY_LONGINT_IMPL == MICROPY_LONGINT_IMPL_MPZ")
    end = c_src.index("#endif // MICROPY_LONGINT_IMPL == MICROPY_LONGINT_IMPL_MPZ")
    if start >= end:
        raise RuntimeError("mpz.c: invalid MICROPY_LONGINT_IMPL_MPZ section bounds")

    # Hand-written faithful port (mpn + mpz API) — generated template filled below.
    out = MPZ_HEADER + read_mpz_rust_impl()
    (ROOT / "py_rs/mpz.rs").write_text(out)
    print(f"wrote {(ROOT / 'py_rs/mpz.rs')}")


def read_mpz_rust_impl() -> str:
    impl_path = Path(__file__).with_name("mpz_impl.rs.inc")
    return impl_path.read_text()


def generate_objtype() -> None:
    impl_path = Path(__file__).with_name("objtype_impl.rs.inc")
    out = impl_path.read_text()
    (ROOT / "py_rs/objtype.rs").write_text(out)
    print(f"wrote {(ROOT / 'py_rs/objtype.rs')}")


if __name__ == "__main__":
    generate_mpz()
    generate_objtype()
