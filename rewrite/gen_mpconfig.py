#!/usr/bin/env python3
"""Generate py_rs/mpconfig.rs from py/mpconfig.h + unix port overrides.

Reads MicroPython C headers only; writes to py_rs/. Never modifies originals.
"""

from __future__ import annotations

import re
import sys
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
MPCONFIG_H = ROOT / "py" / "mpconfig.h"
OUT_RS = ROOT / "py_rs" / "mpconfig.rs"

UNIX_VARIANT_DIR = ROOT / "ports" / "unix" / "variants" / "standard"
UNIX_COMMON = ROOT / "ports" / "unix" / "variants" / "mpconfigvariant_common.h"
UNIX_OVERRIDES = [
    UNIX_VARIANT_DIR / "mpconfigvariant.h",
    ROOT / "ports" / "unix" / "mpconfigport.h",
]

# Pre-seeded platform symbols for a 64-bit little-endian Linux host.
PLATFORM_DEFINES: dict[str, str] = {
    "__BYTE_ORDER__": "__ORDER_LITTLE_ENDIAN__",
    "__ORDER_LITTLE_ENDIAN__": "1234",
    "__ORDER_BIG_ENDIAN__": "4321",
    "INTPTR_MAX": "9223372036854775807",
    "INTPTR_MIN": "-9223372036854775808",
    "INTPTR_UMAX": "18446744073709551615",
    "INT_MAX": "2147483647",
    "INT_MIN": "-2147483648",
    "LONG_MAX": "9223372036854775807",
    "LONG_MIN": "-9223372036854775808",
    "LLONG_MAX": "9223372036854775807",
    "LLONG_MIN": "-9223372036854775808",
    "ULLONG_MAX": "18446744073709551615",
    "SIZE_MAX": "18446744073709551615",
    "SSIZE_MAX": "9223372036854775807",
    "PATH_MAX": "4096",
    "SOMAXCONN": "128",
    "sizeof(mp_uint_t)": "8",
    "sizeof(mp_int_t)": "8",
    "sizeof(mp_obj_t)": "8",
    "sizeof(size_t)": "8",
    "sizeof(void*)": "8",
    "sizeof(byte)": "1",
    # Features referenced before mpconfig.h defaults are fully visible.
    "MICROPY_PY_BLUETOOTH": "0",
    "MICROPY_PY_FFI": "0",
    "MICROPY_BLUETOOTH_NIMBLE": "0",
    "MICROPY_PY_MARSHAL": "1",
    "MICROPY_PY_MACHINE_MEM_BACKUP": "0",
    "MICROPY_PY_SYS_SETTRACE": "0",
    "MICROPY_PY_THREAD": "0",
    "MICROPY_PY_SYS_ATTR_DELEGATION": "0",
    "MICROPY_PY_SSL": "1",
    "MICROPY_SSL_MBEDTLS": "1",
    "MICROPY_PY_SSL_ECDSA_SIGN_ALT": "0",
    "MICROPY_PREVIEW_VERSION_2": "0",
    "MICROPY_GIT_TAG": "\"v1.29.0\"",
    "MICROPY_BUILD_DATE": "\"2026-01-01\"",
    "MICROPY_PLATFORM_COMPILER": "\"rustc\"",
    "MICROPY_HW_BOARD_NAME": "0",
    "MICROPY_HW_MCU_NAME": "0",
}

# Macros we emit manually at the top of mpconfig.rs (not generated).
MANUAL_NAMES = frozenset(
    {
        "MICROPY_VERSION_MAJOR",
        "MICROPY_VERSION_MINOR",
        "MICROPY_VERSION_MICRO",
        "MICROPY_VERSION_PRERELEASE",
        "MICROPY_MAKE_VERSION",
        "MICROPY_VERSION",
        "MICROPY_VERSION_STRING",
        "MICROPY_VERSION_STRING_BASE",
        "MICROPY_IMPLEMENTATION_NAME",
        "MICROPY_GC_HEAP_SIZE",
    }
)

