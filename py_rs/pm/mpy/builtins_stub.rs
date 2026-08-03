//! Stub `pm_mpy_builtins_*` exports wired through the Rust builtins table.
// symmetry: done

use super::builtins::builtins_export;
use super::types::pm_mpy_obj_t;
use crate::obj;

#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins___template__() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("__template__"))
}

#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins_execfile() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("execfile"))
}

#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins_open() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("open"))
}

#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins_ArithmeticError() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("ArithmeticError"))
}

#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins_AssertionError() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("AssertionError"))
}

#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins_AttributeError() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("AttributeError"))
}

#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins_EOFError() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("EOFError"))
}

#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins_GeneratorExit() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("GeneratorExit"))
}

#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins_IndentationError() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("IndentationError"))
}

#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins_IndexError() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("IndexError"))
}

#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins_KeyboardInterrupt() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("KeyboardInterrupt"))
}

#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins_KeyError() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("KeyError"))
}

#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins_LookupError() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("LookupError"))
}

#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins_MemoryError() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("MemoryError"))
}

#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins_NameError() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("NameError"))
}

#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins_NotImplementedError() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("NotImplementedError"))
}

#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins_OSError() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("OSError"))
}

#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins_OverflowError() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("OverflowError"))
}

#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins_RuntimeError() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("RuntimeError"))
}

#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins_StopAsyncIteration() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("StopAsyncIteration"))
}

#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins_SyntaxError() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("SyntaxError"))
}

#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins_SystemExit() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("SystemExit"))
}

#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins_UnicodeError() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("UnicodeError"))
}

#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins_ViperTypeError() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("ViperTypeError"))
}

#[no_mangle]
pub unsafe extern "C" fn pm_mpy_builtins_ZeroDivisionError() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(builtins_export("ZeroDivisionError"))
}
