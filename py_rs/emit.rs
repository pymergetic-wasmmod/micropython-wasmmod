//! rewrite of py/emit.h (types, constants, bytecode emitter declarations)
// symmetry: done

use crate::lexer::TokenKind;
use crate::map::Map;
use crate::obj::Obj;
use crate::qstr::Qstr;
use crate::scope::Scope;

/// Compiler pass (`pass_kind_t`).
#[repr(u8)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum PassKind {
    Scope = 1,
    StackSize = 2,
    CodeSize = 3,
    Emit = 4,
}

pub const EMIT_STAR_FLAG_SINGLE: u8 = 0x01;
pub const EMIT_STAR_FLAG_DOUBLE: u8 = 0x02;
pub const EMIT_BREAK_FROM_FOR: u16 = 0x8000;

pub const EMIT_IDOP_LOCAL_FAST: i32 = 0;
pub const EMIT_IDOP_LOCAL_DEREF: i32 = 1;

pub const EMIT_IDOP_GLOBAL_NAME: i32 = 0;
pub const EMIT_IDOP_GLOBAL_GLOBAL: i32 = 1;

pub const EMIT_IMPORT_NAME: i32 = 0;
pub const EMIT_IMPORT_FROM: i32 = 1;
pub const EMIT_IMPORT_STAR: i32 = 2;

pub const EMIT_SUBSCR_LOAD: i32 = 0;
pub const EMIT_SUBSCR_STORE: i32 = 1;
pub const EMIT_SUBSCR_DELETE: i32 = 2;

pub const EMIT_ATTR_LOAD: i32 = 0;
pub const EMIT_ATTR_STORE: i32 = 1;
pub const EMIT_ATTR_DELETE: i32 = 2;

pub const EMIT_SETUP_BLOCK_WITH: i32 = 0;
pub const EMIT_SETUP_BLOCK_EXCEPT: i32 = 1;
pub const EMIT_SETUP_BLOCK_FINALLY: i32 = 2;

pub const EMIT_BUILD_TUPLE: i32 = 0;
pub const EMIT_BUILD_LIST: i32 = 1;
pub const EMIT_BUILD_MAP: i32 = 2;
pub const EMIT_BUILD_SET: i32 = 3;
pub const EMIT_BUILD_SLICE: i32 = 4;

pub const EMIT_YIELD_VALUE: i32 = 0;
pub const EMIT_YIELD_FROM: i32 = 1;

/// Opaque bytecode emitter (`emit_t`), defined in emitbc.rs.
pub struct Emit {
    _private: [u8; 0],
}

/// Shared emitter state (`mp_emit_common_t`).
pub struct EmitCommon {
    pub pass: PassKind,
    pub ct_cur_child: usize,
    pub children: *mut *mut crate::emitglue::RawCode,
    pub qstr_map: Map,
    pub const_obj_list: Vec<Obj>,
}

#[derive(Copy, Clone)]
pub enum EmitIdOps {
    Load,
    Store,
    Delete,
}

pub fn emit_common_use_qstr(emit: &mut EmitCommon, qst: Qstr) -> Qstr {
    if crate::mpconfig::EMIT_BYTECODE_USES_QSTR_TABLE {
        use crate::map::LookupKind;
        use crate::obj;
        let used = emit.qstr_map.used;
        let elem = crate::map::lookup(&mut emit.qstr_map, obj::new_qstr(qst), LookupKind::AddIfNotFound)
            .expect("qstr map insert");
        if elem.value == obj::OBJ_NULL {
            elem.value = obj::new_small_int((used - 1) as crate::obj::Int);
        }
        obj::small_int_value(elem.value) as Qstr
    } else {
        qst
    }
}

pub fn emit_common_use_const_obj(emit: &mut EmitCommon, const_obj: Obj) -> usize {
    crate::emitcommon::use_const_obj(emit, const_obj)
}