# Skip function-like, hook, or non-const macros.
SKIP_PATTERNS = (
    re.compile(r"\(f\)"),
    re.compile(r"\(p\)"),
    re.compile(r"\(min_size"),
    re.compile(r"\(x\)"),
    re.compile(r"\(major"),
    re.compile(r"\(msg\)"),
    re.compile(r"\(cond\)"),
    re.compile(r"\(i\)"),
    re.compile(r"\(n\)"),
    re.compile(r"\(buf\)"),
    re.compile(r"do \{"),
    re.compile(r"while \(0\)"),
    re.compile(r"__attribute__"),
    re.compile(r"^&"),
    re.compile(r"^[a-z_][a-z0-9_]*$"),  # bare identifier function ref
)

SKIP_EXACT = frozenset(
    {
        "MICROPY_VM_HOOK_INIT",
        "MICROPY_VM_HOOK_LOOP",
        "MICROPY_VM_HOOK_RETURN",
        "MICROPY_SCHED_HOOK_SCHEDULED",
        "MICROPY_GC_HOOK_LOOP",
        "MICROPY_PORT_BUILTINS",
        "MICROPY_PORT_EXTRA_BUILTINS",
        "MICROPY_PORT_CONSTANTS",
        "MICROPY_OBJ_BASE_ALIGNMENT",
        "MICROPY_INCLUDED_PY_MPCONFIG_H",
        "MP_CONFIGFILE",
        "MP_STATE_PORT",
        "MP_STATE_VM",
        "MP_PLAT_ALLOC_EXEC",
        "MP_PLAT_FREE_EXEC",
        "MP_PLAT_ALLOC_HEAP",
        "MP_PLAT_FREE_HEAP",
        "MICROPY_MACHINE_MEM_GET_READ_ADDR",
        "MICROPY_MACHINE_MEM_GET_WRITE_ADDR",
        "MICROPY_PY_MACHINE_INCLUDEFILE",
        "MICROPY_PY_OS_INCLUDEFILE",
        "MICROPY_PY_TIME_INCLUDEFILE",
        "MICROPY_DEBUG_PRINTER",
        "MICROPY_ERROR_PRINTER",
        "MICROPY_PY_RANDOM_SEED_INIT_FUNC",
        "MICROPY_UNIX_MACHINE_IDLE",
        "MICROPY_FLOAT_CONST",
        "MICROPY_WRAP_MP_BINARY_OP",
        "MICROPY_WRAP_MP_EXECUTE_BYTECODE",
        "MICROPY_WRAP_MP_LOAD_GLOBAL",
        "MICROPY_WRAP_MP_LOAD_NAME",
        "MICROPY_WRAP_MP_MAP_LOOKUP",
        "MICROPY_WRAP_MP_OBJ_GET_TYPE",
        "MICROPY_WRAP_MP_SCHED_EXCEPTION",
        "MICROPY_WRAP_MP_SCHED_KEYBOARD_INTERRUPT",
        "MICROPY_WRAP_MP_SCHED_SCHEDULE",
        "MICROPY_WRAP_MP_SCHED_VM_ABORT",
        "MICROPY_MAKE_POINTER_CALLABLE",
        "MICROPY_BANNER_NAME_AND_VERSION",
        "MICROPY_BANNER_MACHINE",
        "INT_FMT",
        "UINT_FMT",
        "HEX_FMT",
        "SIZE_FMT",
        "MP_NORETURN",
        "MP_WEAK",
        "MP_NOINLINE",
        "MP_ALWAYSINLINE",
        "MP_LIKELY",
        "MP_UNLIKELY",
        "MP_FALLTHROUGH",
        "MP_UNREACHABLE",
    }
)


