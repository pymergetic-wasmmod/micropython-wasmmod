"""Shared constants for the symmetry checker."""

from __future__ import annotations

STATUS_ORDER = ("done", "partial", "stub", "stale", "missing", "ignored")

SKIP_DIR_NAMES = {
    "mbedtls",
    "btstack",
    "nimble",
    "berkeley-db",
    "libmetal",
    "variants",
    "boards",
    "test-frzmpy",
}
SKIP_DIR_SUFFIXES = ("-include",)
SKIP_FILE_GLOBS = (
    "Makefile",
    "README*",
    "Dockerfile*",
    "*.mk",
    "*.cmake",
    "*.ld",
    "*.md",
    "make*.py",
    "make_*.py",
)

PM_SEARCH = ("py_rs/pm", "extmod_rs/pm", "include/metalpython")

DEFAULT_INFRA = (
    "pm_mpy_runtime_init",
    "pm_mpy_runtime_deinit",
    "pm_mpy_runtime_call",
    "pm_mpy_runtime_call_n_kw",
    "pm_mpy_runtime_eval",
    "pm_mpy_runtime_exec",
    "pm_mpy_runtime_execfile",
    "pm_mpy_module_new",
    "pm_mpy_module_register",
    "pm_mpy_module_get",
    "pm_mpy_module_set_attr",
    "pm_mpy_module_get_attr",
    "pm_mpy_module_globals",
    "pm_mpy_module_from_map",
    "pm_mpy_import_import_module",
    "pm_mpy_import___import__",
    "pm_mpy_import_stat",
    "pm_mpy_lookup",
    "pm_mpy_obj_new_int",
    "pm_mpy_obj_new_str",
    "pm_mpy_obj_new_bytes",
    "pm_mpy_obj_new_bool",
    "pm_mpy_obj_new_list",
    "pm_mpy_obj_new_dict",
    "pm_mpy_obj_new_tuple",
    "pm_mpy_obj_getattr",
    "pm_mpy_obj_setattr",
    "pm_mpy_qstr_from_str",
    "pm_mpy_exc_raise",
    "pm_mpy_exc_raise_type_msg",
)

# Weighted progress for conversion tracking.
STATUS_WEIGHT = {
    "done": 1.0,
    "partial": 0.5,
    "stub": 0.25,
    "stale": 0.1,
    "missing": 0.0,
    "ignored": 0.0,
    "present": 1.0,  # pm_*
}
