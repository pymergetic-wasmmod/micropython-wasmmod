//! rewrite of py/dynruntime.h
// symmetry: done

//! Dynamic native module runtime. C uses macros overriding the static API;
//! Rust exposes helpers and re-exports the native function table.

use crate::mpconfig;
use crate::nativeglue;
use crate::obj::Obj;

pub fn mp_fun_table() -> *const usize {
    nativeglue::fun_table_reloc_entries()
}

pub fn enabled() -> bool {
    mpconfig::ENABLE_NATIVE_CODE
}

/// `mp_obj_len_dyn` — call built-in len().
pub fn obj_len(o: Obj) -> Obj {
    crate::runtime::call_function_1(crate::runtime::load_name(crate::qstr::from_str("len")), o)
}

/// `mp_obj_get_array_dyn` for tuple/list.
pub fn obj_get_array(o: Obj) -> (usize, Vec<Obj>) {
    crate::obj::get_array(o)
}