def strip_comments(text: str) -> str:
    out: list[str] = []
    i = 0
    n = len(text)
    while i < n:
        if text[i : i + 2] == "//":
            while i < n and text[i] != "\n":
                i += 1
        elif text[i : i + 2] == "/*":
            i += 2
            while i < n - 1 and text[i : i + 2] != "*/":
                i += 1
            i += 2
        else:
            out.append(text[i])
            i += 1
    return "".join(out)


def join_lines(lines: list[str], start: int) -> tuple[str, int]:
    parts = [lines[start].strip()]
    i = start + 1
    while i < len(lines) and lines[i].strip().endswith("\\"):
        parts.append(lines[i].strip().rstrip("\\").strip())
        i += 1
    return " ".join(parts), i


@dataclass
class Preprocessor:
    defines: dict[str, str] = field(default_factory=dict)
    defined: set[str] = field(default_factory=set)
    _processing: set[Path] = field(default_factory=set)

    def __post_init__(self) -> None:
        for k, v in PLATFORM_DEFINES.items():
            self.defines[k] = v
            self.defined.add(k)

    def set_def(self, name: str, value: str) -> None:
        self.defines[name] = value.strip()
        self.defined.add(name)

    def is_defined(self, name: str) -> bool:
        if name in self.defined:
            return True
        if name in self.defines:
            val = self.eval_expr(self.defines[name])
            if isinstance(val, bool):
                return val
            if isinstance(val, int):
                return val != 0
            if isinstance(val, str):
                return val not in ("", "0", '""')
        return False

    def eval_expr(self, expr: str, _depth: int = 0) -> Any:
        if _depth > 64:
            raise RuntimeError(f"expression depth exceeded: {expr!r}")
        expr = expr.strip()
        if not expr:
            return 0

        # String literal
        if (expr.startswith('"') and expr.endswith('"')) or (
            expr.startswith("'") and expr.endswith("'")
        ):
            return expr

        # Parenthesized
        if expr.startswith("(") and expr.endswith(")"):
            inner = expr[1:-1].strip()
            if self._balanced_parens(inner):
                return self.eval_expr(inner, _depth + 1)

        # defined(X)
        m = re.fullmatch(r"defined\s*\(\s*(\w+)\s*\)", expr)
        if m:
            return self.is_defined(m.group(1))

        # Numeric
        if re.fullmatch(r"-?\d+", expr):
            return int(expr)
        if re.fullmatch(r"0[xX][0-9a-fA-F]+", expr):
            return int(expr, 16)

        # Unary !
        if expr.startswith("!"):
            v = self.eval_expr(expr[1:], _depth + 1)
            return not bool(v)

        # Ternary cond ? a : b
        qpos = self._find_top_op(expr, "?")
        if qpos >= 0:
            cond = expr[:qpos].strip()
            rest = expr[qpos + 1 :]
            cpos = self._find_top_op(rest, ":")
            if cpos >= 0:
                when_true = rest[:cpos].strip()
                when_false = rest[cpos + 1 :].strip()
                return (
                    self.eval_expr(when_true, _depth + 1)
                    if self.eval_cond(cond)
                    else self.eval_expr(when_false, _depth + 1)
                )

        # C cast: (type)expr
        m = re.match(r"^\(\s*[a-zA-Z_]\w*\s*\)(.+)$", expr)
        if m:
            return self.eval_expr(m.group(1).strip(), _depth + 1)

        # Logical ||
        if "||" in expr:
            parts = self._split_top(expr, "||")
            return any(bool(self.eval_expr(p, _depth + 1)) for p in parts)

        # Logical &&
        if "&&" in expr:
            parts = self._split_top(expr, "&&")
            return all(bool(self.eval_expr(p, _depth + 1)) for p in parts)

        # Comparisons
        for op in (">=", "<=", "!=", "==", ">", "<"):
            parts = self._split_top(expr, op)
            if len(parts) == 2:
                left = self.eval_expr(parts[0], _depth + 1)
                right = self.eval_expr(parts[1], _depth + 1)
                if isinstance(left, str) or isinstance(right, str):
                    return str(left) == str(right) if op == "==" else str(left) != str(right)
                li, ri = int(left), int(right)
                return {
                    ">=": li >= ri,
                    "<=": li <= ri,
                    "!=": li != ri,
                    "==": li == ri,
                    ">": li > ri,
                    "<": li < ri,
                }[op]

        # Bit shifts
        for op in ("<<", ">>"):
            parts = self._split_top(expr, op)
            if len(parts) == 2:
                return int(self.eval_expr(parts[0], _depth + 1)) << int(
                    self.eval_expr(parts[1], _depth + 1)
                ) if op == "<<" else int(self.eval_expr(parts[0], _depth + 1)) >> int(
                    self.eval_expr(parts[1], _depth + 1)
                )

        # Additive
        for op in ("+", "-"):
            parts = self._split_top(expr, op)
            if len(parts) >= 2 and op == "+":
                total = 0
                for p in parts:
                    total += int(self.eval_expr(p, _depth + 1))
                return total
            if len(parts) == 2 and op == "-":
                return int(self.eval_expr(parts[0], _depth + 1)) - int(
                    self.eval_expr(parts[1], _depth + 1)
                )

        # Multiplicative
        for op in ("*", "/"):
            parts = self._split_top(expr, op)
            if len(parts) >= 2:
                acc = int(self.eval_expr(parts[0], _depth + 1))
                for p in parts[1:]:
                    rv = int(self.eval_expr(p, _depth + 1))
                    acc = acc * rv if op == "*" else acc // rv if rv else 0
                return acc

        # sizeof(...)
        m = re.fullmatch(r"sizeof\s*\(\s*([^)]+)\s*\)", expr)
        if m:
            key = f"sizeof({m.group(1).strip()})"
            if key in self.defines:
                return int(self.eval_expr(self.defines[key], _depth + 1))
            return int(self.eval_expr(self.defines.get(key, "8"), _depth + 1))

        # Bare macro
        if re.fullmatch(r"\w+", expr):
            if expr in self.defines:
                return self.eval_expr(self.defines[expr], _depth + 1)
            raise KeyError(expr)

        raise RuntimeError(f"cannot evaluate: {expr!r}")

    @staticmethod
    def _balanced_parens(s: str) -> bool:
        depth = 0
        for ch in s:
            if ch == "(":
                depth += 1
            elif ch == ")":
                depth -= 1
                if depth < 0:
                    return False
        return depth == 0

    @staticmethod
    def _find_top_op(expr: str, op: str) -> int:
        depth = 0
        i = 0
        n = len(expr)
        while i < n:
            ch = expr[i]
            if ch in "()":
                depth += 1 if ch == "(" else -1
                i += 1
                continue
            if depth == 0 and expr.startswith(op, i):
                return i
            i += 1
        return -1

    @staticmethod
    def _split_top(expr: str, op: str) -> list[str]:
        parts: list[str] = []
        depth = 0
        i = 0
        start = 0
        while i < len(expr):
            ch = expr[i]
            if ch in "()":
                depth += 1 if ch == "(" else -1
                i += 1
                continue
            if depth == 0 and expr.startswith(op, i):
                parts.append(expr[start:i].strip())
                i += len(op)
                start = i
                continue
            i += 1
        parts.append(expr[start:].strip())
        return parts if len(parts) > 1 else [expr]

    def eval_cond(self, expr: str) -> bool:
        try:
            v = self.eval_expr(expr)
        except (KeyError, RuntimeError):
            return False
        if isinstance(v, bool):
            return v
        if isinstance(v, int):
            return v != 0
        if isinstance(v, str):
            return v not in ("0", '""', "")
        return bool(v)

    def process_file(self, path: Path, *, ifndef_only: bool = False) -> None:
        path = path.resolve()
        if path in self._processing:
            return
        self._processing.add(path)
        text = strip_comments(path.read_text())
        raw_lines = text.splitlines()
        lines = [ln.strip() for ln in raw_lines]
        i = 0
        stack: list[bool] = [True]

        while i < len(lines):
            line, i = join_lines(lines, i)
            if not line or line.startswith("#pragma"):
                continue

            active = all(stack)

            if line.startswith("#include"):
                if "<mpconfigport.h>" in line:
                    self.process_file(ROOT / "ports" / "unix" / "mpconfigport.h", ifndef_only=False)
                elif '"mpconfigvariant.h"' in line:
                    self.process_file(UNIX_VARIANT_DIR / "mpconfigvariant.h", ifndef_only=False)
                elif "mpconfigvariant_common.h" in line:
                    self.process_file(UNIX_COMMON, ifndef_only=False)
                continue

            if line.startswith("#ifndef "):
                name = line.split()[1]
                cond = name not in self.defined
                stack.append(active and cond)
                continue

            if line.startswith("#ifdef "):
                name = line.split()[1]
                stack.append(active and self.is_defined(name))
                continue

            if line.startswith("#if "):
                expr = line[4:].strip()
                stack.append(active and self.eval_cond(expr))
                continue

            if line.startswith("#elif "):
                if not stack:
                    continue
                expr = line[6:].strip()
                prev = stack.pop()
                # If a previous branch was taken, disable remaining elif.
                if prev:
                    stack.append(False)
                else:
                    parent_active = all(stack)
                    stack.append(parent_active and self.eval_cond(expr))
                continue

            if line.startswith("#else"):
                if stack:
                    prev = stack.pop()
                    stack.append(not prev and all(stack))
                continue

            if line.startswith("#endif"):
                if stack:
                    stack.pop()
                continue

            if line.startswith("#undef "):
                if active:
                    name = line.split()[1]
                    self.defined.discard(name)
                    self.defines.pop(name, None)
                continue

            if line.startswith("#define ") and active:
                rest = line[8:].strip()
                if "(" in rest.split()[0]:
                    continue  # function-like macro
                parts = rest.split(None, 1)
                name = parts[0]
                value = parts[1].strip() if len(parts) > 1 else "1"
                if ifndef_only and name in self.defined:
                    continue
                self.set_def(name, value)

        self._processing.discard(path)

    @staticmethod
    def _tokenize_string_expr(expr: str) -> list[str]:
        tokens: list[str] = []
        i = 0
        n = len(expr)
        while i < n:
            if expr[i].isspace():
                i += 1
                continue
            if expr[i] == '"':
                j = i + 1
                while j < n and expr[j] != '"':
                    j += 1
                tokens.append(expr[i : j + 1])
                i = j + 1
                continue
            if expr[i].isalpha() or expr[i] == "_":
                j = i + 1
                while j < n and (expr[j].isalnum() or expr[j] == "_"):
                    j += 1
                tokens.append(expr[i:j])
                i = j
                continue
            i += 1
        return tokens

    def eval_string(self, expr: str) -> str | None:
        expr = expr.strip()
        if expr.startswith('"') and expr.endswith('"') and expr.count('"') == 2:
            return expr
        tokens = self._tokenize_string_expr(expr)
        if not tokens:
            return None
        parts: list[str] = []
        for tok in tokens:
            if tok.startswith('"'):
                parts.append(tok[1:-1])
                continue
            if tok not in self.defines:
                return None
            try:
                val = self.eval_expr(self.defines[tok])
            except (KeyError, RuntimeError, ValueError):
                return None
            if isinstance(val, str) and val.startswith('"'):
                parts.append(val[1:-1])
            else:
                return None
        return '"' + "".join(parts) + '"'

    def resolve_all(self) -> dict[str, Any]:
        resolved: dict[str, Any] = {}
        for name, raw in sorted(self.defines.items()):
            if not (name.startswith("MICROPY_") or name.startswith("MP_")):
                continue
            if '"' in raw:
                sval = self.eval_string(raw)
                if sval is not None:
                    resolved[name] = sval
                    continue
            try:
                resolved[name] = self.eval_expr(raw)
            except (KeyError, RuntimeError, ValueError):
                resolved[name] = raw
        return resolved


