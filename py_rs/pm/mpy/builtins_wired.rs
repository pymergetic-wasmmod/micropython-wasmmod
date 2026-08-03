//! Wired `pm_mpy_builtins_*` accessors.
// symmetry: done

use super::builtins::builtins_export;
use super::types::pm_mpy_obj_t;

/// `pm_mpy_builtins___build_class__` — return the `__build_class__` export from `builtins`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins___build_class__() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("__build_class__"))
}

/// `pm_mpy_builtins___import__` — return the `__import__` export from `builtins`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins___import__() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("__import__"))
}

/// `pm_mpy_builtins___repl_print__` — return the `__repl_print__` export from `builtins`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins___repl_print__() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("__repl_print__"))
}

/// `pm_mpy_builtins_bool` — return the `bool` export from `builtins`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins_bool() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("bool"))
}

/// `pm_mpy_builtins_bytes` — return the `bytes` export from `builtins`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins_bytes() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("bytes"))
}

/// `pm_mpy_builtins_bytearray` — return the `bytearray` export from `builtins`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins_bytearray() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("bytearray"))
}

/// `pm_mpy_builtins_complex` — return the `complex` export from `builtins`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins_complex() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("complex"))
}

/// `pm_mpy_builtins_dict` — return the `dict` export from `builtins`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins_dict() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("dict"))
}

/// `pm_mpy_builtins_enumerate` — return the `enumerate` export from `builtins`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins_enumerate() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("enumerate"))
}

/// `pm_mpy_builtins_filter` — return the `filter` export from `builtins`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins_filter() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("filter"))
}

/// `pm_mpy_builtins_float` — return the `float` export from `builtins`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins_float() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("float"))
}

/// `pm_mpy_builtins_frozenset` — return the `frozenset` export from `builtins`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins_frozenset() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("frozenset"))
}

/// `pm_mpy_builtins_int` — return the `int` export from `builtins`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins_int() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("int"))
}

/// `pm_mpy_builtins_list` — return the `list` export from `builtins`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins_list() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("list"))
}

/// `pm_mpy_builtins_map` — return the `map` export from `builtins`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins_map() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("map"))
}

/// `pm_mpy_builtins_memoryview` — return the `memoryview` export from `builtins`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins_memoryview() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("memoryview"))
}

/// `pm_mpy_builtins_object` — return the `object` export from `builtins`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins_object() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("object"))
}

/// `pm_mpy_builtins_property` — return the `property` export from `builtins`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins_property() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("property"))
}

/// `pm_mpy_builtins_range` — return the `range` export from `builtins`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins_range() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("range"))
}

/// `pm_mpy_builtins_reversed` — return the `reversed` export from `builtins`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins_reversed() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("reversed"))
}

/// `pm_mpy_builtins_set` — return the `set` export from `builtins`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins_set() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("set"))
}

/// `pm_mpy_builtins_slice` — return the `slice` export from `builtins`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins_slice() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("slice"))
}

/// `pm_mpy_builtins_str` — return the `str` export from `builtins`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins_str() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("str"))
}

/// `pm_mpy_builtins_super` — return the `super` export from `builtins`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins_super() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("super"))
}

/// `pm_mpy_builtins_tuple` — return the `tuple` export from `builtins`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins_tuple() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("tuple"))
}

/// `pm_mpy_builtins_type` — return the `type` export from `builtins`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins_type() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("type"))
}

/// `pm_mpy_builtins_zip` — return the `zip` export from `builtins`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins_zip() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("zip"))
}

/// `pm_mpy_builtins_classmethod` — return the `classmethod` export from `builtins`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins_classmethod() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("classmethod"))
}

/// `pm_mpy_builtins_staticmethod` — return the `staticmethod` export from `builtins`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins_staticmethod() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("staticmethod"))
}

/// `pm_mpy_builtins_Ellipsis` — return the `Ellipsis` export from `builtins`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins_Ellipsis() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("Ellipsis"))
}

/// `pm_mpy_builtins_NotImplemented` — return the `NotImplemented` export from `builtins`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins_NotImplemented() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("NotImplemented"))
}

/// `pm_mpy_builtins_abs` — return the `abs` export from `builtins`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins_abs() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("abs"))
}

/// `pm_mpy_builtins_all` — return the `all` export from `builtins`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins_all() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("all"))
}

