#!/usr/bin/env python3
"""Generate py_rs/vm.rs opcode dispatch arms from py/vm.c ENTRY() blocks."""

from __future__ import annotations

import re
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]
VM_C = REPO / "py" / "vm.c"
OUT = REPO / "py_rs" / "vm_dispatch.inc.rs"


def main() -> None:
    text = VM_C.read_text()
    # Extract switch-body between "switch (*ip++) {" and matching closing before pending_exception_check
    m = re.search(r"#else\s+TRACE\(ip\);.*?switch \(\*ip\+\+\) \{(.*?)\n\s+#endif\s+\n\s+pending_exception_check:", text, re.S)
    if not m:
        raise SystemExit("could not find dispatch switch body")
    body = m.group(1)
    # Convert ENTRY(MP_BC_XXX): to Rust arms
    body = re.sub(r"ENTRY\(MP_BC_([A-Z0-9_]+)\):", r"bc0::\1 => {", body)
    body = re.sub(r"ENTRY_DEFAULT:", r"_ => {", body)
    # Remove C preprocessor blocks we don't need
    body = re.sub(r"#if MICROPY_[^\n]*\n", "", body)
    body = re.sub(r"#else[^\n]*\n", "", body)
    body = re.sub(r"#endif[^\n]*\n", "", body)
    body = re.sub(r"#if 0.*?#endif\s*", "", body, flags=re.S)
    OUT.write_text("// generated from py/vm.c — do not edit by hand\n" + body)
    print(f"wrote {OUT}")


if __name__ == "__main__":
    main()