NUMERIC_NAME_RE = re.compile(
    r"(_SIZE|_INIT|_INC|_MAX|_MIN|_COUNT|_LEN|_BITS|_FRAC|_EXP|_OFFSET|"
    r"_CODE_PAGE|_BACKLOG|_LEVEL|_REPR|_IMPL|_TYPE|_THRESH|_CHUNK|_DICT|"
    r"_STACK|_HISTORY|_COLUMNS|_MSBIT|BYTES_PER|BITS_PER|ROM_LEVEL)"
)


def is_numeric_config(c_name: str) -> bool:
    if c_name in {
        "MICROPY_CONFIG_ROM_LEVEL",
        "MICROPY_OBJ_REPR",
        "MICROPY_OBJ_REPR_A",
        "MICROPY_OBJ_REPR_B",
        "MICROPY_OBJ_REPR_C",
        "MICROPY_OBJ_REPR_D",
        "MICROPY_FLOAT_IMPL",
        "MICROPY_LONGINT_IMPL",
        "MICROPY_ERROR_REPORTING",
        "MICROPY_TIMESTAMP_IMPL",
        "MICROPY_FATFS_RPATH",
        "MICROPY_FATFS_LFN_CODE_PAGE",
        "MICROPY_PY_BUILTINS_CODE",
        "MICROPY_PY_TIME_TICKS_PERIOD",
        "MICROPY_PY_SOCKET_LISTEN_BACKLOG_DEFAULT",
        "MICROPY_OBJ_WORD_MSBIT_HIGH",
        "MP_OBJ_WORD_MSBIT_HIGH",
        "MP_SMALL_INT_POSITIVE_MASK",
    }:
        return True
    return bool(NUMERIC_NAME_RE.search(c_name))


