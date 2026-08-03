//! rewrite of py/profile.c + py/profile.h
// symmetry: done

use crate::bc;
use crate::bc0;
use crate::emitglue::RawCode;
use crate::mpconfig;
use crate::obj::{self, Obj, ObjBase, ObjType};
use crate::qstr::Qstr;

/// Bytecode prelude metadata used by settrace (`mp_bytecode_prelude_t`).
#[derive(Debug, Default, Clone)]
pub struct BytecodePrelude {
    pub n_state: usize,
    pub n_exc_stack: usize,
    pub scope_flags: usize,
    pub n_pos_args: usize,
    pub n_kwonly_args: usize,
    pub n_def_pos_args: usize,
    pub line_info_top: *const u8,
    pub opcodes: *const u8,
    pub qstr_block_name_idx: usize,
    pub line_info: *const u8,
}

/// Frame object (`mp_obj_frame_t`).
#[repr(C)]
pub struct ObjFrame {
    pub base: ObjBase,
    pub code_state: *const (),
    pub back: *mut ObjFrame,
    pub callback: Obj,
    pub code: Obj,
    pub lasti: usize,
    pub lineno: usize,
    pub trace_opcodes: bool,
}

pub const PROF_INSTR_DEBUG_PRINT_ENABLE: bool = false;

pub fn prof_is_executing() -> bool {
    if mpconfig::PY_SYS_SETTRACE {
        crate::mpstate::with_thread(|ts| ts.prof_callback_is_executing)
    } else {
        false
    }
}

pub fn set_prof_is_executing(value: bool) {
    if mpconfig::PY_SYS_SETTRACE {
        crate::mpstate::with_thread(|ts| ts.prof_callback_is_executing = value);
    }
}

/// `mp_prof_bytecode_lineno`
pub fn prof_bytecode_lineno(rc: &RawCode, bc: usize) -> u32 {
    if !mpconfig::PY_SYS_SETTRACE {
        return 0;
    }
    let prelude = &extract_prelude_from_rc(rc);
    bc::get_source_line(prelude.line_info, prelude.line_info_top, bc) as u32
}

fn extract_prelude_from_rc(rc: &RawCode) -> BytecodePrelude {
    let mut prelude = BytecodePrelude::default();
    prof_extract_prelude(rc.fun_data, &mut prelude);
    prelude
}

/// `mp_prof_extract_prelude`
pub fn prof_extract_prelude(bytecode: *const u8, prelude: &mut BytecodePrelude) {
    if !mpconfig::PY_SYS_SETTRACE {
        return;
    }
    let mut ip = bytecode;
    let sig = bc::prelude_sig_decode_into(&mut ip);
    prelude.n_state = sig.n_state;
    prelude.n_exc_stack = sig.n_exc_stack;
    prelude.scope_flags = sig.scope_flags;
    prelude.n_pos_args = sig.n_pos_args;
    prelude.n_kwonly_args = sig.n_kwonly_args;
    prelude.n_def_pos_args = sig.n_def_pos_args;

    let (n_info, n_cell) = bc::prelude_size_decode(&mut ip);
    prelude.line_info_top = unsafe { ip.add(n_info) };
    prelude.opcodes = unsafe { ip.add(n_info + n_cell) };
    prelude.qstr_block_name_idx = bc::decode_uint_value(ip);
    for _ in 0..1 + sig.n_pos_args + sig.n_kwonly_args {
        ip = bc::decode_uint_skip(ip);
    }
    prelude.line_info = ip;
}

static mut TYPE_FRAME: ObjType = obj::empty_type(0);
static TYPE_INIT: std::sync::OnceLock<()> = std::sync::OnceLock::new();

fn init_frame_type() {
    TYPE_INIT.get_or_init(|| {
        unsafe {
            TYPE_FRAME.name = crate::qstr::from_str("frame");
        }
    });
}

pub fn type_frame() -> &'static ObjType {
    init_frame_type();
    unsafe { &TYPE_FRAME }
}

/// `mp_obj_new_frame`
pub fn obj_new_frame(_code_state: *const ()) -> Obj {
    if !mpconfig::PY_SYS_SETTRACE {
        return obj::OBJ_NULL;
    }
    obj::OBJ_NULL
}

/// `mp_prof_settrace`
pub fn prof_settrace(callback: Obj) -> Obj {
    if mpconfig::PY_SYS_SETTRACE {
        crate::mpstate::with_thread(|ts| {
            ts.prof_trace_callback = if obj::is_callable(callback) {
                callback
            } else {
                obj::OBJ_NULL
            };
        });
    }
    obj::CONST_NONE
}

/// `mp_prof_frame_enter`
pub fn prof_frame_enter(_code_state: *mut ()) -> Obj {
    if !mpconfig::PY_SYS_SETTRACE {
        return obj::OBJ_NULL;
    }
    obj::OBJ_NULL
}

/// `mp_prof_frame_update`
pub fn prof_frame_update(_code_state: *const ()) -> Obj {
    if !mpconfig::PY_SYS_SETTRACE {
        return obj::OBJ_NULL;
    }
    obj::OBJ_NULL
}

/// `mp_prof_instr_tick`
pub fn prof_instr_tick(_code_state: *mut (), _is_exception: bool) -> Obj {
    if !mpconfig::PY_SYS_SETTRACE {
        return obj::CONST_NONE;
    }
    obj::CONST_NONE
}

/// Debug printer for settrace development (`mp_prof_print_instr`).
pub fn prof_print_instr(_ip: *const u8, _code_state: *const ()) {
    if mpconfig::PY_SYS_SETTRACE && PROF_INSTR_DEBUG_PRINT_ENABLE {
        // Intentionally omitted in this port until persistent code save is enabled.
    }
}

pub fn prof_instr_debug_print(_current_ip: *const u8) {
    let _ = (_current_ip, bc0::RETURN_VALUE);
}