pub fn emit_common_alloc_const_child(emit: &mut EmitCommon, rc: *mut crate::emitglue::RawCode) -> usize {
    if emit.pass == PassKind::Emit {
        unsafe {
            *emit.children.add(emit.ct_cur_child) = rc;
        }
    }
    let idx = emit.ct_cur_child;
    emit.ct_cur_child += 1;
    idx
}

pub fn emit_common_get_id_for_load(scope: &mut Scope, qst: Qstr) {
    crate::scope::find_or_add_id(scope, qst, crate::scope::IdInfoKind::GlobalImplicit);
}

pub fn emit_common_get_id_for_modification(scope: &mut Scope, qst: Qstr) {
    let scope_kind = scope.kind;
    let id = crate::scope::find_or_add_id(scope, qst, crate::scope::IdInfoKind::GlobalImplicit);
    if id.kind == crate::scope::IdInfoKind::GlobalImplicit {
        id.kind = if crate::scope::scope_is_func_like(scope_kind) {
            crate::scope::IdInfoKind::Local
        } else {
            crate::scope::IdInfoKind::GlobalImplicitAssigned
        };
    }
}

pub fn emit_common_id_op(emit: *mut Emit, ops: EmitIdOps, scope: &Scope, qst: Qstr) {
    crate::emitcommon::id_op(emit, ops, scope, qst);
}

// Re-export bytecode emitter entry points (mp_emit_bc_*).
pub use crate::emitbc::{
    adjust_stack_size as emit_bc_adjust_stack_size,
    attr as emit_bc_attr,
    binary_op as emit_bc_binary_op,
    build as emit_bc_build,
    call_function as emit_bc_call_function,
    call_method as emit_bc_call_method,
    delete_global as emit_bc_delete_global,
    delete_local as emit_bc_delete_local,
    dup_top as emit_bc_dup_top,
    dup_top_two as emit_bc_dup_top_two,
    end_except_handler as emit_bc_end_except_handler,
    end_finally as emit_bc_end_finally,
    for_iter as emit_bc_for_iter,
    for_iter_end as emit_bc_for_iter_end,
    free as emit_bc_free,
    get_iter as emit_bc_get_iter,
    import as emit_bc_import,
    jump as emit_bc_jump,
    jump_if_or_pop as emit_bc_jump_if_or_pop,
    label_assign as emit_bc_label_assign,
    load_build_class as emit_bc_load_build_class,
    load_const_obj as emit_bc_load_const_obj,
    load_const_small_int as emit_bc_load_const_small_int,
    load_const_str as emit_bc_load_const_str,
    load_const_tok as emit_bc_load_const_tok,
    load_global as emit_bc_load_global,
    load_local as emit_bc_load_local,
    load_method as emit_bc_load_method,
    load_null as emit_bc_load_null,
    make_closure as emit_bc_make_closure,
    make_function as emit_bc_make_function,
    new as emit_bc_new,
    pop_except_jump as emit_bc_pop_except_jump,
    pop_jump_if as emit_bc_pop_jump_if,
    pop_top as emit_bc_pop_top,
    raise_varargs as emit_bc_raise_varargs,
    return_value as emit_bc_return_value,
    rot_three as emit_bc_rot_three,
    rot_two as emit_bc_rot_two,
    set_max_num_labels as emit_bc_set_max_num_labels,
    set_source_line as emit_bc_set_source_line,
    setup_block as emit_bc_setup_block,
    start_except_handler as emit_bc_start_except_handler,
    start_pass as emit_bc_start_pass,
    end_pass as emit_bc_end_pass,
    store_comp as emit_bc_store_comp,
    store_global as emit_bc_store_global,
    store_local as emit_bc_store_local,
    store_map as emit_bc_store_map,
    subscr as emit_bc_subscr,
    unary_op as emit_bc_unary_op,
    unpack_ex as emit_bc_unpack_ex,
    unpack_sequence as emit_bc_unpack_sequence,
    unwind_jump as emit_bc_unwind_jump,
    with_cleanup as emit_bc_with_cleanup,
    yield_ as emit_bc_yield,
};