def rust_name(c_name: str) -> str:
    if c_name.startswith("MICROPY_"):
        return c_name[len("MICROPY_") :]
    if c_name.startswith("MP_"):
        return c_name[len("MP_") :]
    return c_name


def rust_type(value: Any, c_name: str) -> str:
    if isinstance(value, bool):
        return "bool"
    if isinstance(value, str) and value.startswith('"'):
        return "&'static str"
    if isinstance(value, int):
        if value in (0, 1) and not is_numeric_config(c_name):
            return "bool"
        if "ROM_LEVEL" in c_name and (
            c_name.endswith(
                (
                    "MINIMUM",
                    "CORE_FEATURES",
                    "BASIC_FEATURES",
                    "EXTRA_FEATURES",
                    "FULL_FEATURES",
                    "EVERYTHING",
                )
            )
            or c_name == "MICROPY_CONFIG_ROM_LEVEL"
        ):
            return "u32"
        if c_name in {"MICROPY_ALLOC_PATH_MAX", "MICROPY_EMERGENCY_EXCEPTION_BUF_SIZE"}:
            return "usize"
        if c_name in {
            "MP_OBJ_WORD_MSBIT_HIGH",
            "MP_SMALL_INT_POSITIVE_MASK",
            "MICROPY_PY_TIME_TICKS_PERIOD",
            "MP_UINT_MAX",
            "MP_INT_MAX",
        }:
            return "u64"
        if value < 0:
            return "i64"
        if value <= 255:
            return "u8"
        if value <= 65535:
            return "u16"
        if value <= 4294967295:
            return "u32"
        return "u64"
    return "bool"