/// `pm_mpy_builtins_any` — return the `any` export from `builtins`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins_any() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("any"))
}

/// `pm_mpy_builtins_bin` — return the `bin` export from `builtins`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins_bin() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("bin"))
}

/// `pm_mpy_builtins_callable` — return the `callable` export from `builtins`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins_callable() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("callable"))
}

/// `pm_mpy_builtins_compile` — return the `compile` export from `builtins`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins_compile() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("compile"))
}

/// `pm_mpy_builtins_chr` — return the `chr` export from `builtins`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins_chr() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("chr"))
}

/// `pm_mpy_builtins_delattr` — return the `delattr` export from `builtins`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins_delattr() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("delattr"))
}

/// `pm_mpy_builtins_dir` — return the `dir` export from `builtins`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins_dir() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("dir"))
}

/// `pm_mpy_builtins_divmod` — return the `divmod` export from `builtins`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins_divmod() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("divmod"))
}

/// `pm_mpy_builtins_eval` — return the `eval` export from `builtins`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins_eval() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("eval"))
}

/// `pm_mpy_builtins_exec` — return the `exec` export from `builtins`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins_exec() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("exec"))
}

/// `pm_mpy_builtins_getattr` — return the `getattr` export from `builtins`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins_getattr() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("getattr"))
}

/// `pm_mpy_builtins_setattr` — return the `setattr` export from `builtins`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins_setattr() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("setattr"))
}

/// `pm_mpy_builtins_globals` — return the `globals` export from `builtins`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins_globals() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("globals"))
}

/// `pm_mpy_builtins_hasattr` — return the `hasattr` export from `builtins`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins_hasattr() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("hasattr"))
}

/// `pm_mpy_builtins_hash` — return the `hash` export from `builtins`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins_hash() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("hash"))
}

/// `pm_mpy_builtins_help` — return the `help` export from `builtins`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins_help() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("help"))
}

/// `pm_mpy_builtins_hex` — return the `hex` export from `builtins`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins_hex() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("hex"))
}

/// `pm_mpy_builtins_id` — return the `id` export from `builtins`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins_id() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("id"))
}

/// `pm_mpy_builtins_input` — return the `input` export from `builtins`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins_input() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("input"))
}

/// `pm_mpy_builtins_isinstance` — return the `isinstance` export from `builtins`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins_isinstance() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("isinstance"))
}

/// `pm_mpy_builtins_issubclass` — return the `issubclass` export from `builtins`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins_issubclass() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("issubclass"))
}

/// `pm_mpy_builtins_iter` — return the `iter` export from `builtins`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins_iter() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("iter"))
}

/// `pm_mpy_builtins_len` — return the `len` export from `builtins`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins_len() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("len"))
}

/// `pm_mpy_builtins_locals` — return the `locals` export from `builtins`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins_locals() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("locals"))
}

/// `pm_mpy_builtins_max` — return the `max` export from `builtins`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins_max() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("max"))
}

/// `pm_mpy_builtins_min` — return the `min` export from `builtins`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins_min() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("min"))
}

/// `pm_mpy_builtins_next` — return the `next` export from `builtins`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins_next() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("next"))
}

/// `pm_mpy_builtins_oct` — return the `oct` export from `builtins`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins_oct() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("oct"))
}

/// `pm_mpy_builtins_ord` — return the `ord` export from `builtins`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins_ord() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("ord"))
}

/// `pm_mpy_builtins_pow` — return the `pow` export from `builtins`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins_pow() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("pow"))
}

/// `pm_mpy_builtins_print` — return the `print` export from `builtins`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins_print() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("print"))
}

/// `pm_mpy_builtins_repr` — return the `repr` export from `builtins`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins_repr() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("repr"))
}

/// `pm_mpy_builtins_round` — return the `round` export from `builtins`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins_round() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("round"))
}

/// `pm_mpy_builtins_sorted` — return the `sorted` export from `builtins`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins_sorted() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("sorted"))
}

/// `pm_mpy_builtins_sum` — return the `sum` export from `builtins`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins_sum() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("sum"))
}

/// `pm_mpy_builtins_BaseException` — return the `BaseException` export from `builtins`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins_BaseException() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("BaseException"))
}

/// `pm_mpy_builtins_Exception` — return the `Exception` export from `builtins`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins_Exception() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("Exception"))
}

