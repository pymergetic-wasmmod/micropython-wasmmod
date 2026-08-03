#!/usr/bin/env python3
"""Retarget false-`done` shadows to `// symmetry: gaps` with listed reasons.

Usage:
  python3 rewrite/mark_gaps.py
"""

from __future__ import annotations

import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]

# path relative to repo → gap bullet reasons
GAPS: dict[str, list[str]] = {}

# machine_* host placeholders (need port HAL)
for name in [
    "machine_adc",
    "machine_adc_block",
    "machine_bitstream",
    "machine_can",
    "machine_i2c",
    "machine_i2c_target",
    "machine_i2s",
    "machine_mem",
    "machine_pinbase",
    "machine_pulse",
    "machine_pwm",
    "machine_signal",
    "machine_spi",
    "machine_uart",
    "machine_usb_device",
    "machine_wdt",
]:
    GAPS[f"extmod_rs/{name}.rs"] = [
        "port HAL / peripheral wiring not available on host unix rewrite yet",
        "types registered only as placeholders until port backends exist",
    ]

# short / structural extmod modules (size asymmetry vs C)
# Fully-ported extmods (do not mark gaps): modheapq, modbinascii, modrandom.
# modjson is being completed separately — omit here.
for name in [
    "modframebuf",
    "moddeflate",
    "modcryptolib",
    "modhashlib",
    "modre",
    "modselect",
    "modsocket",
    "modtime",
    "moductypes",
    "modvfs",
    "modwebrepl",
    "modbluetooth",
    "modbtree",
    "modlwip",
    "modmachine",
    "modmarshal",
    "modnetwork",
    "modonewire",
    "modopenamp",
    "modopenamp_remoteproc",
    "modopenamp_remoteproc_store",
    "modos",
    "modplatform",
    "modtls_axtls",
    "modtls_mbedtls",
    "network_cyw43",
    "network_esp_hosted",
    "network_lwip",
    "network_ninaw10",
    "network_ppp_lwip",
    "network_wiznet5k",
    "vfs",
    "vfs_blockdev",
    "vfs_fat",
    "vfs_fat_diskio",
    "vfs_fat_file",
    "vfs_lfs",
    "vfs_lfsx",
    "vfs_lfsx_file",
    "vfs_posix",
    "vfs_posix_file",
    "vfs_reader",
    "vfs_rom",
    "vfs_rom_file",
    "virtpin",
    "misc",
    "mpbthci",
    "os_dupterm",
    "font_petme128_8x8",
    "cyw43_config_common",
    "machine_can_port",
    "wasm_pack",
    "wasm_fetch",
    "wasm_forward",
    "wasm_finder",
    "wasm_verify",
]:
    GAPS[f"extmod_rs/{name}.rs"] = [
        "API surface / init_module only — behavior not yet parity with C reference",
    ]

for name in [
    "asyncio/core",
    "asyncio/task",
    "asyncio/event",
    "asyncio/lock",
    "asyncio/funcs",
    "asyncio/stream",
    "asyncio/uasyncio",
    "asyncio/__init__",
]:
    GAPS[f"extmod_rs/{name}.rs"] = [
        "Python asyncio rewrite incomplete vs extmod/asyncio/*.py (event loop / tasks)",
    ]

# py_rs known gaps
GAPS.update(
    {
        "py_rs/emitnative.rs": [
            "native emitter body incomplete vs py/emitnative.c (~3k lines)",
            "arch backends compile but are not full C parity",
        ],
        "py_rs/emitnative_impl.rs": [
            "shared native emit impl still missing many emitnative.c paths",
        ],
        "py_rs/objstr.rs": [
            "string method surface incomplete vs py/objstr.c (many methods still thin)",
        ],
        "py_rs/objgenerator.rs": [
            "generator helpers still note incomplete objfun integration paths",
        ],
        "py_rs/runtime.rs": [
            "mp_resume / some runtime paths are host-simplified vs py/runtime.c",
        ],
        "py_rs/persistentcode.rs": [
            "mpy save/load uses simplified raw-code path vs full persistentcode.c",
        ],
        "py_rs/nativeglue.rs": [
            "native function table is a host placeholder until emit-native is complete",
        ],
        "py_rs/mpthread.rs": [
            "GIL/thread stubs when MICROPY_PY_THREAD is off or host lacks port threads",
        ],
        "py_rs/objexcept.rs": [
            "emergency exception buffer simplified vs full C emergency buf logic",
        ],
        "ports_rs/unix/modffi.rs": [
            "ffi callable objects are placeholders pending libffi host integration",
        ],
    }
)


def apply_gaps(path: Path, reasons: list[str]) -> bool:
    text = path.read_text(encoding="utf-8")
    if "// symmetry: gaps" in text:
        return False
    # Replace done/partial/stub marker
    new = re.sub(
        r"^// symmetry: (done|partial|stub)\s*$",
        "// symmetry: gaps",
        text,
        count=1,
        flags=re.M,
    )
    if new == text:
        # insert after rewrite-of line
        lines = text.splitlines(keepends=True)
        out: list[str] = []
        inserted = False
        for i, ln in enumerate(lines):
            out.append(ln)
            if not inserted and ln.startswith("//! rewrite"):
                out.append("// symmetry: gaps\n")
                inserted = True
        if not inserted:
            out.insert(0, "// symmetry: gaps\n")
        new = "".join(out)

    # Ensure gaps block exists after marker
    if "// gaps:" not in new:
        block = "// gaps:\n" + "".join(f"// - {r}\n" for r in reasons)
        new = new.replace("// symmetry: gaps\n", "// symmetry: gaps\n" + block, 1)
    path.write_text(new, encoding="utf-8")
    return True


def main() -> None:
    n = 0
    for rel, reasons in sorted(GAPS.items()):
        path = ROOT / rel
        if not path.is_file():
            print(f"skip missing {rel}")
            continue
        if apply_gaps(path, reasons):
            print(f"gaps ← {rel}")
            n += 1
        else:
            print(f"keep {rel}")
    print(f"updated {n} files")


if __name__ == "__main__":
    main()