def rust_value(value: Any, rs_type: str) -> str:
    if rs_type == "bool":
        if isinstance(value, bool):
            return "true" if value else "false"
        if isinstance(value, int):
            return "true" if value else "false"
    if isinstance(value, bool):
        return "true" if value else "false"
    if isinstance(value, str):
        if value.startswith('"'):
            inner = value[1:-1]
            return f'"{inner}"'
        return f'"{value}"'
    if rs_type == "usize":
        return str(value)
    if rs_type == "u8":
        return str(value)
    if rs_type == "i64":
        return str(value)
    return str(value)


def should_emit(c_name: str, value: Any) -> bool:
    if c_name in MANUAL_NAMES or c_name in SKIP_EXACT:
        return False
    if not (c_name.startswith("MICROPY_") or c_name.startswith("MP_")):
        return False
    if isinstance(value, str):
        if value.startswith('"'):
            return True
        raw = value.strip()
        for pat in SKIP_PATTERNS:
            if pat.search(raw):
                return False
        if re.fullmatch(r"[a-z_][a-z0-9_]*", raw):
            return False
        if raw.startswith("(") and any(
            tok in raw for tok in ("do {", "while (0)", "__attribute__", "f)", "p)", "?", ":")
        ):
            return False
        # Unresolved preprocessor expression.
        return False
    return True