/// `pm_mpy_builtins_ImportError` — return the `ImportError` export from `builtins`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins_ImportError() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("ImportError"))
}

/// `pm_mpy_builtins_StopIteration` — return the `StopIteration` export from `builtins`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins_StopIteration() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("StopIteration"))
}

/// `pm_mpy_builtins_TypeError` — return the `TypeError` export from `builtins`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins_TypeError() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("TypeError"))
}

/// `pm_mpy_builtins_ValueError` — return the `ValueError` export from `builtins`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins_ValueError() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("ValueError"))
}

/// `pm_mpy_builtins___template__` — return the `__template__` export from `builtins`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins___template__() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("__template__"))
}

/// `pm_mpy_builtins_execfile` — return the `execfile` export from `builtins`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins_execfile() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("execfile"))
}

/// `pm_mpy_builtins_open` — return the `open` export from `builtins`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins_open() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("open"))
}

/// `pm_mpy_builtins_ArithmeticError` — return the `ArithmeticError` export from `builtins`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins_ArithmeticError() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("ArithmeticError"))
}

/// `pm_mpy_builtins_AssertionError` — return the `AssertionError` export from `builtins`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins_AssertionError() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("AssertionError"))
}

/// `pm_mpy_builtins_AttributeError` — return the `AttributeError` export from `builtins`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins_AttributeError() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("AttributeError"))
}

/// `pm_mpy_builtins_EOFError` — return the `EOFError` export from `builtins`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins_EOFError() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("EOFError"))
}

/// `pm_mpy_builtins_GeneratorExit` — return the `GeneratorExit` export from `builtins`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins_GeneratorExit() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("GeneratorExit"))
}

/// `pm_mpy_builtins_IndentationError` — return the `IndentationError` export from `builtins`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins_IndentationError() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("IndentationError"))
}

/// `pm_mpy_builtins_IndexError` — return the `IndexError` export from `builtins`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins_IndexError() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("IndexError"))
}

/// `pm_mpy_builtins_KeyboardInterrupt` — return the `KeyboardInterrupt` export from `builtins`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins_KeyboardInterrupt() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("KeyboardInterrupt"))
}

/// `pm_mpy_builtins_KeyError` — return the `KeyError` export from `builtins`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins_KeyError() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("KeyError"))
}

/// `pm_mpy_builtins_LookupError` — return the `LookupError` export from `builtins`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins_LookupError() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("LookupError"))
}

/// `pm_mpy_builtins_MemoryError` — return the `MemoryError` export from `builtins`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins_MemoryError() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("MemoryError"))
}

/// `pm_mpy_builtins_NameError` — return the `NameError` export from `builtins`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins_NameError() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("NameError"))
}

/// `pm_mpy_builtins_NotImplementedError` — return the `NotImplementedError` export from `builtins`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins_NotImplementedError() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("NotImplementedError"))
}

/// `pm_mpy_builtins_OSError` — return the `OSError` export from `builtins`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins_OSError() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("OSError"))
}

/// `pm_mpy_builtins_OverflowError` — return the `OverflowError` export from `builtins`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins_OverflowError() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("OverflowError"))
}

/// `pm_mpy_builtins_RuntimeError` — return the `RuntimeError` export from `builtins`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins_RuntimeError() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("RuntimeError"))
}

/// `pm_mpy_builtins_StopAsyncIteration` — return the `StopAsyncIteration` export from `builtins`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins_StopAsyncIteration() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("StopAsyncIteration"))
}

/// `pm_mpy_builtins_SyntaxError` — return the `SyntaxError` export from `builtins`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins_SyntaxError() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("SyntaxError"))
}

/// `pm_mpy_builtins_SystemExit` — return the `SystemExit` export from `builtins`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins_SystemExit() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("SystemExit"))
}

/// `pm_mpy_builtins_UnicodeError` — return the `UnicodeError` export from `builtins`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins_UnicodeError() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("UnicodeError"))
}

/// `pm_mpy_builtins_ViperTypeError` — return the `ViperTypeError` export from `builtins`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins_ViperTypeError() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("ViperTypeError"))
}

/// `pm_mpy_builtins_ZeroDivisionError` — return the `ZeroDivisionError` export from `builtins`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins_ZeroDivisionError() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("ZeroDivisionError"))
}