def enrich_derived(resolved: dict[str, Any]) -> None:
    """Fill derived MP_* constants not stored as simple #ifndef defaults."""
    bpw = int(resolved.get("MP_BYTES_PER_OBJ_WORD", 8))
    bits = int(resolved.get("MP_BITS_PER_BYTE", 8))
    word_bits = bpw * bits
    word_max = (1 << word_bits) - 1
    msbit = 1 << (word_bits - 1)
    resolved["MP_OBJ_WORD_MSBIT_HIGH"] = msbit
    obj_repr = int(resolved.get("MICROPY_OBJ_REPR", 0))
    if obj_repr == int(resolved.get("MICROPY_OBJ_REPR_A", 0)):
        mask = (~(msbit | (msbit >> 1))) & word_max
    elif obj_repr == int(resolved.get("MICROPY_OBJ_REPR_B", 1)):
        mask = (~(msbit | (msbit >> 1) | (msbit >> 2))) & word_max
    else:
        nan_mask = 0xFFFF_8000_0000_0000
        mask = (~(nan_mask | (nan_mask >> 1))) & word_max
    resolved["MP_SMALL_INT_POSITIVE_MASK"] = mask
    resolved["MICROPY_PY_TIME_TICKS_PERIOD"] = (mask + 1) & word_max


def section_for(name: str) -> str:
    # Rough grouping mirroring mpconfig.h sections.
    if name.startswith("OBJ_REPR") or name.startswith("OBJ_IMMEDIATE"):
        return "Object representation"
    if "ROM_LEVEL" in name:
        return "ROM level"
    if name.startswith("ALLOC_") or "GC_" in name or "MALLOC" in name or "MEM_" in name:
        return "Memory allocation"
    if name.startswith("EMIT_") or "PERSISTENT_CODE" in name or "NATIVE" in name:
        return "Emitters / native code"
    if name.startswith("COMP_") or name.startswith("ENABLE_COMPILER"):
        return "Compiler"
    if name.startswith("DEBUG_") or name.startswith("MEM_STATS"):
        return "Debugging"
    if name.startswith("OPT_"):
        return "Optimisations"
    if name.startswith("PY_") or name.startswith("MODULE_"):
        return "Python features"
    if name.startswith("FLOAT") or name.startswith("LONGINT"):
        return "Numeric types"
    if name.startswith("NLR") or name.startswith("STACK"):
        return "Runtime / VM"
    if name.startswith("READER_") or name.startswith("VFS") or name.startswith("STREAM"):
        return "I/O"
    if name.startswith("QSTR"):
        return "Qstr"
    if name.startswith("BYTES_PER") or name.startswith("BITS_PER") or name.startswith("ENDIANNESS"):
        return "Platform word / endianness"
    return "Miscellaneous"


def emit_mpconfig_rs(resolved: dict[str, Any]) -> str:
    manual_header = '''//! rewrite of py/mpconfig.h
//! MetalPython host defaults (unix standard variant). Port overrides live in ports_rs/*/mpconfigport.rs.
//! C macro mapping: `MICROPY_FOO` → `mpconfig::FOO`, `MP_BAR` → `mpconfig::BAR`.
// symmetry: done

/// MicroPython-compatible version numbers (reference tree is 1.29.x).
pub const VERSION_MAJOR: u32 = 1;
pub const VERSION_MINOR: u32 = 29;
pub const VERSION_MICRO: u32 = 0;
pub const VERSION_PRERELEASE: bool = true;

pub const fn make_version(major: u32, minor: u32, patch: u32) -> u32 {
    (major << 16) | (minor << 8) | patch
}

pub const VERSION: u32 = make_version(VERSION_MAJOR, VERSION_MINOR, VERSION_MICRO);

pub const VERSION_STRING: &str = if VERSION_PRERELEASE {
    "1.29.0-preview"
} else {
    "1.29.0"
};

/// Brand string for banners / `sys.implementation.name` (MetalPython, not CPython).
pub const IMPLEMENTATION_NAME: &str = "metalpython";

/// GC heap size for the host smoke path (bytes). Grown later per-port.
pub const GC_HEAP_SIZE: usize = 256 * 1024;

/// Derived from `MICROPY_OBJ_REPR != MICROPY_OBJ_REPR_D` (computed after OBJ_REPR is set).
'''

    items: list[tuple[str, str, str, str]] = []
    for c_name, value in sorted(resolved.items()):
        if not should_emit(c_name, value):
            continue
        rs_name = rust_name(c_name)
        rs_type = rust_type(value, c_name)
        rs_val = rust_value(value, rs_type)
        items.append((section_for(rs_name), rs_name, rs_type, rs_val))

    # Insert OBJ_IMMEDIATE_OBJS derived const after OBJ_REPR if present.
    sections: dict[str, list[tuple[str, str, str]]] = {}
    for sec, name, typ, val in items:
        if name == "OBJ_IMMEDIATE_OBJS":
            continue
        sections.setdefault(sec, []).append((name, typ, val))

    out: list[str] = [manual_header]
    out.append("pub const OBJ_IMMEDIATE_OBJS: bool = OBJ_REPR != OBJ_REPR_D;")
    out.append("")

    order = [
        "ROM level",
        "Object representation",
        "Platform word / endianness",
        "Memory allocation",
        "Qstr",
        "Emitters / native code",
        "Compiler",
        "Debugging",
        "Optimisations",
        "Runtime / VM",
        "I/O",
        "Numeric types",
        "Python features",
        "Miscellaneous",
    ]
    seen: set[str] = set()
    for sec in order + [s for s in sections if s not in order]:
        if sec not in sections or sec in seen:
            continue
        seen.add(sec)
        out.append(f"// --- {sec} ---")
        for name, typ, val in sections[sec]:
            out.append(f"pub const {name}: {typ} = {val};")
        out.append("")

    return "\n".join(out).rstrip() + "\n"


def main() -> int:
    pp = Preprocessor()
    for path in UNIX_OVERRIDES:
        if not path.exists():
            print(f"missing override: {path}", file=sys.stderr)
            return 1
    # Single pass: mpconfig.h pulls unix overrides at `#include <mpconfigport.h>`.
    pp.process_file(MPCONFIG_H, ifndef_only=False)

    resolved = pp.resolve_all()
    enrich_derived(resolved)
    OUT_RS.write_text(emit_mpconfig_rs(resolved))
    emit_count = sum(1 for k, v in resolved.items() if should_emit(k, v))
    print(f"Wrote {OUT_RS} ({OUT_RS.stat().st_size} bytes, {emit_count} consts)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
