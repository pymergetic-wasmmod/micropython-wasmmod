//! Shared native emitter implementation (body of py/emitnative.c).
// symmetry: host unix+x64 generator throw/yield_from delegate exceptions return through VmReturnKind
#![allow(clippy::all, non_snake_case)]

use super::*;
use core::mem::size_of;
use crate::malloc;
use crate::objexcept;
use crate::objstr;
use crate::qstr;

fn need_fun_obj<B: NativeBackend>(emit: &EmitNative<B>) -> bool {
    unsafe {
        (*emit.scope).exc_stack_size > 0
            || ((*emit.scope).scope_flags & (MP_SCOPE_FLAG_REFGLOBALS | MP_SCOPE_FLAG_HASCONSTS)) != 0
    }
}

fn need_global_exc_handler<B: NativeBackend>(emit: &EmitNative<B>) -> bool {
    unsafe {
        (*emit.scope).exc_stack_size > 0
            || ((*emit.scope).scope_flags & (MP_SCOPE_FLAG_GENERATOR | MP_SCOPE_FLAG_REFGLOBALS)) != 0
    }
}

fn need_exc_handler_unwind<B: NativeBackend>(emit: &EmitNative<B>) -> bool {
    unsafe { (*emit.scope).exc_stack_size > 0 }
}

fn need_throw_val<B: NativeBackend>(emit: &EmitNative<B>) -> bool {
    unsafe { (*emit.scope).scope_flags & MP_SCOPE_FLAG_GENERATOR != 0 }
}

fn need_gen_return_obj<B: NativeBackend>(emit: &EmitNative<B>) -> bool {
    unsafe {
        ((*emit.scope).scope_flags & MP_SCOPE_FLAG_GENERATOR != 0)
            && (*emit.scope).exc_stack_size > 0
    }
}

fn generator_n_state<B: NativeBackend>(emit: &EmitNative<B>) -> i32 {
    let scope = unsafe { &*emit.scope };
    scope.num_locals as i32 + scope.stack_size as i32
}

fn generator_return_x_slot_if_active<B: NativeBackend>(emit: &EmitNative<B>) -> Option<i32> {
    let base = emit
        .exc_stack
        .iter()
        .rev()
        .find(|entry| entry.is_finally && entry.is_active)
        .map(|entry| entry.finally_sp_index as i32)?;
    Some(emit.stack_start as i32 + base + 1)
}

fn can_use_regs_for_locals<B: NativeBackend>(emit: &EmitNative<B>) -> bool {
    unsafe {
        (*emit.scope).exc_stack_size == 0 && ((*emit.scope).scope_flags & MP_SCOPE_FLAG_GENERATOR) == 0
    }
}

fn local_idx_exc_val<B: NativeBackend>(_emit: &EmitNative<B>) -> i32 {
    NLR_BUF_IDX_RET_VAL as i32
}
fn local_idx_exc_handler_pc<B: NativeBackend>(_emit: &EmitNative<B>) -> i32 {
    B::NLR_BUF_IDX_LOCAL_1 as i32
}
fn local_idx_exc_handler_unwind<B: NativeBackend>(_emit: &EmitNative<B>) -> i32 {
    (SIZEOF_NLR_BUF + 1) as i32
}
fn local_idx_throw_val<B: NativeBackend>(_emit: &EmitNative<B>) -> i32 {
    (SIZEOF_NLR_BUF + 2) as i32
}

fn vtype_name(vtype: VType) -> &'static [u8] {
    match vtype {
        VType::PyObj => b"object",
        VType::Bool => b"bool",
        VType::Int => b"int",
        VType::Uint => b"uint",
        VType::Ptr => b"ptr",
        VType::Ptr8 => b"ptr8",
        VType::Ptr16 => b"ptr16",
        VType::Ptr32 => b"ptr32",
        _ => b"None",
    }
}

fn set_emit_compile_error<B: NativeBackend>(emit: &mut EmitNative<B>, exc: Obj) {
    unsafe {
        *emit.error_slot = exc;
    }
}

fn emit_not_implemented_error<B: NativeBackend>(emit: &mut EmitNative<B>, msg: &[u8]) {
    set_emit_compile_error(
        emit,
        objexcept::new_exception_args(objexcept::type_not_implemented_error(), 1, &[objstr::new_str(msg)]),
    );
}

fn viper_type_error_msg<B: NativeBackend>(emit: &mut EmitNative<B>, msg: &[u8]) {
    set_emit_compile_error(
        emit,
        objexcept::new_exception_args(objexcept::type_viper_type_error(), 1, &[objstr::new_str(msg)]),
    );
}

fn viper_type_error_vtype<B: NativeBackend>(emit: &mut EmitNative<B>, prefix: &[u8], vtype: VType) {
    let name = vtype_name(vtype);
    let mut msg = Vec::with_capacity(prefix.len() + name.len() + 1);
    msg.extend_from_slice(prefix);
    msg.extend_from_slice(name);
    msg.push(b'\'');
    viper_type_error_msg(emit, &msg);
}

fn viper_type_error_vtypes<B: NativeBackend>(
    emit: &mut EmitNative<B>,
    prefix: &[u8],
    mid: &[u8],
    suffix: &[u8],
    lhs: VType,
    rhs: VType,
) {
    let lhs_name = vtype_name(lhs);
    let rhs_name = vtype_name(rhs);
    let mut msg = Vec::with_capacity(prefix.len() + lhs_name.len() + mid.len() + rhs_name.len() + suffix.len());
    msg.extend_from_slice(prefix);
    msg.extend_from_slice(lhs_name);
    msg.extend_from_slice(mid);
    msg.extend_from_slice(rhs_name);
    msg.extend_from_slice(suffix);
    viper_type_error_msg(emit, &msg);
}

fn viper_type_error_vtype_suffix<B: NativeBackend>(
    emit: &mut EmitNative<B>,
    prefix: &[u8],
    vtype: VType,
    suffix: &[u8],
) {
    let name = vtype_name(vtype);
    let mut msg = Vec::with_capacity(prefix.len() + name.len() + suffix.len() + 1);
    msg.extend_from_slice(prefix);
    msg.extend_from_slice(name);
    msg.push(b'\'');
    msg.extend_from_slice(suffix);
    viper_type_error_msg(emit, &msg);
}

fn viper_type_error_local<B: NativeBackend>(
    emit: &mut EmitNative<B>,
    qst: Qstr,
    expected: VType,
    got: VType,
) {
    let local = qstr::str_data(qst).unwrap_or_else(|| b"<?>".to_vec());
    let exp = vtype_name(expected);
    let got_name = vtype_name(got);
    let mut msg = Vec::with_capacity(local.len() + exp.len() + got_name.len() + 40);
    msg.extend_from_slice(b"local '");
    msg.extend_from_slice(&local);
    msg.extend_from_slice(b"' has type '");
    msg.extend_from_slice(exp);
    msg.extend_from_slice(b"' but source is '");
    msg.extend_from_slice(got_name);
    msg.push(b'\'');
    viper_type_error_msg(emit, &msg);
}

fn local_idx_ret_val<B: NativeBackend>(_emit: &EmitNative<B>) -> i32 {
    SIZEOF_NLR_BUF as i32
}
fn local_idx_fun_obj<B: NativeBackend>(emit: &EmitNative<B>) -> i32 {
    emit.code_state_start as i32 + OFFSETOF_CODE_STATE_FUN_BC as i32
}
fn local_idx_old_globals<B: NativeBackend>(emit: &EmitNative<B>) -> i32 {
    emit.code_state_start as i32 + OFFSETOF_CODE_STATE_IP as i32
}
fn local_idx_gen_pc<B: NativeBackend>(emit: &EmitNative<B>) -> i32 {
    emit.code_state_start as i32 + OFFSETOF_CODE_STATE_IP as i32
}
fn local_idx_local_var<B: NativeBackend>(emit: &EmitNative<B>, local_num: usize) -> i32 {
    emit.stack_start as i32 + emit.n_state - 1 - local_num as i32
}

fn max_regs_for_local_vars<B: NativeBackend>() -> usize {
    if mpconfig::PERSISTENT_CODE_SAVE {
        2
    } else {
        3
    }
}

fn reg_local_table<B: NativeBackend>() -> &'static [i32] {
    B::REG_LOCAL_TABLE
}

impl<B: NativeBackend> EmitNative<B> {
    pub fn new(
        emit_common: *mut EmitCommon,
        error_slot: *mut Obj,
        label_slot: *mut usize,
        max_num_labels: usize,
    ) -> *mut EmitNative<B> {
        debug_assert_eq!(obj::OBJ_NULL, Obj(0));
        let mut emit = Box::new(EmitNative {
            emit_common,
            error_slot,
            label_slot,
            exit_label: 0,
            pass: 0,
            do_viper_types: false,
            local_vtype: Vec::new(),
            stack_info: vec![
                StackInfo {
                    vtype: VType::Unbound,
                    kind: StackInfoKind::Value,
                    u_reg: 0,
                    u_imm: 0,
                };
                8
            ],
            saved_stack_vtype: VType::PyObj,
            exc_stack: Vec::with_capacity(8),
            prelude_offset: 0,
            prelude_ptr_index: 0,
            start_offset: 0,
            n_state: 0,
            code_state_start: 0,
            stack_start: 0,
            stack_size: 0,
            n_info: 0,
            n_cell: 0,
            scope: core::ptr::null_mut(),
            as_: B::new_asm(max_num_labels),
            _backend: core::marker::PhantomData,
        });
        asmbase::init(B::asm_base(&mut emit.as_), max_num_labels);
        Box::into_raw(emit)
    }

    pub fn free(emit: *mut EmitNative<B>) {
        if emit.is_null() {
            return;
        }
        unsafe {
            let emit = &mut *emit;
            asmbase::deinit(B::asm_base(&mut emit.as_), false);
            drop(Box::from_raw(emit));
        }
    }

    fn mov_reg_const(emit: &mut EmitNative<B>, reg_dest: i32, const_val: i32) {
        B::load_reg_reg_offset(&mut emit.as_, reg_dest, B::REG_FUN_TABLE, const_val);
    }

    fn mov_state_reg(emit: &mut EmitNative<B>, local_num: i32, reg_src: i32) {
        if unsafe { (*emit.scope).scope_flags & MP_SCOPE_FLAG_GENERATOR != 0 } {
            B::store_reg_reg_offset(&mut emit.as_, reg_src, B::REG_GENERATOR_STATE, local_num);
        } else {
            B::mov_local_reg(&mut emit.as_, local_num, reg_src);
        }
    }

    fn mov_reg_state(emit: &mut EmitNative<B>, reg_dest: i32, local_num: i32) {
        if unsafe { (*emit.scope).scope_flags & MP_SCOPE_FLAG_GENERATOR != 0 } {
            B::load_reg_reg_offset(&mut emit.as_, reg_dest, B::REG_GENERATOR_STATE, local_num);
        } else {
            B::mov_reg_local(&mut emit.as_, reg_dest, local_num);
        }
    }

    fn mov_reg_state_addr(emit: &mut EmitNative<B>, reg_dest: i32, local_num: i32) {
        if unsafe { (*emit.scope).scope_flags & MP_SCOPE_FLAG_GENERATOR != 0 } {
            B::mov_reg_imm(&mut emit.as_, reg_dest, (local_num * B::WORD_SIZE) as usize);
            B::add_reg_reg(&mut emit.as_, reg_dest, B::REG_GENERATOR_STATE);
        } else {
            B::mov_reg_local_addr(&mut emit.as_, reg_dest, local_num);
        }
    }

    fn mov_reg_qstr(emit: &mut EmitNative<B>, arg_reg: i32, qst: Qstr) {
        if mpconfig::PERSISTENT_CODE_SAVE {
            let idx = emit::emit_common_use_qstr(unsafe { &mut *emit.emit_common }, qst);
            B::load16_reg_reg_offset(&mut emit.as_, arg_reg, B::REG_QSTR_TABLE, idx as i32);
        } else if B::HAS_ASM_MOV_REG_QSTR {
            B::mov_reg_qstr(&mut emit.as_, arg_reg, qst);
        } else {
            B::mov_reg_imm(&mut emit.as_, arg_reg, qst as usize);
        }
    }

    fn mov_reg_qstr_obj(emit: &mut EmitNative<B>, reg_dest: i32, qst: Qstr) {
        if mpconfig::PERSISTENT_CODE_SAVE {
            Self::load_reg_with_object(emit, reg_dest, obj::new_qstr(qst));
        } else {
            B::mov_reg_imm(&mut emit.as_, reg_dest, obj::new_qstr(qst).0);
        }
    }

    fn mov_state_imm_via(emit: &mut EmitNative<B>, local_num: i32, imm: i32, reg_temp: i32) {
        B::mov_reg_imm(&mut emit.as_, reg_temp, imm as usize);
        Self::mov_state_reg(emit, local_num, reg_temp);
    }

    fn ensure_extra_stack(emit: &mut EmitNative<B>, delta: usize) {
        let need = (emit.stack_size as usize).saturating_add(delta);
        if need > emit.stack_info.len() {
            let new_alloc = (need + 8) & !3;
            emit.stack_info.resize(new_alloc, StackInfo {
                vtype: VType::Unbound,
                kind: StackInfoKind::Value,
                u_reg: 0,
                u_imm: 0,
            });
        }
    }

    fn adjust_stack(emit: &mut EmitNative<B>, delta: i32) {
        debug_assert!(emit.stack_size + delta >= 0);
        emit.stack_size += delta;
        if emit.pass > PassKind::Scope as i32 {
            unsafe {
                if emit.stack_size as u16 > (*emit.scope).stack_size {
                    (*emit.scope).stack_size = emit.stack_size as u16;
                }
            }
        }
    }

    fn peek_stack(emit: &EmitNative<B>, depth: usize) -> &StackInfo {
        &emit.stack_info[(emit.stack_size as usize).wrapping_sub(1 + depth)]
    }

    fn peek_stack_mut(emit: &mut EmitNative<B>, depth: usize) -> &mut StackInfo {
        let idx = (emit.stack_size as usize).wrapping_sub(1 + depth);
        &mut emit.stack_info[idx]
    }

    fn peek_vtype(emit: &EmitNative<B>, depth: usize) -> VType {
        if emit.do_viper_types {
            Self::peek_stack(emit, depth).vtype
        } else {
            VType::PyObj
        }
    }

    fn need_reg_single(emit: &mut EmitNative<B>, reg_needed: i32, skip_stack_pos: i32) {
        let skip = emit.stack_size - skip_stack_pos;
        let stack_start = emit.stack_start;
        let mut spill = Vec::new();
        for i in 0..emit.stack_size {
            if i != skip {
                let si = &emit.stack_info[i as usize];
                if si.kind == StackInfoKind::Reg && si.u_reg == reg_needed {
                    spill.push((i, si.u_reg));
                }
            }
        }
        for (i, reg) in spill {
            emit.stack_info[i as usize].kind = StackInfoKind::Value;
            Self::mov_state_reg(emit, stack_start as i32 + i, reg);
        }
    }

    fn need_reg_all(emit: &mut EmitNative<B>) {
        let stack_start = emit.stack_start;
        let mut spill = Vec::new();
        for i in 0..emit.stack_size {
            let si = &emit.stack_info[i as usize];
            if si.kind == StackInfoKind::Reg {
                spill.push((i, si.u_reg));
            }
        }
        for (i, reg) in spill {
            emit.stack_info[i as usize].kind = StackInfoKind::Value;
            Self::mov_state_reg(emit, stack_start as i32 + i, reg);
        }
    }

    fn load_reg_stack_imm(emit: &mut EmitNative<B>, reg_dest: i32, si: &StackInfo, convert_to_pyobj: bool) -> VType {
        if !convert_to_pyobj && emit.do_viper_types {
            B::mov_reg_imm(&mut emit.as_, reg_dest, si.u_imm as usize);
            si.vtype
        } else {
            match si.vtype {
                VType::PyObj => B::mov_reg_imm(&mut emit.as_, reg_dest, si.u_imm as usize),
                VType::Bool => {
                    Self::mov_reg_const(emit, reg_dest, (mp_f::CONST_FALSE_OBJ + si.u_imm as u32) as i32)
                }
                VType::Int | VType::Uint => {
                    B::mov_reg_imm(
                        &mut emit.as_,
                        reg_dest,
                        obj::new_small_int(si.u_imm as obj::Int).0 as usize,
                    )
                }
                VType::PtrNone => Self::mov_reg_const(emit, reg_dest, mp_f::CONST_NONE_OBJ as i32),
                _ => viper_type_error_vtype(emit, b"can't load immediate value for '", si.vtype),
            }
            VType::PyObj
        }
    }

    fn need_stack_settled(emit: &mut EmitNative<B>) {
        Self::need_reg_all(emit);
        for i in 0..emit.stack_size {
            if emit.stack_info[i as usize].kind == StackInfoKind::Imm {
                let vtype;
                let imm = emit.stack_info[i as usize].u_imm;
                emit.stack_info[i as usize].kind = StackInfoKind::Value;
                let si = StackInfo {
                    vtype: emit.stack_info[i as usize].vtype,
                    kind: StackInfoKind::Imm,
                    u_reg: 0,
                    u_imm: imm,
                };
                vtype = Self::load_reg_stack_imm(emit, B::REG_TEMP1, &si, false);
                emit.stack_info[i as usize].vtype = vtype;
                Self::mov_state_reg(emit, emit.stack_start as i32 + i, B::REG_TEMP1);
            }
        }
    }

    fn emit_access_stack(emit: &mut EmitNative<B>, pos: i32, vtype: &mut VType, reg_dest: i32) {
        Self::need_reg_single(emit, reg_dest, pos);
        let idx = (emit.stack_size - pos) as usize;
        let si = emit.stack_info[idx];
        *vtype = si.vtype;
        match si.kind {
            StackInfoKind::Value => {
                Self::mov_reg_state(emit, reg_dest, emit.stack_start as i32 + emit.stack_size - pos);
            }
            StackInfoKind::Reg => {
                if si.u_reg != reg_dest {
                    B::mov_reg_reg(&mut emit.as_, reg_dest, si.u_reg);
                }
            }
            StackInfoKind::Imm => {
                *vtype = Self::load_reg_stack_imm(emit, reg_dest, &si, false);
            }
        }
    }

    fn emit_pre_pop_reg(emit: &mut EmitNative<B>, vtype: &mut VType, reg_dest: i32) {
        Self::emit_access_stack(emit, 1, vtype, reg_dest);
        Self::adjust_stack(emit, -1);
    }

    fn emit_pre_pop_discard(emit: &mut EmitNative<B>) {
        Self::adjust_stack(emit, -1);
    }

    /// X = pop(); discard(); push(X) — see `emit_fold_stack_top` in py/emitnative.c.
    fn fold_stack_top(emit: &mut EmitNative<B>, reg_dest: i32) {
        let below = (emit.stack_size - 2) as usize;
        emit.stack_info[below] = emit.stack_info[below + 1];
        if emit.stack_info[below].kind == StackInfoKind::Value && !need_gen_return_obj(emit) {
            Self::mov_reg_state(emit, reg_dest, emit.stack_start as i32 + emit.stack_size - 1);
            emit.stack_info[below].kind = StackInfoKind::Reg;
            emit.stack_info[below].u_reg = reg_dest;
        } else if emit.stack_info[below].kind == StackInfoKind::Value {
            emit.stack_info[below].kind = StackInfoKind::Reg;
            emit.stack_info[below].u_reg = reg_dest;
        }
        Self::adjust_stack(emit, -1);
    }

    fn emit_pre_pop_reg_reg(
        emit: &mut EmitNative<B>,
        vtype_a: &mut VType,
        reg_a: i32,
        vtype_b: &mut VType,
        reg_b: i32,
    ) {
        Self::emit_pre_pop_reg(emit, vtype_a, reg_a);
        Self::emit_pre_pop_reg(emit, vtype_b, reg_b);
    }

    fn emit_pre_pop_reg_flexible(
        emit: &mut EmitNative<B>,
        vtype: &mut VType,
        reg_dest: &mut i32,
        not_r1: i32,
        not_r2: i32,
    ) {
        let si = *Self::peek_stack(emit, 0);
        if si.kind == StackInfoKind::Reg && si.u_reg != not_r1 && si.u_reg != not_r2 {
            *vtype = si.vtype;
            *reg_dest = si.u_reg;
            Self::need_reg_single(emit, *reg_dest, 1);
        } else {
            Self::emit_access_stack(emit, 1, vtype, *reg_dest);
        }
        Self::adjust_stack(emit, -1);
    }

    fn viper_type_error(emit: &mut EmitNative<B>, msg: &[u8]) {
        viper_type_error_msg(emit, msg);
    }

    fn normalize_inplace_binary_op(op: BinaryOp) -> BinaryOp {
        if matches!(
            op,
            BinaryOp::InplaceOr
                | BinaryOp::InplaceXor
                | BinaryOp::InplaceAnd
                | BinaryOp::InplaceLshift
                | BinaryOp::InplaceRshift
                | BinaryOp::InplaceAdd
                | BinaryOp::InplaceSubtract
                | BinaryOp::InplaceMultiply
                | BinaryOp::InplaceFloorDivide
                | BinaryOp::InplaceModulo
                | BinaryOp::InplacePower
        ) {
            BinaryOp::from_u8(
                (op as u8).wrapping_sub((BinaryOp::InplaceOr as u8).wrapping_sub(BinaryOp::Or as u8)),
            )
        } else {
            op
        }
    }

    fn emit_post_push_reg(emit: &mut EmitNative<B>, vtype: VType, reg: i32) {
        Self::ensure_extra_stack(emit, 1);
        let si = &mut emit.stack_info[emit.stack_size as usize];
        si.vtype = vtype;
        si.kind = StackInfoKind::Reg;
        si.u_reg = reg;
        Self::adjust_stack(emit, 1);
    }

    fn emit_post_push_imm(emit: &mut EmitNative<B>, vtype: VType, imm: i64) {
        Self::ensure_extra_stack(emit, 1);
        let si = &mut emit.stack_info[emit.stack_size as usize];
        si.vtype = vtype;
        si.kind = StackInfoKind::Imm;
        si.u_imm = imm;
        Self::adjust_stack(emit, 1);
    }

    fn emit_call(emit: &mut EmitNative<B>, fun_kind: u32) {
        Self::need_reg_all(emit);
        B::call_ind(&mut emit.as_, fun_kind);
    }

    fn emit_call_with_imm_arg(emit: &mut EmitNative<B>, fun_kind: u32, arg_val: i64, arg_reg: i32) {
        Self::need_reg_all(emit);
        B::mov_reg_imm(&mut emit.as_, arg_reg, arg_val as usize);
        B::call_ind(&mut emit.as_, fun_kind);
    }

    fn emit_call_with_qstr_arg(emit: &mut EmitNative<B>, fun_kind: u32, qst: Qstr, arg_reg: i32) {
        Self::need_reg_all(emit);
        Self::mov_reg_qstr(emit, arg_reg, qst);
        B::call_ind(&mut emit.as_, fun_kind);
    }

    fn emit_call_with_2_imm_args(
        emit: &mut EmitNative<B>,
        fun_kind: u32,
        arg_val1: i64,
        arg_reg1: i32,
        arg_val2: i64,
        arg_reg2: i32,
    ) {
        Self::need_reg_all(emit);
        B::mov_reg_imm(&mut emit.as_, arg_reg1, arg_val1 as usize);
        B::mov_reg_imm(&mut emit.as_, arg_reg2, arg_val2 as usize);
        B::call_ind(&mut emit.as_, fun_kind);
    }

    fn host_native_gen_uses_exception_return() -> bool {
        mpconfig::NLR_SETJMP && !B::N_NLR_SETJMP
    }

    fn emit_gen_exception_return(e: &mut EmitNative<B>) {
        let ret_local = local_idx_ret_val(e);
        B::mov_reg_imm(&mut e.as_, B::REG_TEMP1, MP_VM_RETURN_EXCEPTION);
        B::mov_local_reg(&mut e.as_, ret_local, B::REG_TEMP1);
        Self::jump(Self::emit_ptr(e), e.exit_label);
    }

    fn emit_ptr(e: &mut EmitNative<B>) -> *mut crate::emit::Emit {
        e as *mut EmitNative<B> as *mut crate::emit::Emit
    }

    fn emit_gen_throw_if_pending(e: &mut EmitNative<B>, continue_label: usize) {
        let throw_val = local_idx_throw_val(e);
        B::mov_reg_local(&mut e.as_, B::REG_ARG_1, throw_val);
        B::mov_local_mp_obj_null(&mut e.as_, throw_val, B::REG_ARG_2);
        if Self::host_native_gen_uses_exception_return() {
            B::jump_if_reg_zero(&mut e.as_, B::REG_ARG_1, continue_label, false);
            B::mov_reg_reg(&mut e.as_, B::REG_ARG_2, B::REG_GENERATOR_STATE);
            Self::emit_call(e, mp_f::NATIVE_GEN_FINISH_THROW);
            Self::emit_gen_exception_return(e);
        } else {
            Self::emit_call(e, mp_f::NATIVE_RAISE);
        }
        asmbase::label_assign(B::asm_base(&mut e.as_), continue_label);
    }

    fn emit_yield_from_handle_delegate_result(e: &mut EmitNative<B>, label_slot: usize) {
        if Self::host_native_gen_uses_exception_return() {
            B::mov_reg_imm(&mut e.as_, B::REG_TEMP1, MP_VM_RETURN_YIELD);
            B::jump_if_reg_eq(&mut e.as_, B::REG_RET, B::REG_TEMP1, label_slot + 1);
            B::mov_reg_imm(&mut e.as_, B::REG_TEMP1, MP_VM_RETURN_EXCEPTION);
            B::jump_if_reg_eq(&mut e.as_, B::REG_RET, B::REG_TEMP1, label_slot + 3);
            e.saved_stack_vtype = VType::PyObj;
            Self::adjust_stack_size(Self::emit_ptr(e), 1);
            Self::fold_stack_top(e, B::REG_ARG_1);
            B::jump(&mut e.as_, label_slot + 4);
            asmbase::label_assign(B::asm_base(&mut e.as_), label_slot + 3);
            let mut vtype = VType::PyObj;
            Self::emit_access_stack(e, 1, &mut vtype, B::REG_TEMP0);
            B::store_reg_reg_offset(
                &mut e.as_,
                B::REG_TEMP0,
                B::REG_GENERATOR_STATE,
                OFFSETOF_CODE_STATE_STATE as i32,
            );
            Self::emit_gen_exception_return(e);
            asmbase::label_assign(B::asm_base(&mut e.as_), label_slot + 4);
            return;
        }
        B::jump_if_reg_nonzero(&mut e.as_, B::REG_RET, label_slot + 1, true);
        e.saved_stack_vtype = VType::PyObj;
        Self::adjust_stack_size(Self::emit_ptr(e), 1);
        Self::fold_stack_top(e, B::REG_ARG_1);
    }

    fn emit_get_stack_pointer_to_reg_for_pop(emit: &mut EmitNative<B>, reg_dest: i32, n_pop: usize) {
        Self::need_reg_all(emit);
        for i in 0..n_pop {
            let idx = (emit.stack_size - 1 - i as i32) as usize;
            let si = emit.stack_info[idx];
            if si.kind == StackInfoKind::Imm {
                emit.stack_info[idx].kind = StackInfoKind::Value;
                let vtype =
                    Self::load_reg_stack_imm(emit, reg_dest, &si, true);
                emit.stack_info[idx].vtype = vtype;
                Self::mov_state_reg(
                    emit,
                    emit.stack_start as i32 + emit.stack_size - 1 - i as i32,
                    reg_dest,
                );
            }
            debug_assert_eq!(emit.stack_info[idx].kind, StackInfoKind::Value);
        }
        for i in 0..n_pop {
            let idx = (emit.stack_size - 1 - i as i32) as usize;
            let vtype = emit.stack_info[idx].vtype;
            if vtype != VType::PyObj {
                let local_num = emit.stack_start as i32 + emit.stack_size - 1 - i as i32;
                Self::mov_reg_state(emit, B::REG_ARG_1, local_num);
                Self::emit_call_with_imm_arg(emit, mp_f::CONVERT_NATIVE_TO_OBJ, vtype as i64, B::REG_ARG_2);
                Self::mov_state_reg(emit, local_num, B::REG_RET);
                emit.stack_info[idx].vtype = VType::PyObj;
            }
        }
        Self::adjust_stack(emit, -(n_pop as i32));
        Self::mov_reg_state_addr(emit, reg_dest, emit.stack_start as i32 + emit.stack_size);
    }

    fn emit_get_stack_pointer_to_reg_for_push(emit: &mut EmitNative<B>, reg_dest: i32, n_push: usize) {
        Self::need_reg_all(emit);
        Self::ensure_extra_stack(emit, n_push);
        for i in 0..n_push {
            let si = &mut emit.stack_info[(emit.stack_size + i as i32) as usize];
            si.kind = StackInfoKind::Value;
            si.vtype = VType::PyObj;
        }
        Self::mov_reg_state_addr(emit, reg_dest, emit.stack_start as i32 + emit.stack_size);
        Self::adjust_stack(emit, n_push as i32);
    }

    fn push_exc_stack(emit: &mut EmitNative<B>, label: usize, is_finally: bool) {
        let finally_sp_index = if is_finally {
            ((emit.stack_size as i32) - 1).max(0) as i16
        } else {
            -1
        };
        let e = ExcStackEntry {
            label: label as u16,
            is_finally,
            unwind_label: UNWIND_LABEL_UNUSED,
            is_active: true,
            finally_sp_index,
        };
        emit.exc_stack.push(e);
        let exc_pc = local_idx_exc_handler_pc(emit);
        B::mov_reg_pcrel(&mut emit.as_, B::REG_RET, label);
        B::mov_local_reg(&mut emit.as_, exc_pc, B::REG_RET);
    }

    fn leave_exc_stack(emit: &mut EmitNative<B>, start_of_handler: bool) {
        debug_assert!(!emit.exc_stack.is_empty());
        if let Some(entry) = emit.exc_stack.last_mut() {
            entry.is_active = false;
        }
        let mut e_idx = emit.exc_stack.len();
        loop {
            if e_idx == 0 {
                if start_of_handler {
                    return;
                }
                B::clr_reg(&mut emit.as_, B::REG_RET);
                break;
            }
            e_idx -= 1;
            if emit.exc_stack[e_idx].is_active {
                B::mov_reg_pcrel(&mut emit.as_, B::REG_RET, emit.exc_stack[e_idx].label as usize);
                break;
            }
        }
        let exc_pc = local_idx_exc_handler_pc(emit);
        B::mov_local_reg(&mut emit.as_, exc_pc, B::REG_RET);
    }

    fn pop_exc_stack(emit: &mut EmitNative<B>) -> ExcStackEntry {
        debug_assert!(!emit.exc_stack.is_empty());
        let entry = emit.exc_stack.pop().unwrap();
        debug_assert!(!entry.is_active);
        entry
    }

    fn setup_with(emit: &mut EmitNative<B>, label: usize) {
        let mut vtype = VType::PyObj;
        Self::emit_access_stack(emit, 1, &mut vtype, B::REG_ARG_1);
        debug_assert_eq!(vtype, VType::PyObj);
        Self::emit_get_stack_pointer_to_reg_for_push(emit, B::REG_ARG_3, 2);
        Self::emit_call_with_qstr_arg(emit, mp_f::LOAD_METHOD, qstr::from_str("__exit__"), B::REG_ARG_2);
        Self::emit_pre_pop_reg(emit, &mut vtype, B::REG_ARG_3);
        Self::emit_pre_pop_reg(emit, &mut vtype, B::REG_ARG_2);
        Self::emit_pre_pop_reg(emit, &mut vtype, B::REG_ARG_1);
        Self::emit_post_push_reg(emit, vtype, B::REG_ARG_2);
        Self::emit_post_push_reg(emit, vtype, B::REG_ARG_3);
        Self::emit_get_stack_pointer_to_reg_for_push(emit, B::REG_ARG_3, 2);
        Self::emit_call_with_qstr_arg(emit, mp_f::LOAD_METHOD, qstr::from_str("__enter__"), B::REG_ARG_2);
        Self::emit_get_stack_pointer_to_reg_for_pop(emit, B::REG_ARG_3, 2);
        Self::emit_call_with_2_imm_args(emit, mp_f::CALL_METHOD_N_KW, 0, B::REG_ARG_1, 0, B::REG_ARG_2);
        Self::emit_post_push_reg(emit, VType::PyObj, B::REG_RET);
        Self::need_stack_settled(emit);
        Self::push_exc_stack(emit, label, true);
        let si = *Self::peek_stack(emit, 0);
        Self::ensure_extra_stack(emit, 1);
        emit.stack_info[emit.stack_size as usize] = si;
        Self::adjust_stack(emit, 1);
    }

    fn load_reg_with_object(emit: &mut EmitNative<B>, reg: i32, obj_in: Obj) {
        unsafe {
            (*emit.scope).scope_flags |= MP_SCOPE_FLAG_HASCONSTS;
        }
        let table_off = emit::emit_common_use_const_obj(unsafe { &mut *emit.emit_common }, obj_in);
        Self::mov_reg_state(emit, B::REG_TEMP0, local_idx_fun_obj(emit));
        B::load_reg_reg_offset(&mut emit.as_, B::REG_TEMP0, B::REG_TEMP0, OFFSETOF_OBJ_FUN_BC_CONTEXT as i32);
        B::load_reg_reg_offset(
            &mut emit.as_,
            B::REG_TEMP0,
            B::REG_TEMP0,
            OFFSETOF_MODULE_CONTEXT_OBJ_TABLE as i32,
        );
        B::load_reg_reg_offset(&mut emit.as_, reg, B::REG_TEMP0, table_off as i32);
    }

    fn write_code_info_byte(emit: &mut EmitNative<B>, val: u8) {
        asmbase::data(B::asm_base(&mut emit.as_), 1, val as usize);
    }

    fn write_code_info_qstr(emit: &mut EmitNative<B>, qst: Qstr) {
        let idx = emit::emit_common_use_qstr(unsafe { &mut *emit.emit_common }, qst);
        encode_uint(
            &mut |b| {
                let p = asmbase::get_cur_to_write_bytes(B::asm_base(&mut emit.as_), 1);
                if !p.is_null() {
                    unsafe {
                        *p = b;
                    }
                }
            },
            idx,
        );
    }

    fn prelude_sig_encode(emit: &mut EmitNative<B>, scope: &Scope) {
        let mut s = scope.num_locals as usize + scope.stack_size as usize;
        if s == 0 {
            s = 1;
        }
        if mpconfig::DEBUG_VM_STACK_OVERFLOW != 0 {
            s += 1;
        }
        let mut e = 0usize;
        let mut f = scope.scope_flags as usize & bc0::SCOPE_FLAG_ALL_SIG as usize;
        let mut a = scope.num_pos_args as usize;
        let mut k = scope.num_kwonly_args as usize;
        let mut d = scope.num_def_pos_args as usize;
        s -= 1;
        let mut z: u8 = ((s & 0xf) << 3) as u8 | ((e & 1) << 2) as u8 | (a & 3) as u8;
        s >>= 4;
        e >>= 1;
        a >>= 2;
        while s | e | f | a | k | d != 0 {
            Self::write_code_info_byte(emit, 0x80 | z);
            z = ((f & 1) << 6) as u8
                | ((s & 3) << 4) as u8
                | ((k & 1) << 3) as u8
                | ((a & 1) << 2) as u8
                | ((e & 1) << 1) as u8
                | (d & 1) as u8;
            s >>= 2;
            e >>= 1;
            f >>= 1;
            a >>= 1;
            k >>= 1;
            d >>= 1;
        }
        Self::write_code_info_byte(emit, z);
    }

    fn prelude_size_encode(emit: &mut EmitNative<B>, mut i: usize, mut c: usize) {
        loop {
            let mut z = ((i & 0x3f) << 1) | (c & 1);
            c >>= 1;
            i >>= 6;
            if c | i != 0 {
                z |= 0x80;
            }
            Self::write_code_info_byte(emit, z as u8);
            if c | i == 0 {
                break;
            }
        }
    }

    fn copy_parent_args_to_call_regs(emit: &mut EmitNative<B>) {
        if B::REG_ARG_2 != B::REG_PARENT_ARG_2 {
            B::mov_reg_reg(&mut emit.as_, B::REG_ARG_2, B::REG_PARENT_ARG_2);
        }
        if B::REG_ARG_3 != B::REG_PARENT_ARG_3 {
            B::mov_reg_reg(&mut emit.as_, B::REG_ARG_3, B::REG_PARENT_ARG_3);
        }
        if B::REG_ARG_4 != B::REG_PARENT_ARG_4 {
            B::mov_reg_reg(&mut emit.as_, B::REG_ARG_4, B::REG_PARENT_ARG_4);
        }
    }

    fn load_fun_table(emit: &mut EmitNative<B>, fun_table_off: usize) {
        B::load_reg_reg_offset(
            &mut emit.as_,
            B::REG_FUN_TABLE,
            B::REG_PARENT_ARG_1,
            OFFSETOF_OBJ_FUN_BC_CONTEXT as i32,
        );
        if mpconfig::PERSISTENT_CODE_SAVE {
            B::load_reg_reg_offset(
                &mut emit.as_,
                B::REG_QSTR_TABLE,
                B::REG_FUN_TABLE,
                OFFSETOF_MODULE_CONTEXT_QSTR_TABLE as i32,
            );
        }
        B::load_reg_reg_offset(
            &mut emit.as_,
            B::REG_FUN_TABLE,
            B::REG_FUN_TABLE,
            OFFSETOF_MODULE_CONTEXT_OBJ_TABLE as i32,
        );
        B::load_reg_reg_offset(
            &mut emit.as_,
            B::REG_FUN_TABLE,
            B::REG_FUN_TABLE,
            fun_table_off as i32,
        );
    }

    fn global_exc_entry(emit: &mut EmitNative<B>) {
        emit.exit_label = unsafe { *emit.label_slot };

        if !need_global_exc_handler(emit) {
            return;
        }

        let nlr_label = unsafe { *emit.label_slot + 1 };
        let start_label = unsafe { *emit.label_slot + 2 };
        let global_except_label = unsafe { *emit.label_slot + 3 };

        if unsafe { (*emit.scope).scope_flags & MP_SCOPE_FLAG_GENERATOR == 0 } {
            Self::mov_reg_state(emit, B::REG_ARG_1, local_idx_fun_obj(emit));
            B::load_reg_reg_offset(
                &mut emit.as_,
                B::REG_ARG_1,
                B::REG_ARG_1,
                OFFSETOF_OBJ_FUN_BC_CONTEXT as i32,
            );
            B::load_reg_reg_offset(
                &mut emit.as_,
                B::REG_ARG_1,
                B::REG_ARG_1,
                OFFSETOF_MODULE_CONTEXT_GLOBALS as i32,
            );
            Self::emit_call(emit, mp_f::NATIVE_SWAP_GLOBALS);
            Self::mov_state_reg(emit, local_idx_old_globals(emit), B::REG_RET);
        }

        if mpconfig::NLR_SETJMP && !B::N_NLR_SETJMP {
            // Host NLR uses outer nlr::protect (gen_resume / dispatch); inline NLR_PUSH
            // + setjmp cannot unwind through emitted machine code on this port.
            if unsafe { (*emit.scope).exc_stack_size != 0 } {
                let exc_unwind = local_idx_exc_handler_unwind(emit);
                let exc_val = local_idx_exc_val(emit);
                B::mov_local_mp_obj_null(&mut emit.as_, exc_unwind, B::REG_ZERO);
                B::mov_local_reg(&mut emit.as_, exc_val, B::REG_ZERO);
            }
            B::jump(&mut emit.as_, start_label);
            B::asm_base(&mut emit.as_).suppress_code();
        } else if unsafe { (*emit.scope).exc_stack_size == 0 } {
            if unsafe { (*emit.scope).scope_flags & MP_SCOPE_FLAG_GENERATOR == 0 } {
                B::jump_if_reg_zero(&mut emit.as_, B::REG_RET, start_label, false);
            }
            B::mov_reg_local_addr(&mut emit.as_, B::REG_ARG_1, 0);
            Self::emit_call(emit, mp_f::NLR_PUSH);
            if B::N_NLR_SETJMP {
                B::mov_reg_local_addr(&mut emit.as_, B::REG_ARG_1, 2);
                Self::emit_call(emit, mp_f::SETJMP);
            }
            B::jump_if_reg_zero(&mut emit.as_, B::REG_RET, start_label, true);
        } else {
            let exc_unwind = local_idx_exc_handler_unwind(emit);
            let exc_val = local_idx_exc_val(emit);
            let exc_pc = local_idx_exc_handler_pc(emit);
            B::mov_local_mp_obj_null(&mut emit.as_, exc_unwind, B::REG_ZERO);
            B::mov_local_reg(&mut emit.as_, exc_val, B::REG_ZERO);
            B::mov_reg_pcrel(&mut emit.as_, B::REG_LOCAL_1, start_label);
            asmbase::label_assign(B::asm_base(&mut emit.as_), nlr_label);
            B::mov_reg_local_addr(&mut emit.as_, B::REG_ARG_1, 0);
            Self::emit_call(emit, mp_f::NLR_PUSH);
            if B::N_NLR_SETJMP {
                B::mov_reg_local_addr(&mut emit.as_, B::REG_ARG_1, 2);
                Self::emit_call(emit, mp_f::SETJMP);
            }
            B::jump_if_reg_nonzero(&mut emit.as_, B::REG_RET, global_except_label, true);
            B::mov_local_mp_obj_null(&mut emit.as_, exc_pc, B::REG_ZERO);
            B::jump_reg(&mut emit.as_, B::REG_LOCAL_1);
            asmbase::label_assign(B::asm_base(&mut emit.as_), global_except_label);
            B::mov_reg_local(&mut emit.as_, B::REG_LOCAL_1, exc_pc);
            B::jump_if_reg_nonzero(&mut emit.as_, B::REG_LOCAL_1, nlr_label, false);
        }

        if unsafe { (*emit.scope).scope_flags & MP_SCOPE_FLAG_GENERATOR == 0 } {
            Self::mov_reg_state(emit, B::REG_ARG_1, local_idx_old_globals(emit));
            Self::emit_call(emit, mp_f::NATIVE_SWAP_GLOBALS);
        }

        if unsafe { (*emit.scope).scope_flags & MP_SCOPE_FLAG_GENERATOR != 0 } {
            let exc_val = local_idx_exc_val(emit);
            B::mov_reg_local(&mut emit.as_, B::REG_TEMP0, exc_val);
            B::store_reg_reg_offset(
                &mut emit.as_,
                B::REG_TEMP0,
                B::REG_GENERATOR_STATE,
                OFFSETOF_CODE_STATE_STATE as i32,
            );
            B::mov_reg_imm(&mut emit.as_, B::REG_PARENT_RET, MP_VM_RETURN_EXCEPTION);
            B::exit(&mut emit.as_);
        } else {
            let exc_val = local_idx_exc_val(emit);
            B::mov_reg_local(&mut emit.as_, B::REG_ARG_1, exc_val);
            Self::emit_call(emit, mp_f::NATIVE_RAISE);
        }

        asmbase::label_assign(B::asm_base(&mut emit.as_), start_label);

        if unsafe { (*emit.scope).scope_flags & MP_SCOPE_FLAG_GENERATOR != 0 } {
            Self::mov_reg_state(emit, B::REG_TEMP0, local_idx_gen_pc(emit));
            B::jump_reg(&mut emit.as_, B::REG_TEMP0);
            emit.start_offset = B::asm_base(&mut emit.as_).get_code_pos() as i32;
            if !emit.do_viper_types && Self::host_native_gen_uses_exception_return() {
                Self::emit_gen_throw_if_pending(emit, unsafe { *emit.label_slot + 4 });
            } else {
                let throw_val = local_idx_throw_val(emit);
                B::mov_reg_local(&mut emit.as_, B::REG_ARG_1, throw_val);
                B::mov_local_mp_obj_null(&mut emit.as_, throw_val, B::REG_ARG_2);
                Self::emit_call(emit, mp_f::NATIVE_RAISE);
            }
        }
    }

    fn global_exc_exit(emit: &mut EmitNative<B>) {
        Self::need_stack_settled(emit);
        asmbase::label_assign(B::asm_base(&mut emit.as_), emit.exit_label);

        if need_global_exc_handler(emit) {
            if unsafe { (*emit.scope).scope_flags & MP_SCOPE_FLAG_GENERATOR == 0 } {
                Self::mov_reg_state(emit, B::REG_ARG_1, local_idx_old_globals(emit));
                if unsafe { (*emit.scope).exc_stack_size == 0 } {
                    B::jump_if_reg_zero(
                        &mut emit.as_,
                        B::REG_ARG_1,
                        emit.exit_label + 1,
                        false,
                    );
                }
                Self::emit_call(emit, mp_f::NATIVE_SWAP_GLOBALS);
            }
            if mpconfig::NLR_SETJMP && !B::N_NLR_SETJMP {
                // Matched host-only skip in global_exc_entry; no inline NLR to pop.
            } else {
                Self::emit_call(emit, mp_f::NLR_POP);
            }
            if unsafe { (*emit.scope).scope_flags & MP_SCOPE_FLAG_GENERATOR == 0 }
                && unsafe { (*emit.scope).exc_stack_size == 0 }
            {
                asmbase::label_assign(B::asm_base(&mut emit.as_), emit.exit_label + 1);
            }
            let ret_local = local_idx_ret_val(emit);
            B::mov_reg_local(&mut emit.as_, B::REG_PARENT_RET, ret_local);
        }

        B::exit(&mut emit.as_);
    }

    fn start_pass_native_py(emit: &mut EmitNative<B>, scope: &Scope, fun_table_off: usize) {
        emit.n_state = generator_n_state(emit);

        if scope.scope_flags & MP_SCOPE_FLAG_GENERATOR != 0 {
            asmbase::data(
                B::asm_base(&mut emit.as_),
                B::WORD_SIZE as u32,
                emit.prelude_ptr_index as usize,
            );
            asmbase::data(
                B::asm_base(&mut emit.as_),
                B::WORD_SIZE as u32,
                emit.start_offset as usize,
            );
            B::entry(&mut emit.as_, emit.code_state_start as i32, None);
            emit.code_state_start = 0;
            emit.stack_start = SIZEOF_CODE_STATE as u16;
            B::mov_reg_reg(&mut emit.as_, B::REG_GENERATOR_STATE, B::REG_PARENT_ARG_1);
            let throw_val = local_idx_throw_val(emit);
            B::mov_local_reg(&mut emit.as_, throw_val, B::REG_PARENT_ARG_2);
            let fun_obj = local_idx_fun_obj(emit);
            B::load_reg_reg_offset(
                &mut emit.as_,
                B::REG_TEMP0,
                B::REG_GENERATOR_STATE,
                fun_obj,
            );
            B::load_reg_reg_offset(
                &mut emit.as_,
                B::REG_TEMP0,
                B::REG_TEMP0,
                OFFSETOF_OBJ_FUN_BC_CONTEXT as i32,
            );
            if mpconfig::PERSISTENT_CODE_SAVE {
                B::load_reg_reg_offset(
                    &mut emit.as_,
                    B::REG_QSTR_TABLE,
                    B::REG_TEMP0,
                    OFFSETOF_MODULE_CONTEXT_QSTR_TABLE as i32,
                );
            }
            B::load_reg_reg_offset(
                &mut emit.as_,
                B::REG_TEMP0,
                B::REG_TEMP0,
                OFFSETOF_MODULE_CONTEXT_OBJ_TABLE as i32,
            );
            B::load_reg_reg_offset(
                &mut emit.as_,
                B::REG_FUN_TABLE,
                B::REG_TEMP0,
                fun_table_off as i32,
            );
        } else {
            emit.stack_start = emit.code_state_start + SIZEOF_CODE_STATE as u16;
            asmbase::align(B::asm_base(&mut emit.as_), B::WORD_SIZE as u32);
            asmbase::data(
                B::asm_base(&mut emit.as_),
                B::WORD_SIZE as u32,
                emit.prelude_ptr_index as usize,
            );
            B::entry(
                &mut emit.as_,
                emit.stack_start as i32 + emit.n_state,
                None,
            );
            Self::load_fun_table(emit, fun_table_off);
            Self::mov_state_reg(emit, local_idx_fun_obj(emit), B::REG_PARENT_ARG_1);
            Self::mov_state_imm_via(
                emit,
                emit.code_state_start as i32 + OFFSETOF_CODE_STATE_N_STATE as i32,
                emit.n_state,
                B::REG_ARG_1,
            );
            B::mov_reg_local_addr(
                &mut emit.as_,
                B::REG_ARG_1,
                emit.code_state_start as i32,
            );
            Self::copy_parent_args_to_call_regs(emit);
            B::setup_code_state_call(&mut emit.as_);
        }

        Self::global_exc_entry(emit);

        if can_use_regs_for_locals(emit) {
            let n = max_regs_for_local_vars::<B>().min(scope.num_locals as usize);
            for i in 0..n {
                let local = local_idx_local_var(emit, i);
                B::mov_reg_local(&mut emit.as_, reg_local_table::<B>()[i], local);
            }
        }

        for id in &scope.id_info {
            if id.kind == IdInfoKind::Cell {
                emit.local_vtype[id.local_num as usize] = VType::PyObj;
            }
        }
    }

    fn start_pass_viper(emit: &mut EmitNative<B>, scope: &Scope, fun_table_off: usize) {
        emit.n_state = generator_n_state(emit);
        let mut num_locals_in_regs = 0i32;
        if can_use_regs_for_locals(emit) {
            num_locals_in_regs = scope.num_locals as i32;
            if num_locals_in_regs > max_regs_for_local_vars::<B>() as i32 {
                num_locals_in_regs = max_regs_for_local_vars::<B>() as i32;
            }
            if scope.num_pos_args as i32 >= max_regs_for_local_vars::<B>() as i32 + 1 {
                num_locals_in_regs -= 1;
            }
        }

        if need_global_exc_handler(emit) {
            emit.stack_start = emit.code_state_start + 2;
        } else if scope.scope_flags & MP_SCOPE_FLAG_HASCONSTS != 0 {
            emit.stack_start = emit.code_state_start + 1;
        } else {
            emit.stack_start = emit.code_state_start;
        }

        B::entry(
            &mut emit.as_,
            emit.stack_start as i32 + emit.n_state - num_locals_in_regs,
            None,
        );
        Self::load_fun_table(emit, fun_table_off);
        if need_fun_obj(emit) {
            Self::mov_state_reg(emit, local_idx_fun_obj(emit), B::REG_PARENT_ARG_1);
        }
        B::mov_reg_reg(&mut emit.as_, B::REG_ARG_1, B::REG_PARENT_ARG_2);
        B::mov_reg_reg(&mut emit.as_, B::REG_ARG_2, B::REG_PARENT_ARG_3);
        B::mov_reg_reg(&mut emit.as_, B::REG_LOCAL_LAST, B::REG_PARENT_ARG_4);
        B::jump_if_reg_nonzero(&mut emit.as_, B::REG_ARG_2, unsafe { *emit.label_slot + 4 }, true);
        B::mov_reg_imm(&mut emit.as_, B::REG_ARG_3, scope.num_pos_args as usize);
        B::jump_if_reg_eq(
            &mut emit.as_,
            B::REG_ARG_1,
            B::REG_ARG_3,
            unsafe { *emit.label_slot + 5 },
        );
        asmbase::label_assign(B::asm_base(&mut emit.as_), unsafe { *emit.label_slot + 4 });
        B::mov_reg_imm(
            &mut emit.as_,
            B::REG_ARG_3,
            crate::argcheck::make_sig(
                scope.num_pos_args as usize,
                scope.num_pos_args as usize,
                false,
            ) as usize,
        );
        Self::emit_call(emit, mp_f::ARG_CHECK_NUM_SIG);
        asmbase::label_assign(B::asm_base(&mut emit.as_), unsafe { *emit.label_slot + 5 });

        for i in 0..scope.num_pos_args as i32 {
            let mut r = B::REG_ARG_1;
            B::load_reg_reg_offset(&mut emit.as_, B::REG_ARG_1, B::REG_LOCAL_LAST, i);
            if emit.local_vtype[i as usize] != VType::PyObj {
                Self::emit_call_with_imm_arg(
                    emit,
                    mp_f::CONVERT_OBJ_TO_NATIVE,
                    emit.local_vtype[i as usize] as i64,
                    B::REG_ARG_2,
                );
                r = B::REG_RET;
            }
            if i < max_regs_for_local_vars::<B>() as i32
                && can_use_regs_for_locals(emit)
                && (i != max_regs_for_local_vars::<B>() as i32 - 1
                    || scope.num_pos_args as i32 == max_regs_for_local_vars::<B>() as i32)
            {
                B::mov_reg_reg(&mut emit.as_, reg_local_table::<B>()[i as usize], r);
            } else {
                Self::mov_state_reg(emit, local_idx_local_var(emit, i as usize), r);
            }
        }
        if scope.num_pos_args as i32 >= max_regs_for_local_vars::<B>() as i32 + 1
            && can_use_regs_for_locals(emit)
        {
            let last_local = local_idx_local_var(emit, max_regs_for_local_vars::<B>() - 1);
            B::mov_reg_local(&mut emit.as_, B::REG_LOCAL_LAST, last_local);
        }
        Self::global_exc_entry(emit);
    }

    pub fn start_pass(emit: *mut crate::emit::Emit, pass: PassKind, scope: *mut Scope) {
        let emit = emit_mut::<B>(emit);
        emit.pass = pass as i32;
        unsafe {
            emit.do_viper_types = (*scope).emit_options == EMIT_OPT_VIPER;
            emit.scope = scope;
        }
        emit.stack_size = 0;
        unsafe {
            let num_locals = (*scope).num_locals as usize;
            if emit.local_vtype.len() < num_locals {
                emit.local_vtype.resize(num_locals, VType::PyObj);
            }
            let mut num_args = (*scope).num_pos_args as usize + (*scope).num_kwonly_args as usize;
            if (*scope).scope_flags & MP_SCOPE_FLAG_VARARGS != 0 {
                num_args += 1;
            }
            if (*scope).scope_flags & MP_SCOPE_FLAG_VARKEYWORDS != 0 {
                num_args += 1;
            }
            for i in 0..num_args {
                emit.local_vtype[i] = VType::PyObj;
            }
            if emit.do_viper_types {
                for id in &(*scope).id_info {
                    if id.flags & scope::ID_FLAG_IS_PARAM != 0 {
                        emit.local_vtype[id.local_num as usize] =
                            VType::from_u8(id.flags >> scope::ID_FLAG_VIPER_TYPE_POS);
                    }
                }
            }
            for i in num_args..emit.local_vtype.len() {
                emit.local_vtype[i] = if emit.do_viper_types {
                    VType::Unbound
                } else {
                    VType::PyObj
                };
            }
        }
        for si in &mut emit.stack_info {
            si.kind = StackInfoKind::Value;
            si.vtype = VType::Unbound;
        }
        asmbase::start_pass(
            B::asm_base(&mut emit.as_),
            if pass == PassKind::Emit {
                MP_ASM_PASS_EMIT as i32
            } else {
                MP_ASM_PASS_COMPUTE as i32
            },
        );

        emit.code_state_start = 0;
        if need_global_exc_handler(&emit) {
            emit.code_state_start = SIZEOF_NLR_BUF as u16;
            emit.code_state_start += 1;
            if need_throw_val(&emit) {
                emit.code_state_start += 2;
            } else if need_exc_handler_unwind(&emit) {
                emit.code_state_start += 1;
            }
        }

        let fun_table_off = emit::emit_common_use_const_obj(
            unsafe { &mut *emit.emit_common },
            Obj(crate::nativeglue::fun_table_reloc_base()),
        );

        let scope_ref = unsafe { &*scope };
        if emit.do_viper_types {
            Self::start_pass_viper(emit, scope_ref, fun_table_off);
        } else {
            Self::start_pass_native_py(emit, scope_ref, fun_table_off);
        }
    }

    pub fn end_pass(emit: *mut crate::emit::Emit) -> bool {
        let emit = emit_mut::<B>(emit);
        Self::global_exc_exit(emit);

        if !emit.do_viper_types {
            emit.prelude_offset = B::asm_base(&mut emit.as_).get_code_pos() as i32;
            emit.prelude_ptr_index = unsafe { (*emit.emit_common).ct_cur_child as i32 };
            unsafe {
                let scope = &*emit.scope;
                emit.n_state = generator_n_state(emit);
                let n_state = emit.n_state as usize;
                let n_exc_stack = 0usize;
                Self::prelude_sig_encode(emit, scope);
                Self::prelude_size_encode(emit, emit.n_info as usize, emit.n_cell as usize);
                let info_start = B::asm_base(&mut emit.as_).get_code_pos();
                Self::write_code_info_qstr(emit, scope.simple_name);
                for i in 0..scope.num_pos_args + scope.num_kwonly_args {
                    let mut qst = qstr::from_str("*");
                    for id in &scope.id_info {
                        if id.flags & scope::ID_FLAG_IS_PARAM != 0 && id.local_num == i {
                            qst = id.qst;
                            break;
                        }
                    }
                    Self::write_code_info_qstr(emit, qst);
                }
                emit.n_info = (B::asm_base(&mut emit.as_).get_code_pos() - info_start) as u16;
                let cell_start = B::asm_base(&mut emit.as_).get_code_pos();
                for id in &scope.id_info {
                    if id.kind == IdInfoKind::Cell {
                        asmbase::data(B::asm_base(&mut emit.as_), 1, id.local_num as usize);
                    }
                }
                emit.n_cell = (B::asm_base(&mut emit.as_).get_code_pos() - cell_start) as u16;
            }
        }

        B::end_pass(&mut emit.as_);
        let has_error = unsafe { *emit.error_slot != obj::OBJ_NULL };
        if !has_error {
            debug_assert_eq!(emit.stack_size, 0);
            debug_assert!(emit.exc_stack.is_empty());
        }
        if emit.pass as u8 == PassKind::Emit as u8 {
            unsafe {
                let base = B::asm_base(&mut emit.as_);
                let f = base.get_code();
                let f_len = base.get_code_size() as u32;
                let mut children = (*emit.emit_common).children;
                if !emit.do_viper_types {
                    let prelude_ptr = f.add(emit.prelude_offset as usize);
                    debug_assert_eq!(emit.prelude_ptr_index, (*emit.emit_common).ct_cur_child as i32);
                    if emit.prelude_ptr_index == 0 {
                        children = prelude_ptr as *mut *mut RawCode;
                    } else {
                        let idx = emit.prelude_ptr_index as usize;
                        if children.is_null() {
                            children = malloc::new(idx + 1).expect("native child table");
                        } else {
                            children = malloc::renew(children, idx, idx + 1).expect("native child table");
                        }
                        *children.add(idx) = prelude_ptr as *mut RawCode;
                    }
                }
                emitglue::assign_native(
                    (*emit.scope).raw_code,
                    if emit.do_viper_types {
                        RawCodeKind::NativeViper
                    } else {
                        RawCodeKind::NativePy
                    },
                    f,
                    f_len,
                    children,
                    (*emit.emit_common).ct_cur_child as u16,
                    emit.prelude_offset as u16,
                    (*emit.scope).scope_flags,
                    0,
                    0,
                );
            }
        }
        true
    }

    pub fn adjust_stack_size(emit: *mut crate::emit::Emit, delta: i64) {
        let emit = emit_mut::<B>(emit);
        if delta > 0 {
            Self::ensure_extra_stack(emit, delta as usize);
            for i in 0..delta {
                let si = &mut emit.stack_info[(emit.stack_size + i as i32) as usize];
                si.kind = StackInfoKind::Value;
                si.vtype = if delta == 1 {
                    emit.saved_stack_vtype
                } else {
                    VType::PyObj
                };
            }
        }
        Self::adjust_stack(emit, delta as i32);
    }

    pub fn set_source_line(_emit: *mut crate::emit::Emit, _source_line: usize) {}

    pub fn load_local(emit: *mut crate::emit::Emit, qst: Qstr, local_num: usize, kind: i32) {
        if kind == EMIT_IDOP_LOCAL_DEREF {
            Self::load_deref(emit, qst, local_num);
        } else {
            Self::load_fast(emit, qst, local_num);
        }
    }

    fn load_fast(emit: *mut crate::emit::Emit, qst: Qstr, local_num: usize) {
        let emit = emit_mut::<B>(emit);
        let vtype = emit.local_vtype[local_num];
        if vtype == VType::Unbound {
            let local = qstr::str_data(qst).unwrap_or_else(|| b"<?>".to_vec());
            let mut msg = Vec::with_capacity(local.len() + 28);
            msg.extend_from_slice(b"local '");
            msg.extend_from_slice(&local);
            msg.extend_from_slice(b"' used before type known");
            viper_type_error_msg(emit, &msg);
            return;
        }
        if local_num < max_regs_for_local_vars::<B>() && can_use_regs_for_locals(emit) {
            Self::emit_post_push_reg(emit, vtype, reg_local_table::<B>()[local_num]);
        } else {
            Self::need_reg_single(emit, B::REG_TEMP0, 0);
            Self::mov_reg_state(emit, B::REG_TEMP0, local_idx_local_var(emit, local_num));
            Self::emit_post_push_reg(emit, vtype, B::REG_TEMP0);
        }
        let _ = qst;
    }

    fn load_deref(emit: *mut crate::emit::Emit, qst: Qstr, local_num: usize) {
        Self::need_reg_single(emit_mut::<B>(emit), B::REG_RET, 0);
        Self::load_fast(emit, qst, local_num);
        let emit = emit_mut::<B>(emit);
        let mut vtype = VType::PyObj;
        let reg_base = B::REG_RET;
        Self::emit_pre_pop_reg(emit, &mut vtype, reg_base);
        B::load_reg_reg_offset(&mut emit.as_, B::REG_RET, reg_base, 1);
        Self::emit_post_push_reg(emit, VType::PyObj, B::REG_RET);
    }

    pub fn load_global(emit: *mut crate::emit::Emit, qst: Qstr, kind: i32) {
        let emit = emit_mut::<B>(emit);
        if kind == EMIT_IDOP_GLOBAL_GLOBAL && emit.do_viper_types {
            let native_type = nativeglue::native_type_from_qstr(qst);
            if native_type >= NATIVE_TYPE_BOOL as i32 {
                Self::emit_post_push_imm(emit, VType::BuiltinCast, native_type as i64);
                return;
            }
        }
        Self::emit_call_with_qstr_arg(emit, mp_f::LOAD_NAME + kind as u32, qst, B::REG_ARG_1);
        Self::emit_post_push_reg(emit, VType::PyObj, B::REG_RET);
    }

    pub fn store_local(emit: *mut crate::emit::Emit, qst: Qstr, local_num: usize, kind: i32) {
        if kind == EMIT_IDOP_LOCAL_DEREF {
            Self::store_deref(emit, qst, local_num);
        } else {
            Self::store_fast(emit, qst, local_num);
        }
    }

    fn store_fast(emit: *mut crate::emit::Emit, qst: Qstr, local_num: usize) {
        let emit = emit_mut::<B>(emit);
        let mut vtype = VType::PyObj;
        if local_num < max_regs_for_local_vars::<B>() && can_use_regs_for_locals(emit) {
            Self::emit_pre_pop_reg(emit, &mut vtype, reg_local_table::<B>()[local_num]);
        } else {
            Self::emit_pre_pop_reg(emit, &mut vtype, B::REG_TEMP0);
            Self::mov_state_reg(emit, local_idx_local_var(emit, local_num), B::REG_TEMP0);
        }
        if emit.local_vtype[local_num] == VType::Unbound {
            emit.local_vtype[local_num] = vtype;
        } else if emit.local_vtype[local_num] != vtype {
            viper_type_error_local(emit, qst, emit.local_vtype[local_num], vtype);
        }
        let _ = ();
    }

    fn store_deref(emit: *mut crate::emit::Emit, _qst: Qstr, local_num: usize) {
        {
            let e = emit_mut::<B>(emit);
            Self::need_reg_single(e, B::REG_TEMP0, 0);
            Self::need_reg_single(e, B::REG_TEMP1, 0);
        }
        Self::load_fast(emit, _qst, local_num);
        let e = emit_mut::<B>(emit);
        let mut vtype = VType::PyObj;
        let reg_base = B::REG_TEMP0;
        Self::emit_pre_pop_reg(e, &mut vtype, reg_base);
        let mut vtype_src = VType::PyObj;
        Self::emit_pre_pop_reg(e, &mut vtype_src, B::REG_TEMP1);
        B::store_reg_reg_offset(&mut e.as_, B::REG_TEMP1, reg_base, 1);
    }

    pub fn store_global(emit: *mut crate::emit::Emit, qst: Qstr, kind: i32) {
        let emit = emit_mut::<B>(emit);
        if kind == EMIT_IDOP_GLOBAL_NAME {
            let mut vtype = VType::PyObj;
            Self::emit_pre_pop_reg(emit, &mut vtype, B::REG_ARG_2);
        } else {
            let vtype_val = Self::peek_vtype(emit, 0);
            if vtype_val == VType::PyObj {
                let mut vtype = VType::PyObj;
                Self::emit_pre_pop_reg(emit, &mut vtype, B::REG_ARG_2);
            } else {
                let mut vtype = vtype_val;
                Self::emit_pre_pop_reg(emit, &mut vtype, B::REG_ARG_1);
                Self::emit_call_with_imm_arg(emit, mp_f::CONVERT_NATIVE_TO_OBJ, vtype as i64, B::REG_ARG_2);
                B::mov_reg_reg(&mut emit.as_, B::REG_ARG_2, B::REG_RET);
            }
        }
        Self::emit_call_with_qstr_arg(emit, mp_f::STORE_NAME + kind as u32, qst, B::REG_ARG_1);
    }

    pub fn delete_local(emit: *mut crate::emit::Emit, qst: Qstr, local_num: usize, kind: i32) {
        if kind == EMIT_IDOP_LOCAL_FAST {
            Self::load_const_tok(emit, TokenKind::KwNone);
            Self::store_fast(emit, qst, local_num);
        }
    }

    pub fn delete_global(emit: *mut crate::emit::Emit, qst: Qstr, kind: i32) {
        Self::emit_call_with_qstr_arg(emit_mut::<B>(emit), mp_f::DELETE_NAME + kind as u32, qst, B::REG_ARG_1);
    }

    pub fn label_assign(emit: *mut crate::emit::Emit, l: usize) {
        let emit = emit_mut::<B>(emit);
        let mut is_finally = false;
        if let Some(entry) = emit.exc_stack.last() {
            is_finally = entry.is_finally && entry.label as usize == l;
        }
        if is_finally {
            let mut vtype = VType::PyObj;
            Self::emit_access_stack(emit, 1, &mut vtype, B::REG_TEMP0);
            let exc_val = local_idx_exc_val(emit);
            B::mov_local_reg(&mut emit.as_, exc_val, B::REG_TEMP0);
        }
        Self::need_stack_settled(emit);
        asmbase::label_assign(B::asm_base(&mut emit.as_), l);
        if is_finally {
            Self::leave_exc_stack(emit, false);
        }
    }

    pub fn import(emit: *mut crate::emit::Emit, qst: Qstr, kind: i32) {
        match kind {
            EMIT_IMPORT_NAME => Self::import_name(emit, qst),
            EMIT_IMPORT_FROM => Self::import_from(emit, qst),
            _ => Self::import_star(emit),
        }
    }

    fn import_name(emit: *mut crate::emit::Emit, qst: Qstr) {
        let emit = emit_mut::<B>(emit);
        let orig = emit.do_viper_types;
        emit.do_viper_types = false;
        let mut vt1 = VType::PyObj;
        let mut vt2 = VType::PyObj;
        Self::emit_pre_pop_reg(emit, &mut vt1, B::REG_ARG_3);
        Self::emit_pre_pop_reg(emit, &mut vt2, B::REG_ARG_2);
        emit.do_viper_types = orig;
        Self::emit_call_with_qstr_arg(emit, mp_f::IMPORT_NAME, qst, B::REG_ARG_1);
        Self::emit_post_push_reg(emit, VType::PyObj, B::REG_RET);
    }

    fn import_from(emit: *mut crate::emit::Emit, qst: Qstr) {
        let emit = emit_mut::<B>(emit);
        let mut vtype = VType::PyObj;
        Self::emit_access_stack(emit, 1, &mut vtype, B::REG_ARG_1);
        Self::emit_call_with_qstr_arg(emit, mp_f::IMPORT_FROM, qst, B::REG_ARG_2);
        Self::emit_post_push_reg(emit, VType::PyObj, B::REG_RET);
    }

    fn import_star(emit: *mut crate::emit::Emit) {
        let emit = emit_mut::<B>(emit);
        let mut vtype = VType::PyObj;
        Self::emit_pre_pop_reg(emit, &mut vtype, B::REG_ARG_1);
        Self::emit_call(emit, mp_f::IMPORT_ALL);
    }

    pub fn load_const_tok(emit: *mut crate::emit::Emit, tok: TokenKind) {
        if tok == TokenKind::Ellipsis {
            Self::load_const_obj(emit, obj::OBJ_NULL);
        } else if tok == TokenKind::KwNone {
            Self::emit_post_push_imm(emit_mut::<B>(emit), VType::PtrNone, 0);
        } else {
            Self::emit_post_push_imm(emit_mut::<B>(emit), VType::Bool, if tok == TokenKind::KwFalse { 0 } else { 1 });
        }
    }

    pub fn load_const_small_int(emit: *mut crate::emit::Emit, arg: i64) {
        Self::emit_post_push_imm(emit_mut::<B>(emit), VType::Int, arg);
    }

    pub fn load_const_str(emit: *mut crate::emit::Emit, qst: Qstr) {
        let emit = emit_mut::<B>(emit);
        Self::need_reg_single(emit, B::REG_TEMP0, 0);
        Self::mov_reg_qstr_obj(emit, B::REG_TEMP0, qst);
        Self::emit_post_push_reg(emit, VType::PyObj, B::REG_TEMP0);
    }

    pub fn load_const_obj(emit: *mut crate::emit::Emit, obj_in: Obj) {
        let emit = emit_mut::<B>(emit);
        Self::need_reg_single(emit, B::REG_TEMP0, 0);
        Self::load_reg_with_object(emit, B::REG_TEMP0, obj_in);
        Self::emit_post_push_reg(emit, VType::PyObj, B::REG_TEMP0);
    }

    pub fn load_null(emit: *mut crate::emit::Emit) {
        Self::emit_post_push_imm(emit_mut::<B>(emit), VType::PyObj, 0);
    }

    pub fn load_method(emit: *mut crate::emit::Emit, qst: Qstr, is_super: bool) {
        let emit = emit_mut::<B>(emit);
        Self::need_stack_settled(emit);
        if is_super {
            Self::emit_get_stack_pointer_to_reg_for_pop(emit, B::REG_ARG_2, 3);
            Self::emit_get_stack_pointer_to_reg_for_push(emit, B::REG_ARG_2, 2);
            Self::emit_call_with_qstr_arg(emit, mp_f::LOAD_SUPER_METHOD, qst, B::REG_ARG_1);
        } else {
            let mut vtype = VType::PyObj;
            Self::emit_pre_pop_reg(emit, &mut vtype, B::REG_ARG_1);
            Self::emit_get_stack_pointer_to_reg_for_push(emit, B::REG_ARG_3, 2);
            Self::emit_call_with_qstr_arg(emit, mp_f::LOAD_METHOD, qst, B::REG_ARG_2);
        }
    }

    pub fn load_build_class(emit: *mut crate::emit::Emit) {
        Self::emit_call(emit_mut::<B>(emit), mp_f::LOAD_BUILD_CLASS);
        Self::emit_post_push_reg(emit_mut::<B>(emit), VType::PyObj, B::REG_RET);
    }

    pub fn subscr(emit: *mut crate::emit::Emit, kind: i32) {
        match kind {
            EMIT_SUBSCR_LOAD => Self::load_subscr(emit),
            EMIT_SUBSCR_STORE => Self::store_subscr(emit),
            _ => Self::delete_subscr(emit),
        }
    }

    fn load_subscr(emit: *mut crate::emit::Emit) {
        let emit = emit_mut::<B>(emit);
        let mut vtype_base = Self::peek_vtype(emit, 1);
        if vtype_base == VType::PyObj {
            let vtype_index = Self::peek_vtype(emit, 0);
            if vtype_index == VType::PyObj {
                let mut vt = vtype_index;
                Self::emit_pre_pop_reg(emit, &mut vt, B::REG_ARG_2);
            } else {
                let mut vt = vtype_index;
                Self::emit_pre_pop_reg(emit, &mut vt, B::REG_ARG_1);
                Self::emit_call_with_imm_arg(emit, mp_f::CONVERT_NATIVE_TO_OBJ, vt as i64, B::REG_ARG_2);
                B::mov_reg_reg(&mut emit.as_, B::REG_ARG_2, B::REG_RET);
            }
            let mut vt_base = vtype_base;
            Self::emit_pre_pop_reg(emit, &mut vt_base, B::REG_ARG_1);
            Self::emit_call_with_imm_arg(emit, mp_f::OBJ_SUBSCR, obj::OBJ_SENTINEL.0 as i64, B::REG_ARG_3);
            Self::emit_post_push_reg(emit, VType::PyObj, B::REG_RET);
            return;
        }

        let top = *Self::peek_stack(emit, 0);
        if top.vtype == VType::Int && top.kind == StackInfoKind::Imm {
            let index_value = top.u_imm;
            Self::emit_pre_pop_discard(emit);
            let mut reg_base = B::REG_ARG_1;
            let reg_index = B::REG_ARG_2;
            Self::emit_pre_pop_reg_flexible(emit, &mut vtype_base, &mut reg_base, reg_index, reg_index);
            Self::need_reg_single(emit, B::REG_RET, 0);
            match vtype_base {
                VType::Ptr8 => {
                    if B::HAS_ASM_LOAD8_REG_REG_OFFSET {
                        B::load8_reg_reg_offset(&mut emit.as_, B::REG_RET, reg_base, index_value as i32);
                    } else if index_value != 0 {
                        Self::need_reg_single(emit, reg_index, 0);
                        B::mov_reg_imm(&mut emit.as_, reg_index, index_value as usize);
                        B::add_reg_reg(&mut emit.as_, reg_index, reg_base);
                        B::load8_reg_reg(&mut emit.as_, B::REG_RET, reg_index);
                    } else {
                        B::load8_reg_reg(&mut emit.as_, B::REG_RET, reg_base);
                    }
                }
                VType::Ptr16 => {
                    if B::HAS_ASM_LOAD16_REG_REG_OFFSET {
                        B::load16_reg_reg_offset(&mut emit.as_, B::REG_RET, reg_base, index_value as i32);
                    } else if index_value != 0 {
                        Self::need_reg_single(emit, reg_index, 0);
                        B::mov_reg_imm(&mut emit.as_, reg_index, (index_value << 1) as usize);
                        B::add_reg_reg(&mut emit.as_, reg_index, reg_base);
                        B::load16_reg_reg(&mut emit.as_, B::REG_RET, reg_index);
                    } else {
                        B::load16_reg_reg(&mut emit.as_, B::REG_RET, reg_base);
                    }
                }
                VType::Ptr32 => {
                    if B::HAS_ASM_LOAD32_REG_REG_OFFSET {
                        B::load32_reg_reg_offset(&mut emit.as_, B::REG_RET, reg_base, index_value as i32);
                    } else if index_value != 0 {
                        Self::need_reg_single(emit, reg_index, 0);
                        B::mov_reg_imm(&mut emit.as_, reg_index, (index_value << 2) as usize);
                        B::add_reg_reg(&mut emit.as_, reg_index, reg_base);
                        B::load32_reg_reg(&mut emit.as_, B::REG_RET, reg_index);
                    } else {
                        B::load32_reg_reg(&mut emit.as_, B::REG_RET, reg_base);
                    }
                }
                _ => viper_type_error_vtype(emit, b"can't load from '", vtype_base),
            }
        } else {
            let mut vtype_index = VType::Int;
            let mut reg_index = B::REG_ARG_2;
            Self::emit_pre_pop_reg_flexible(emit, &mut vtype_index, &mut reg_index, B::REG_ARG_1, B::REG_ARG_1);
            let mut vt_base = vtype_base;
            Self::emit_pre_pop_reg(emit, &mut vt_base, B::REG_ARG_1);
            Self::need_reg_single(emit, B::REG_RET, 0);
            if !matches!(vtype_index, VType::Int | VType::Uint) {
                viper_type_error_vtype_suffix(emit, b"can't load with '", vtype_index, b" index");
            } else {
                match vt_base {
                    VType::Ptr8 => {
                        if B::HAS_ASM_LOAD8_REG_REG_REG {
                            B::add_reg_reg(&mut emit.as_, B::REG_ARG_1, reg_index);
                            B::load8_reg_reg(&mut emit.as_, B::REG_RET, B::REG_ARG_1);
                        } else {
                            B::add_reg_reg(&mut emit.as_, B::REG_ARG_1, reg_index);
                            B::load8_reg_reg(&mut emit.as_, B::REG_RET, B::REG_ARG_1);
                        }
                    }
                    VType::Ptr16 => {
                        if B::HAS_ASM_LOAD16_REG_REG_REG {
                            B::add_reg_reg(&mut emit.as_, B::REG_ARG_1, reg_index);
                            B::add_reg_reg(&mut emit.as_, B::REG_ARG_1, reg_index);
                            B::load16_reg_reg(&mut emit.as_, B::REG_RET, B::REG_ARG_1);
                        } else {
                            B::add_reg_reg(&mut emit.as_, B::REG_ARG_1, reg_index);
                            B::add_reg_reg(&mut emit.as_, B::REG_ARG_1, reg_index);
                            B::load16_reg_reg(&mut emit.as_, B::REG_RET, B::REG_ARG_1);
                        }
                    }
                    VType::Ptr32 => {
                        if B::HAS_ASM_LOAD32_REG_REG_REG {
                            B::add_reg_reg(&mut emit.as_, B::REG_ARG_1, reg_index);
                            B::add_reg_reg(&mut emit.as_, B::REG_ARG_1, reg_index);
                            B::add_reg_reg(&mut emit.as_, B::REG_ARG_1, reg_index);
                            B::add_reg_reg(&mut emit.as_, B::REG_ARG_1, reg_index);
                            B::load32_reg_reg(&mut emit.as_, B::REG_RET, B::REG_ARG_1);
                        } else {
                            B::add_reg_reg(&mut emit.as_, B::REG_ARG_1, reg_index);
                            B::add_reg_reg(&mut emit.as_, B::REG_ARG_1, reg_index);
                            B::add_reg_reg(&mut emit.as_, B::REG_ARG_1, reg_index);
                            B::add_reg_reg(&mut emit.as_, B::REG_ARG_1, reg_index);
                            B::load32_reg_reg(&mut emit.as_, B::REG_RET, B::REG_ARG_1);
                        }
                    }
                    _ => viper_type_error_vtype(emit, b"can't load from '", vtype_base),
                }
            }
        }
        Self::emit_post_push_reg(emit, VType::Int, B::REG_RET);
    }

    fn store_subscr(emit: *mut crate::emit::Emit) {
        let emit = emit_mut::<B>(emit);
        let mut vtype_base = Self::peek_vtype(emit, 1);
        if vtype_base == VType::PyObj {
            let vtype_index = Self::peek_vtype(emit, 0);
            let vtype_value = Self::peek_vtype(emit, 2);
            if vtype_index != VType::PyObj || vtype_value != VType::PyObj {
                Self::adjust_stack(emit, 3);
            }
            let mut vt_i = vtype_index;
            let mut vt_b = vtype_base;
            let mut vt_v = vtype_value;
            Self::emit_pre_pop_reg_reg_reg(emit, &mut vt_i, B::REG_ARG_2, &mut vt_b, B::REG_ARG_1, &mut vt_v, B::REG_ARG_3);
            Self::emit_call(emit, mp_f::OBJ_SUBSCR);
            return;
        }

        let top = *Self::peek_stack(emit, 0);
        if top.vtype == VType::Int && top.kind == StackInfoKind::Imm {
            let index_value = top.u_imm;
            Self::emit_pre_pop_discard(emit);
            let mut reg_base = B::REG_ARG_1;
            let reg_index = B::REG_ARG_2;
            let mut reg_value = B::REG_ARG_3;
            Self::emit_pre_pop_reg_flexible(emit, &mut vtype_base, &mut reg_base, reg_index, reg_value);
            let mut vtype_value = Self::peek_vtype(emit, 0);
            if B::N_X64 || B::N_X86 {
                Self::emit_pre_pop_reg(emit, &mut vtype_value, reg_value);
            } else {
                Self::emit_pre_pop_reg_flexible(emit, &mut vtype_value, &mut reg_value, reg_base, reg_index);
            }
            if !matches!(vtype_value, VType::Bool | VType::Int | VType::Uint) {
                viper_type_error_vtype(emit, b"can't store '", vtype_value);
            } else {
                Self::viper_store_ptr_index_imm(emit, vtype_base, reg_base, reg_index, reg_value, index_value);
            }
        } else {
            let mut reg_index = B::REG_ARG_2;
            let mut vtype_index = VType::Int;
            Self::emit_pre_pop_reg_flexible(emit, &mut vtype_index, &mut reg_index, B::REG_ARG_1, B::REG_ARG_3);
            let mut vt_base = vtype_base;
            Self::emit_pre_pop_reg(emit, &mut vt_base, B::REG_ARG_1);
            if !matches!(vtype_index, VType::Int | VType::Uint) {
                viper_type_error_vtype_suffix(emit, b"can't store with '", vtype_index, b" index");
            } else {
                let mut reg_value = B::REG_ARG_3;
                let mut vtype_value = Self::peek_vtype(emit, 0);
                if B::N_X64 || B::N_X86 {
                    Self::emit_pre_pop_reg(emit, &mut vtype_value, reg_value);
                } else {
                    Self::emit_pre_pop_reg_flexible(emit, &mut vtype_value, &mut reg_value, B::REG_ARG_1, reg_index);
                }
                if !matches!(vtype_value, VType::Bool | VType::Int | VType::Uint) {
                    viper_type_error_vtype(emit, b"can't store '", vtype_value);
                } else {
                    Self::viper_store_ptr_index_reg(emit, vt_base, B::REG_ARG_1, reg_index, reg_value);
                }
            }
        }
    }

    fn viper_store_ptr_index_imm(
        emit: &mut EmitNative<B>,
        vtype_base: VType,
        mut reg_base: i32,
        reg_index: i32,
        reg_value: i32,
        index_value: i64,
    ) {
        match vtype_base {
            VType::Ptr8 => {
                if B::HAS_ASM_STORE8_REG_REG_OFFSET {
                    B::store8_reg_reg_offset(&mut emit.as_, reg_value, reg_base, index_value as i32);
                } else if index_value != 0 {
                    B::mov_reg_imm(&mut emit.as_, reg_index, index_value as usize);
                    B::add_reg_reg(&mut emit.as_, reg_index, reg_base);
                    reg_base = reg_index;
                    B::store8_reg_reg(&mut emit.as_, reg_value, reg_base);
                } else {
                    B::store8_reg_reg(&mut emit.as_, reg_value, reg_base);
                }
            }
            VType::Ptr16 => {
                if B::HAS_ASM_STORE16_REG_REG_OFFSET {
                    B::store16_reg_reg_offset(&mut emit.as_, reg_value, reg_base, index_value as i32);
                } else if index_value != 0 {
                    B::mov_reg_imm(&mut emit.as_, reg_index, (index_value << 1) as usize);
                    B::add_reg_reg(&mut emit.as_, reg_index, reg_base);
                    reg_base = reg_index;
                    B::store16_reg_reg(&mut emit.as_, reg_value, reg_base);
                } else {
                    B::store16_reg_reg(&mut emit.as_, reg_value, reg_base);
                }
            }
            VType::Ptr32 => {
                if B::HAS_ASM_STORE32_REG_REG_OFFSET {
                    B::store32_reg_reg_offset(&mut emit.as_, reg_value, reg_base, index_value as i32);
                } else if index_value != 0 {
                    B::mov_reg_imm(&mut emit.as_, reg_index, (index_value << 2) as usize);
                    B::add_reg_reg(&mut emit.as_, reg_index, reg_base);
                    reg_base = reg_index;
                    B::store32_reg_reg(&mut emit.as_, reg_value, reg_base);
                } else {
                    B::store32_reg_reg(&mut emit.as_, reg_value, reg_base);
                }
            }
            _ => viper_type_error_vtype(emit, b"can't store to '", vtype_base),
        }
    }

    fn viper_store_ptr_index_reg(
        emit: &mut EmitNative<B>,
        vtype_base: VType,
        reg_base: i32,
        reg_index: i32,
        reg_value: i32,
    ) {
        match vtype_base {
            VType::Ptr8 => {
                if B::HAS_ASM_STORE8_REG_REG_REG {
                    B::add_reg_reg(&mut emit.as_, reg_base, reg_index);
                    B::store8_reg_reg(&mut emit.as_, reg_value, reg_base);
                } else {
                    B::add_reg_reg(&mut emit.as_, reg_base, reg_index);
                    B::store8_reg_reg(&mut emit.as_, reg_value, reg_base);
                }
            }
            VType::Ptr16 => {
                if B::HAS_ASM_STORE16_REG_REG_REG {
                    B::add_reg_reg(&mut emit.as_, reg_base, reg_index);
                    B::add_reg_reg(&mut emit.as_, reg_base, reg_index);
                    B::store16_reg_reg(&mut emit.as_, reg_value, reg_base);
                } else {
                    B::add_reg_reg(&mut emit.as_, reg_base, reg_index);
                    B::add_reg_reg(&mut emit.as_, reg_base, reg_index);
                    B::store16_reg_reg(&mut emit.as_, reg_value, reg_base);
                }
            }
            VType::Ptr32 => {
                if B::HAS_ASM_STORE32_REG_REG_REG {
                    B::add_reg_reg(&mut emit.as_, reg_base, reg_index);
                    B::add_reg_reg(&mut emit.as_, reg_base, reg_index);
                    B::add_reg_reg(&mut emit.as_, reg_base, reg_index);
                    B::add_reg_reg(&mut emit.as_, reg_base, reg_index);
                    B::store32_reg_reg(&mut emit.as_, reg_value, reg_base);
                } else {
                    B::add_reg_reg(&mut emit.as_, reg_base, reg_index);
                    B::add_reg_reg(&mut emit.as_, reg_base, reg_index);
                    B::add_reg_reg(&mut emit.as_, reg_base, reg_index);
                    B::add_reg_reg(&mut emit.as_, reg_base, reg_index);
                    B::store32_reg_reg(&mut emit.as_, reg_value, reg_base);
                }
            }
            _ => viper_type_error_vtype(emit, b"can't store to '", vtype_base),
        }
    }

    fn emit_pre_pop_reg_reg_reg(
        emit: &mut EmitNative<B>,
        vtype_a: &mut VType,
        reg_a: i32,
        vtype_b: &mut VType,
        reg_b: i32,
        vtype_c: &mut VType,
        reg_c: i32,
    ) {
        Self::emit_pre_pop_reg(emit, vtype_a, reg_a);
        Self::emit_pre_pop_reg(emit, vtype_b, reg_b);
        Self::emit_pre_pop_reg(emit, vtype_c, reg_c);
    }

    fn delete_subscr(emit: *mut crate::emit::Emit) {
        let emit = emit_mut::<B>(emit);
        let mut vtype_index = VType::PyObj;
        let mut vtype_base = VType::PyObj;
        Self::emit_pre_pop_reg_reg(emit, &mut vtype_index, B::REG_ARG_2, &mut vtype_base, B::REG_ARG_1);
        Self::emit_call_with_imm_arg(emit, mp_f::OBJ_SUBSCR, 0, B::REG_ARG_3);
    }

    pub fn attr(emit: *mut crate::emit::Emit, qst: Qstr, kind: i32) {
        let emit = emit_mut::<B>(emit);
        match kind {
            EMIT_ATTR_LOAD => {
                let mut vtype = VType::PyObj;
                Self::emit_pre_pop_reg(emit, &mut vtype, B::REG_ARG_1);
                Self::emit_call_with_qstr_arg(emit, mp_f::LOAD_ATTR, qst, B::REG_ARG_2);
                Self::emit_post_push_reg(emit, VType::PyObj, B::REG_RET);
            }
            EMIT_ATTR_STORE => {
                let mut vtype_base = VType::PyObj;
                let mut vtype_val = VType::PyObj;
                Self::emit_pre_pop_reg(emit, &mut vtype_val, B::REG_ARG_3);
                Self::emit_pre_pop_reg(emit, &mut vtype_base, B::REG_ARG_1);
                Self::emit_call_with_qstr_arg(emit, mp_f::STORE_ATTR, qst, B::REG_ARG_2);
            }
            _ => {
                let mut vtype = VType::PyObj;
                Self::emit_pre_pop_reg(emit, &mut vtype, B::REG_ARG_1);
                B::clr_reg(&mut emit.as_, B::REG_ARG_3);
                Self::emit_call_with_qstr_arg(emit, mp_f::STORE_ATTR, qst, B::REG_ARG_2);
            }
        }
    }

    pub fn dup_top(emit: *mut crate::emit::Emit) {
        let emit = emit_mut::<B>(emit);
        Self::need_stack_settled(emit);
        let mut vtype = VType::PyObj;
        Self::emit_pre_pop_reg(emit, &mut vtype, B::REG_TEMP0);
        Self::emit_post_push_reg(emit, vtype, B::REG_TEMP0);
        Self::emit_post_push_reg(emit, vtype, B::REG_TEMP0);
    }

    pub fn dup_top_two(emit: *mut crate::emit::Emit) {
        let emit = emit_mut::<B>(emit);
        let s1 = *Self::peek_stack(emit, 0);
        let s0 = *Self::peek_stack(emit, 1);
        Self::ensure_extra_stack(emit, 2);
        emit.stack_info[emit.stack_size as usize] = s0;
        emit.stack_info[emit.stack_size as usize + 1] = s1;
        Self::adjust_stack(emit, 2);
    }

    pub fn pop_top(emit: *mut crate::emit::Emit) {
        Self::emit_pre_pop_discard(emit_mut::<B>(emit));
    }

    pub fn rot_two(emit: *mut crate::emit::Emit) {
        let emit = emit_mut::<B>(emit);
        let mut vtype0 = VType::PyObj;
        let mut vtype1 = VType::PyObj;
        Self::emit_pre_pop_reg_reg(emit, &mut vtype0, B::REG_TEMP0, &mut vtype1, B::REG_TEMP1);
        Self::emit_post_push_reg(emit, vtype0, B::REG_TEMP0);
        Self::emit_post_push_reg(emit, vtype1, B::REG_TEMP1);
    }

    pub fn rot_three(emit: *mut crate::emit::Emit) {
        let emit = emit_mut::<B>(emit);
        Self::need_stack_settled(emit);
        let mut vtype0 = VType::PyObj;
        let mut vtype1 = VType::PyObj;
        let mut vtype2 = VType::PyObj;
        Self::emit_pre_pop_reg_reg_reg(
            emit,
            &mut vtype0,
            B::REG_TEMP0,
            &mut vtype1,
            B::REG_TEMP1,
            &mut vtype2,
            B::REG_TEMP2,
        );
        Self::emit_post_push_reg(emit, vtype0, B::REG_TEMP0);
        Self::emit_post_push_reg(emit, vtype2, B::REG_TEMP2);
        Self::emit_post_push_reg(emit, vtype1, B::REG_TEMP1);
    }

    pub fn jump(emit: *mut crate::emit::Emit, label: usize) {
        let emit = emit_mut::<B>(emit);
        Self::need_stack_settled(emit);
        B::jump(&mut emit.as_, label);
        B::asm_base(&mut emit.as_).suppress_code();
    }

    pub fn pop_jump_if(emit: *mut crate::emit::Emit, cond: bool, label: usize) {
        Self::jump_helper(emit, cond, label, true);
    }

    pub fn jump_if_or_pop(emit: *mut crate::emit::Emit, cond: bool, label: usize) {
        Self::jump_helper(emit, cond, label, false);
    }

    fn jump_helper(emit: *mut crate::emit::Emit, cond: bool, label: usize, pop: bool) {
        let emit = emit_mut::<B>(emit);
        let vtype = Self::peek_vtype(emit, 0);
        if vtype == VType::PyObj {
            let mut vt = VType::PyObj;
            Self::emit_pre_pop_reg(emit, &mut vt, B::REG_ARG_1);
            if !pop {
                Self::adjust_stack(emit, 1);
            }
            Self::emit_call(emit, mp_f::OBJ_IS_TRUE);
        } else {
            let mut vt = vtype;
            Self::emit_pre_pop_reg(emit, &mut vt, B::REG_RET);
            if !pop {
                Self::adjust_stack(emit, 1);
            }
            if !(matches!(vtype, VType::Bool | VType::Int | VType::Uint)) {
                viper_type_error_vtype_suffix(emit, b"can't implicitly convert '", vtype, b" to 'bool'");
                return;
            }
        }
        if !pop {
            emit.saved_stack_vtype = vtype;
        }
        Self::need_stack_settled(emit);
        if cond {
            B::jump_if_reg_nonzero(&mut emit.as_, B::REG_RET, label, vtype == VType::PyObj);
        } else {
            B::jump_if_reg_zero(&mut emit.as_, B::REG_RET, label, vtype == VType::PyObj);
        }
        if !pop {
            Self::adjust_stack(emit, -1);
        }
    }

    pub fn unwind_jump(emit: *mut crate::emit::Emit, label: usize, except_depth: usize) {
        let mut jump_label = label & !EMIT_BREAK_FROM_FOR as usize;
        if except_depth > 0 {
            let e = emit_mut::<B>(emit);
            let n = e.exc_stack.len();
            let mut first_finally_idx: Option<usize> = None;
            let mut prev_finally_idx: Option<usize> = None;
            let mut e_idx = n;
            for _ in 0..except_depth {
                if e_idx == 0 {
                    break;
                }
                e_idx -= 1;
                let entry = &e.exc_stack[e_idx];
                if entry.is_finally && entry.is_active {
                    if first_finally_idx.is_none() {
                        first_finally_idx = Some(e_idx);
                    }
                    if let Some(prev) = prev_finally_idx {
                        e.exc_stack[prev].unwind_label = e.exc_stack[e_idx].label;
                    }
                    prev_finally_idx = Some(e_idx);
                }
            }
            if prev_finally_idx.is_none() {
                if e_idx == 0 {
                    B::clr_reg(&mut e.as_, B::REG_RET);
                } else {
                    B::mov_reg_pcrel(&mut e.as_, B::REG_RET, e.exc_stack[e_idx - 1].label as usize);
                }
                let exc_pc = local_idx_exc_handler_pc(e);
                B::mov_local_reg(&mut e.as_, exc_pc, B::REG_RET);
            } else {
                let prev = prev_finally_idx.unwrap();
                e.exc_stack[prev].unwind_label = UNWIND_LABEL_DO_FINAL_UNWIND;
                B::mov_reg_pcrel(&mut e.as_, B::REG_RET, jump_label);
                let exc_unwind = local_idx_exc_handler_unwind(e);
                B::mov_local_reg(&mut e.as_, exc_unwind, B::REG_RET);
                B::mov_reg_imm(&mut e.as_, B::REG_RET, 0);
                let exc_val = local_idx_exc_val(e);
                B::mov_local_reg(&mut e.as_, exc_val, B::REG_RET);
                jump_label = e.exc_stack[first_finally_idx.unwrap()].label as usize;
            }
        }
        Self::jump(emit, jump_label);
    }

    pub fn setup_block(emit: *mut crate::emit::Emit, label: usize, kind: i32) {
        let emit = emit_mut::<B>(emit);
        if kind == EMIT_SETUP_BLOCK_WITH {
            Self::setup_with(emit, label);
        } else {
            Self::need_stack_settled(emit);
            Self::push_exc_stack(emit, label, kind == EMIT_SETUP_BLOCK_FINALLY);
        }
    }

    pub fn with_cleanup(emit: *mut crate::emit::Emit, label: usize) {
        let e = emit_mut::<B>(emit);
        Self::leave_exc_stack(e, false);
        Self::adjust_stack(e, -1);
        let label_slot = unsafe { *e.label_slot };
        asmbase::label_assign(B::asm_base(&mut e.as_), label_slot + 2);
        Self::emit_post_push_imm(e, VType::PtrNone, 0);
        Self::emit_post_push_imm(e, VType::PtrNone, 0);
        Self::emit_post_push_imm(e, VType::PtrNone, 0);
        Self::emit_get_stack_pointer_to_reg_for_pop(e, B::REG_ARG_3, 5);
        Self::emit_call_with_2_imm_args(e, mp_f::CALL_METHOD_N_KW, 3, B::REG_ARG_1, 0, B::REG_ARG_2);
        Self::jump(emit, label_slot);
        asmbase::label_assign(B::asm_base(&mut e.as_), label);
        Self::leave_exc_stack(e, true);
        Self::adjust_stack(e, 2);
        let exc_val = local_idx_exc_val(e);
        B::mov_reg_local(&mut e.as_, B::REG_ARG_1, exc_val);
        B::jump_if_reg_zero(&mut e.as_, B::REG_ARG_1, label_slot + 2, false);
        B::load_reg_reg_offset(&mut e.as_, B::REG_ARG_2, B::REG_ARG_1, 0);
        Self::emit_post_push_reg(e, VType::PyObj, B::REG_ARG_2);
        Self::emit_post_push_reg(e, VType::PyObj, B::REG_ARG_1);
        Self::emit_post_push_imm(e, VType::PtrNone, 0);
        Self::emit_get_stack_pointer_to_reg_for_pop(e, B::REG_ARG_3, 5);
        Self::emit_call_with_2_imm_args(e, mp_f::CALL_METHOD_N_KW, 3, B::REG_ARG_1, 0, B::REG_ARG_2);
        if B::REG_ARG_1 != B::REG_RET {
            B::mov_reg_reg(&mut e.as_, B::REG_ARG_1, B::REG_RET);
        }
        Self::emit_call(e, mp_f::OBJ_IS_TRUE);
        B::jump_if_reg_zero(&mut e.as_, B::REG_RET, label_slot + 1, true);
        asmbase::label_assign(B::asm_base(&mut e.as_), label_slot);
        B::mov_local_mp_obj_null(&mut e.as_, exc_val, B::REG_TEMP0);
        asmbase::label_assign(B::asm_base(&mut e.as_), label_slot + 1);
        Self::adjust_stack(e, 1);
    }

    pub fn async_with_setup_finally(
        emit: *mut crate::emit::Emit,
        label_aexit_no_exc: usize,
        label_finally_block: usize,
        label_ret_unwind_jump: usize,
    ) {
        // Match py/emitnative.c: case-1 dummy/None padding, then finally entry via EXC_VAL.
        Self::adjust_stack(emit_mut::<B>(emit), 1);
        Self::rot_two(emit);
        Self::load_const_tok(emit, TokenKind::KwNone);
        Self::rot_two(emit);
        Self::jump(emit, label_aexit_no_exc);
        Self::adjust_stack(emit_mut::<B>(emit), -1);
        Self::label_assign(emit, label_finally_block);
        let e = emit_mut::<B>(emit);
        let exc_val = local_idx_exc_val(e);
        B::mov_reg_local(&mut e.as_, B::REG_ARG_1, exc_val);
        // Return/unwind (case 3): exc_val is null — jump before pushing so the
        // (ctx, X, INT) UNWIND_JUMP stack layout stays intact for rot_three.
        B::jump_if_reg_zero(&mut e.as_, B::REG_ARG_1, label_ret_unwind_jump, false);
        Self::emit_pre_pop_discard(e);
        Self::emit_post_push_reg(e, VType::PyObj, B::REG_ARG_1);
    }

    /// Compile-only: native stack tracker must see `(ctx, X, INT)` at `l_ret_unwind_jump`.
    pub fn async_with_ret_unwind_enter(emit: *mut crate::emit::Emit) {
        let e = emit_mut::<B>(emit);
        let Some(base) = Self::innermost_active_finally_sp_index(e) else {
            return;
        };
        let target = base as i32 + 3;
        while e.stack_size < target {
            let i = e.stack_size as usize;
            Self::ensure_extra_stack(e, 1);
            e.stack_info[i].kind = StackInfoKind::Value;
            e.stack_info[i].vtype = VType::PyObj;
            e.stack_size += 1;
        }
    }

    pub fn end_finally(emit: *mut crate::emit::Emit) {
        let e = emit_mut::<B>(emit);
        let exit_label = e.exit_label;
        let label_slot = unsafe { *e.label_slot };
        let is_gen = unsafe { (*e.scope).scope_flags & MP_SCOPE_FLAG_GENERATOR != 0 };
        if is_gen {
            if let Some(x_slot) = generator_return_x_slot_if_active(e) {
                B::load_reg_reg_offset(
                    &mut e.as_,
                    B::REG_TEMP2,
                    B::REG_GENERATOR_STATE,
                    OFFSETOF_CODE_STATE_SP as i32,
                );
                Self::mov_reg_state_addr(e, B::REG_TEMP1, x_slot);
                B::jump_if_reg_eq(&mut e.as_, B::REG_TEMP2, B::REG_TEMP1, label_slot);
            }
        }
        Self::emit_pre_pop_discard(e);
        let exc_val = local_idx_exc_val(e);
        B::mov_reg_local(&mut e.as_, B::REG_ARG_1, exc_val);
        Self::emit_call(e, mp_f::NATIVE_RAISE);
        if e.exc_stack.is_empty() {
            return;
        }
        let entry = Self::pop_exc_stack(e);
        if entry.unwind_label != UNWIND_LABEL_UNUSED {
            let exc_unwind = local_idx_exc_handler_unwind(e);
            B::mov_reg_local(&mut e.as_, B::REG_RET, exc_unwind);
            B::jump_if_reg_zero(&mut e.as_, B::REG_RET, label_slot + 1, false);
            if entry.unwind_label == UNWIND_LABEL_DO_FINAL_UNWIND {
                B::jump_reg(&mut e.as_, B::REG_RET);
            } else {
                Self::jump(emit, entry.unwind_label as usize);
            }
            asmbase::label_assign(B::asm_base(&mut e.as_), label_slot + 1);
        }
        if !is_gen {
            return;
        }
        asmbase::label_assign(B::asm_base(&mut e.as_), label_slot);
        if !e.exc_stack.is_empty() {
            Self::pop_exc_stack(e);
        }
        let ret_local = local_idx_ret_val(e);
        B::mov_reg_imm(&mut e.as_, B::REG_TEMP0, MP_VM_RETURN_NORMAL);
        B::mov_local_reg(&mut e.as_, ret_local, B::REG_TEMP0);
        Self::unwind_jump(emit, exit_label, 0);
    }

    pub fn get_iter(emit: *mut crate::emit::Emit, use_stack: bool) {
        let emit = emit_mut::<B>(emit);
        let mut vtype = VType::PyObj;
        Self::emit_pre_pop_reg(emit, &mut vtype, B::REG_ARG_1);
        if !use_stack {
            B::mov_reg_imm(&mut emit.as_, B::REG_ARG_2, 0);
        }
        Self::emit_call(emit, mp_f::NATIVE_GETITER);
        if !use_stack {
            Self::emit_post_push_reg(emit, VType::PyObj, B::REG_RET);
        }
    }

    pub fn for_iter(emit: *mut crate::emit::Emit, label: usize) {
        let emit = emit_mut::<B>(emit);
        Self::emit_call(emit, mp_f::NATIVE_ITERNEXT);
        B::jump_if_reg_zero(&mut emit.as_, B::REG_RET, label, false);
        Self::emit_post_push_reg(emit, VType::PyObj, B::REG_RET);
    }

    pub fn for_iter_end(emit: *mut crate::emit::Emit) {
        Self::adjust_stack(emit_mut::<B>(emit), -MP_OBJ_ITER_BUF_NSLOTS);
    }

    pub fn pop_except_jump(emit: *mut crate::emit::Emit, label: usize, within_exc_handler: bool) {
        let e = emit_mut::<B>(emit);
        if within_exc_handler {
            let exc_val = local_idx_exc_val(e);
            B::mov_local_mp_obj_null(&mut e.as_, exc_val, B::REG_TEMP0);
        } else {
            Self::leave_exc_stack(e, false);
        }
        Self::jump(emit, label);
    }

    pub fn unary_op(emit: *mut crate::emit::Emit, op: UnaryOp) {
        let emit = emit_mut::<B>(emit);
        let vtype = Self::peek_vtype(emit, 0);
        if matches!(vtype, VType::Int | VType::Uint) {
            let mut vt = vtype;
            Self::emit_pre_pop_reg(emit, &mut vt, B::REG_RET);
            match op {
                UnaryOp::Positive => Self::emit_post_push_reg(emit, vtype, B::REG_RET),
                UnaryOp::Negative => {
                    B::neg_reg(&mut emit.as_, B::REG_RET);
                    Self::emit_post_push_reg(emit, vtype, B::REG_RET);
                }
                UnaryOp::Invert => {
                    if B::HAS_ASM_NOT_REG {
                        B::not_reg(&mut emit.as_, B::REG_RET);
                    } else {
                        B::mov_reg_imm(&mut emit.as_, B::REG_ARG_1, usize::MAX);
                        B::xor_reg_reg(&mut emit.as_, B::REG_RET, B::REG_ARG_1);
                    }
                    Self::emit_post_push_reg(emit, vtype, B::REG_RET);
                }
                _ => viper_type_error_msg(emit, b"'not' not implemented"),
            }
        } else if vtype == VType::PyObj {
            let mut vt = VType::PyObj;
            Self::emit_pre_pop_reg(emit, &mut vt, B::REG_ARG_2);
            Self::emit_call_with_imm_arg(emit, mp_f::UNARY_OP, op as i64, B::REG_ARG_1);
            Self::emit_post_push_reg(emit, VType::PyObj, B::REG_RET);
        } else {
            viper_type_error_vtype(emit, b"can't do unary op of '", vtype);
        }
    }

    pub fn binary_op(emit: *mut crate::emit::Emit, op: BinaryOp) {
        let emit = emit_mut::<B>(emit);
        let vtype_lhs = Self::peek_vtype(emit, 1);
        let vtype_rhs = Self::peek_vtype(emit, 0);
        if (matches!(vtype_lhs, VType::Int | VType::Uint))
            && (matches!(vtype_rhs, VType::Int | VType::Uint))
        {
            let op = Self::normalize_inplace_binary_op(op);

            if (B::N_X64 || B::N_X86)
                && matches!(op, BinaryOp::Lshift | BinaryOp::Rshift)
            {
                let mut vt_r = vtype_rhs;
                let mut vt_l = vtype_lhs;
                Self::emit_pre_pop_reg(emit, &mut vt_r, B::REG_ARG_4);
                Self::emit_pre_pop_reg(emit, &mut vt_l, B::REG_RET);
                if op == BinaryOp::Lshift {
                    B::lsl_reg(&mut emit.as_, B::REG_RET);
                } else if vtype_lhs == VType::Uint {
                    B::lsr_reg(&mut emit.as_, B::REG_RET);
                } else {
                    B::asr_reg(&mut emit.as_, B::REG_RET);
                }
                Self::emit_post_push_reg(emit, vtype_lhs, B::REG_RET);
                return;
            }

            if matches!(op, BinaryOp::FloorDivide | BinaryOp::Modulo) {
                if vtype_lhs != VType::Int {
                    Self::adjust_stack(emit, -2);
                    viper_type_error_msg(emit, b"div/mod not implemented for uint");
                    return;
                }
                let mut vt_r = vtype_rhs;
                let mut vt_l = vtype_lhs;
                Self::emit_pre_pop_reg_reg(emit, &mut vt_r, B::REG_ARG_2, &mut vt_l, B::REG_ARG_1);
                let fun = if op == BinaryOp::FloorDivide {
                    mp_f::SMALL_INT_FLOOR_DIVIDE
                } else {
                    mp_f::SMALL_INT_MODULO
                };
                Self::emit_call(emit, fun);
                Self::emit_post_push_reg(emit, VType::Int, B::REG_RET);
                return;
            }

            if matches!(
                op,
                BinaryOp::Less
                    | BinaryOp::More
                    | BinaryOp::Equal
                    | BinaryOp::LessEqual
                    | BinaryOp::MoreEqual
                    | BinaryOp::NotEqual
            ) && vtype_lhs != vtype_rhs
            {
                Self::adjust_stack(emit, -2);
                viper_type_error_msg(emit, b"comparison of int and uint");
                return;
            }

            let mut reg_rhs = B::REG_ARG_3;
            let mut vt_r = vtype_rhs;
            Self::emit_pre_pop_reg_flexible(emit, &mut vt_r, &mut reg_rhs, B::REG_RET, B::REG_ARG_2);
            let mut vt_l = vtype_lhs;
            Self::emit_pre_pop_reg(emit, &mut vt_l, B::REG_ARG_2);

            if !(B::N_X64 || B::N_X86) && matches!(op, BinaryOp::Lshift | BinaryOp::Rshift) {
                if op == BinaryOp::Lshift {
                    B::lsl_reg_reg(&mut emit.as_, B::REG_ARG_2, reg_rhs);
                } else if vtype_lhs == VType::Uint {
                    B::lsr_reg_reg(&mut emit.as_, B::REG_ARG_2, reg_rhs);
                } else {
                    B::asr_reg_reg(&mut emit.as_, B::REG_ARG_2, reg_rhs);
                }
                Self::emit_post_push_reg(emit, vtype_lhs, B::REG_ARG_2);
                return;
            }

            match op {
                BinaryOp::Or => B::or_reg_reg(&mut emit.as_, B::REG_ARG_2, reg_rhs),
                BinaryOp::Xor => B::xor_reg_reg(&mut emit.as_, B::REG_ARG_2, reg_rhs),
                BinaryOp::And => B::and_reg_reg(&mut emit.as_, B::REG_ARG_2, reg_rhs),
                BinaryOp::Add => B::add_reg_reg(&mut emit.as_, B::REG_ARG_2, reg_rhs),
                BinaryOp::Subtract => B::sub_reg_reg(&mut emit.as_, B::REG_ARG_2, reg_rhs),
                BinaryOp::Multiply => B::mul_reg_reg(&mut emit.as_, B::REG_ARG_2, reg_rhs),
                BinaryOp::Less
                | BinaryOp::More
                | BinaryOp::Equal
                | BinaryOp::LessEqual
                | BinaryOp::MoreEqual
                | BinaryOp::NotEqual => {
                    let op_idx = viper_compare_op_idx(op, vtype_lhs == VType::Uint).unwrap();
                    Self::need_reg_single(emit, B::REG_RET, 0);
                    B::binary_op_setcc(&mut emit.as_, op_idx, B::REG_RET, B::REG_ARG_2, reg_rhs);
                    Self::emit_post_push_reg(emit, VType::Bool, B::REG_RET);
                    return;
                }
                _ => {
                    Self::adjust_stack(emit, 1);
                    viper_type_error_msg(emit, b"binary op not implemented");
                    return;
                }
            }
            Self::emit_post_push_reg(emit, vtype_lhs, B::REG_ARG_2);
        } else if vtype_lhs == VType::PyObj && vtype_rhs == VType::PyObj {
            let mut vt_r = VType::PyObj;
            let mut vt_l = VType::PyObj;
            Self::emit_pre_pop_reg(emit, &mut vt_r, B::REG_ARG_3);
            Self::emit_pre_pop_reg(emit, &mut vt_l, B::REG_ARG_2);
            Self::emit_call_with_imm_arg(emit, mp_f::BINARY_OP, op as i64, B::REG_ARG_1);
            Self::emit_post_push_reg(emit, VType::PyObj, B::REG_RET);
        } else {
            Self::adjust_stack(emit, -1);
            viper_type_error_vtypes(
                emit,
                b"can't do binary op between '",
                b"' and '",
                b"'",
                vtype_lhs,
                vtype_rhs,
            );
        }
    }

    pub fn build(emit: *mut crate::emit::Emit, n_args: usize, kind: i32) {
        let emit = emit_mut::<B>(emit);
        use crate::emit::{EMIT_BUILD_LIST, EMIT_BUILD_SET, EMIT_BUILD_TUPLE};
        if kind == EMIT_BUILD_TUPLE || kind == EMIT_BUILD_LIST || kind == EMIT_BUILD_SET {
            Self::emit_get_stack_pointer_to_reg_for_pop(emit, B::REG_ARG_2, n_args);
        }
        Self::emit_call_with_imm_arg(emit, mp_f::BUILD_TUPLE + kind as u32, n_args as i64, B::REG_ARG_1);
        Self::emit_post_push_reg(emit, VType::PyObj, B::REG_RET);
    }

    pub fn store_map(emit: *mut crate::emit::Emit) {
        let emit = emit_mut::<B>(emit);
        let mut vt_k = VType::PyObj;
        let mut vt_v = VType::PyObj;
        let mut vt_m = VType::PyObj;
        Self::emit_pre_pop_reg(emit, &mut vt_k, B::REG_ARG_2);
        Self::emit_pre_pop_reg(emit, &mut vt_v, B::REG_ARG_3);
        Self::emit_pre_pop_reg(emit, &mut vt_m, B::REG_ARG_1);
        Self::emit_call(emit, mp_f::STORE_MAP);
        Self::emit_post_push_reg(emit, VType::PyObj, B::REG_RET);
    }

    pub fn store_comp(emit: *mut crate::emit::Emit, kind: ScopeKind, _set_stack_index: usize) {
        let emit = emit_mut::<B>(emit);
        let fun = match kind {
            ScopeKind::ListComp => mp_f::LIST_APPEND,
            ScopeKind::SetComp => mp_f::STORE_SET,
            _ => mp_f::STORE_MAP,
        };
        Self::emit_call(emit, fun);
    }

    pub fn unpack_sequence(emit: *mut crate::emit::Emit, n_args: usize) {
        let emit = emit_mut::<B>(emit);
        let mut vtype = VType::PyObj;
        Self::emit_pre_pop_reg(emit, &mut vtype, B::REG_ARG_1);
        Self::emit_call_with_imm_arg(emit, mp_f::UNPACK_SEQUENCE, n_args as i64, B::REG_ARG_2);
    }

    pub fn unpack_ex(emit: *mut crate::emit::Emit, n_left: usize, n_right: usize) {
        let emit = emit_mut::<B>(emit);
        let mut vtype = VType::PyObj;
        Self::emit_pre_pop_reg(emit, &mut vtype, B::REG_ARG_1);
        Self::emit_call_with_imm_arg(
            emit,
            mp_f::UNPACK_EX,
            (n_left | (n_right << 8)) as i64,
            B::REG_ARG_2,
        );
    }

    pub fn make_function(
        emit: *mut crate::emit::Emit,
        scope: *mut Scope,
        n_pos_defaults: usize,
        n_kw_defaults: usize,
    ) {
        let emit = emit_mut::<B>(emit);
        Self::mov_reg_state(emit, B::REG_ARG_2, local_idx_fun_obj(emit));
        B::load_reg_reg_offset(&mut emit.as_, B::REG_ARG_2, B::REG_ARG_2, OFFSETOF_OBJ_FUN_BC_CONTEXT as i32);
        if n_pos_defaults == 0 && n_kw_defaults == 0 {
            Self::need_reg_all(emit);
            B::mov_reg_imm(&mut emit.as_, B::REG_ARG_3, 0);
        } else {
            Self::emit_get_stack_pointer_to_reg_for_pop(emit, B::REG_ARG_3, 2);
            Self::need_reg_all(emit);
        }
        unsafe {
            let table_off =
                emit::emit_common_alloc_const_child(unsafe { &mut *emit.emit_common }, (*scope).raw_code);
            Self::mov_reg_state(emit, B::REG_TEMP0, local_idx_fun_obj(emit));
            B::load_reg_reg_offset(
                &mut emit.as_,
                B::REG_TEMP0,
                B::REG_TEMP0,
                OFFSETOF_OBJ_FUN_BC_CHILD_TABLE as i32,
            );
            B::load_reg_reg_offset(&mut emit.as_, B::REG_ARG_1, B::REG_TEMP0, table_off as i32);
        }
        B::call_ind(&mut emit.as_, mp_f::MAKE_FUNCTION_FROM_PROTO_FUN);
        Self::emit_post_push_reg(emit, VType::PyObj, B::REG_RET);
    }

    pub fn make_closure(
        emit: *mut crate::emit::Emit,
        scope: *mut Scope,
        n_closed_over: usize,
        n_pos_defaults: usize,
        n_kw_defaults: usize,
    ) {
        let emit = emit_mut::<B>(emit);
        Self::mov_reg_state(emit, B::REG_ARG_2, local_idx_fun_obj(emit));
        B::load_reg_reg_offset(&mut emit.as_, B::REG_ARG_2, B::REG_ARG_2, OFFSETOF_OBJ_FUN_BC_CONTEXT as i32);
        if n_pos_defaults == 0 && n_kw_defaults == 0 {
            Self::need_reg_all(emit);
            B::mov_reg_imm(&mut emit.as_, B::REG_ARG_3, 0);
        } else {
            Self::emit_get_stack_pointer_to_reg_for_pop(emit, B::REG_ARG_3, 2 + n_closed_over);
            Self::adjust_stack(emit, (2 + n_closed_over) as i32);
            Self::need_reg_all(emit);
        }
        unsafe {
            let table_off =
                emit::emit_common_alloc_const_child(unsafe { &mut *emit.emit_common }, (*scope).raw_code);
            Self::mov_reg_state(emit, B::REG_TEMP0, local_idx_fun_obj(emit));
            B::load_reg_reg_offset(
                &mut emit.as_,
                B::REG_TEMP0,
                B::REG_TEMP0,
                OFFSETOF_OBJ_FUN_BC_CHILD_TABLE as i32,
            );
            B::load_reg_reg_offset(&mut emit.as_, B::REG_ARG_1, B::REG_TEMP0, table_off as i32);
        }
        B::call_ind(&mut emit.as_, mp_f::MAKE_FUNCTION_FROM_PROTO_FUN);
        if B::REG_ARG_1 != B::REG_RET {
            B::mov_reg_reg(&mut emit.as_, B::REG_ARG_1, B::REG_RET);
        }
        B::mov_reg_imm(&mut emit.as_, B::REG_ARG_2, n_closed_over);
        Self::emit_get_stack_pointer_to_reg_for_pop(emit, B::REG_ARG_3, n_closed_over);
        if n_pos_defaults != 0 || n_kw_defaults != 0 {
            Self::adjust_stack(emit, -2);
        }
        B::call_ind(&mut emit.as_, mp_f::NEW_CLOSURE);
        Self::emit_post_push_reg(emit, VType::PyObj, B::REG_RET);
    }

    pub fn call_function(
        emit: *mut crate::emit::Emit,
        n_positional: usize,
        n_keyword: usize,
        star_flags: u8,
    ) {
        let emit = emit_mut::<B>(emit);
        if Self::peek_vtype(emit, n_positional + 2 * n_keyword) == VType::BuiltinCast {
            let _ = star_flags;
            return;
        }
        if star_flags != 0 {
            Self::emit_get_stack_pointer_to_reg_for_pop(
                emit,
                B::REG_ARG_3,
                n_positional + 2 * n_keyword + 2,
            );
            Self::emit_call_with_2_imm_args(
                emit,
                mp_f::CALL_METHOD_N_KW_VAR,
                0,
                B::REG_ARG_1,
                ((n_keyword << 8) | n_positional) as i64,
                B::REG_ARG_2,
            );
            Self::emit_post_push_reg(emit, VType::PyObj, B::REG_RET);
            return;
        }
        if n_positional != 0 || n_keyword != 0 {
            Self::emit_get_stack_pointer_to_reg_for_pop(
                emit,
                B::REG_ARG_3,
                n_positional + 2 * n_keyword,
            );
        }
        let mut vtype = VType::PyObj;
        Self::emit_pre_pop_reg(emit, &mut vtype, B::REG_ARG_1);
        Self::emit_call_with_imm_arg(
            emit,
            mp_f::NATIVE_CALL_FUNCTION_N_KW,
            ((n_keyword << 8) | n_positional) as i64,
            B::REG_ARG_2,
        );
        Self::emit_post_push_reg(emit, VType::PyObj, B::REG_RET);
    }

    pub fn call_method(
        emit: *mut crate::emit::Emit,
        n_positional: usize,
        n_keyword: usize,
        star_flags: u8,
    ) {
        let emit = emit_mut::<B>(emit);
        let fun = if star_flags != 0 {
            mp_f::CALL_METHOD_N_KW_VAR
        } else {
            mp_f::CALL_METHOD_N_KW
        };
        if star_flags != 0 {
            Self::emit_get_stack_pointer_to_reg_for_pop(
                emit,
                B::REG_ARG_3,
                n_positional + 2 * n_keyword + 3,
            );
            Self::emit_call_with_2_imm_args(
                emit,
                fun,
                1,
                B::REG_ARG_1,
                ((n_keyword << 8) | n_positional) as i64,
                B::REG_ARG_2,
            );
        } else {
            Self::emit_get_stack_pointer_to_reg_for_pop(
                emit,
                B::REG_ARG_3,
                2 + n_positional + 2 * n_keyword,
            );
            Self::emit_call_with_2_imm_args(
                emit,
                fun,
                n_positional as i64,
                B::REG_ARG_1,
                n_keyword as i64,
                B::REG_ARG_2,
            );
        }
        Self::emit_post_push_reg(emit, VType::PyObj, B::REG_RET);
    }

    fn innermost_active_finally_sp_index(e: &EmitNative<B>) -> Option<i16> {
        e.exc_stack
            .iter()
            .rev()
            .find(|entry| entry.is_finally && entry.is_active)
            .map(|entry| entry.finally_sp_index)
    }

    /// Match py/vm.c `run_unwind_return`: park return in X (base+1) and point sp there.
    fn setup_generator_return_through_finally(e: &mut EmitNative<B>) {
        let x_slot = generator_return_x_slot_if_active(e).expect("return-through-finally");
        Self::need_stack_settled(e);
        let mut vtype = VType::PyObj;
        Self::emit_pre_pop_reg(e, &mut vtype, B::REG_TEMP0);
        Self::mov_state_reg(e, x_slot, B::REG_TEMP0);
        Self::mov_reg_state_addr(e, B::REG_TEMP1, x_slot);
        B::store_reg_reg_offset(
            &mut e.as_,
            B::REG_TEMP1,
            B::REG_GENERATOR_STATE,
            OFFSETOF_CODE_STATE_SP as i32,
        );
    }

    pub fn return_value(emit: *mut crate::emit::Emit) {
        let exit_label = unsafe { (*super::emit_ref::<B>(emit)).exit_label };
        let e = emit_mut::<B>(emit);
        if unsafe { *e.error_slot != obj::OBJ_NULL } {
            return;
        }
        if unsafe { (*e.scope).scope_flags & MP_SCOPE_FLAG_GENERATOR != 0 } {
            if Self::innermost_active_finally_sp_index(e).is_some() {
                Self::setup_generator_return_through_finally(e);
            } else {
                Self::emit_get_stack_pointer_to_reg_for_pop(e, B::REG_TEMP0, 1);
                B::store_reg_reg_offset(
                    &mut e.as_,
                    B::REG_TEMP0,
                    B::REG_GENERATOR_STATE,
                    OFFSETOF_CODE_STATE_SP as i32,
                );
            }
            let ret_local = local_idx_ret_val(e);
            B::mov_reg_imm(&mut e.as_, B::REG_TEMP0, MP_VM_RETURN_NORMAL);
            B::mov_local_reg(&mut e.as_, ret_local, B::REG_TEMP0);
            Self::unwind_jump(emit, exit_label, e.exc_stack.len());
            return;
        }
        if e.do_viper_types {
            let return_vtype =
                VType::from_u8((unsafe { (*e.scope).scope_flags >> MP_SCOPE_FLAG_VIPERRET_POS }) as u8);
            if Self::peek_vtype(e, 0) == VType::PtrNone {
                Self::emit_pre_pop_discard(e);
                if return_vtype == VType::PyObj {
                    Self::mov_reg_const(e, B::REG_PARENT_RET, mp_f::CONST_NONE_OBJ as i32);
                } else {
                    B::mov_reg_imm(&mut e.as_, B::REG_ARG_1, 0);
                }
            } else {
                let mut vtype = VType::PyObj;
                if return_vtype == VType::PyObj {
                    Self::emit_pre_pop_reg(e, &mut vtype, B::REG_PARENT_RET);
                } else {
                    Self::emit_pre_pop_reg(e, &mut vtype, B::REG_ARG_1);
                }
                if vtype != return_vtype {
                    viper_type_error_vtypes(
                        e,
                        b"return expected '",
                        b"' but got '",
                        b"'",
                        return_vtype,
                        vtype,
                    );
                    return;
                }
            }
            if return_vtype != VType::PyObj {
                Self::emit_call_with_imm_arg(
                    e,
                    mp_f::CONVERT_NATIVE_TO_OBJ,
                    return_vtype as i64,
                    B::REG_ARG_2,
                );
                if B::REG_RET != B::REG_PARENT_RET {
                    B::mov_reg_reg(&mut e.as_, B::REG_PARENT_RET, B::REG_RET);
                }
            }
        } else {
            let mut vtype = VType::PyObj;
            Self::emit_pre_pop_reg(e, &mut vtype, B::REG_PARENT_RET);
            debug_assert_eq!(vtype, VType::PyObj);
        }
        if need_global_exc_handler(e) {
            let ret_local = local_idx_ret_val(e);
            B::mov_local_reg(&mut e.as_, ret_local, B::REG_PARENT_RET);
        }
        Self::unwind_jump(emit, exit_label, e.exc_stack.len());
    }

    pub fn raise_varargs(emit: *mut crate::emit::Emit, n_args: usize) {
        if n_args != 1 {
            emit_not_implemented_error(emit_mut::<B>(emit), b"native raise");
            return;
        }
        let emit = emit_mut::<B>(emit);
        let mut vtype = VType::PyObj;
        Self::emit_pre_pop_reg(emit, &mut vtype, B::REG_ARG_1);
        if vtype != VType::PyObj {
            viper_type_error_msg(emit, b"must raise an object");
            return;
        }
        Self::emit_call(emit, mp_f::NATIVE_RAISE);
        B::asm_base(&mut emit.as_).suppress_code();
    }

    pub fn yield_(emit: *mut crate::emit::Emit, kind: i32) {
        use crate::emit::{EMIT_YIELD_FROM, EMIT_YIELD_VALUE};
        let e = emit_mut::<B>(emit);
        if e.do_viper_types {
            emit_not_implemented_error(e, b"native yield");
            return;
        }
        unsafe {
            (*e.scope).scope_flags |= MP_SCOPE_FLAG_GENERATOR;
        }
        Self::need_stack_settled(e);
        let ret_local = local_idx_ret_val(e);
        let exit_label = e.exit_label;
        let label_slot = unsafe { *e.label_slot };
        if kind == EMIT_YIELD_FROM {
            Self::jump(emit, label_slot + 2);
            asmbase::label_assign(B::asm_base(&mut e.as_), label_slot + 1);
        }
        // Return-through-finally parks the value and points sp at the stash; do not
        // overwrite that pointer when a subsequent yield_from runs (__aexit__).
        let sp_save_done = label_slot + 5;
        if kind == EMIT_YIELD_FROM {
            if need_gen_return_obj(e) {
                if let Some(x_slot) = generator_return_x_slot_if_active(e) {
                    B::load_reg_reg_offset(
                        &mut e.as_,
                        B::REG_TEMP2,
                        B::REG_GENERATOR_STATE,
                        OFFSETOF_CODE_STATE_SP as i32,
                    );
                    Self::mov_reg_state_addr(e, B::REG_ARG_4, x_slot);
                    B::jump_if_reg_eq(&mut e.as_, B::REG_TEMP2, B::REG_ARG_4, sp_save_done);
                }
            }
            Self::emit_get_stack_pointer_to_reg_for_pop(e, B::REG_TEMP0, 1);
            B::store_reg_reg_offset(
                &mut e.as_,
                B::REG_TEMP0,
                B::REG_GENERATOR_STATE,
                OFFSETOF_CODE_STATE_SP as i32,
            );
            asmbase::label_assign(B::asm_base(&mut e.as_), sp_save_done);
        } else {
            Self::emit_get_stack_pointer_to_reg_for_pop(e, B::REG_TEMP0, 1);
            B::store_reg_reg_offset(
                &mut e.as_,
                B::REG_TEMP0,
                B::REG_GENERATOR_STATE,
                OFFSETOF_CODE_STATE_SP as i32,
            );
        }
        B::mov_reg_imm(&mut e.as_, B::REG_TEMP0, MP_VM_RETURN_YIELD);
        B::mov_local_reg(&mut e.as_, ret_local, B::REG_TEMP0);
        B::mov_reg_pcrel(&mut e.as_, B::REG_TEMP0, label_slot);
        Self::mov_state_reg(e, local_idx_gen_pc(e), B::REG_TEMP0);
        Self::jump(emit, exit_label);
        asmbase::label_assign(B::asm_base(&mut e.as_), label_slot);
        if !e.exc_stack.is_empty() {
            let mut e_idx = e.exc_stack.len();
            loop {
                if e_idx == 0 {
                    break;
                }
                e_idx -= 1;
                if e.exc_stack[e_idx].is_active {
                    B::mov_reg_pcrel(&mut e.as_, B::REG_RET, e.exc_stack[e_idx].label as usize);
                    let exc_pc = local_idx_exc_handler_pc(e);
                    B::mov_local_reg(&mut e.as_, exc_pc, B::REG_RET);
                    break;
                }
            }
        }
        Self::adjust_stack_size(emit, 1);
        if kind == EMIT_YIELD_VALUE {
            Self::emit_gen_throw_if_pending(e, label_slot + 1);
        } else {
            asmbase::label_assign(B::asm_base(&mut e.as_), label_slot + 2);
            let throw_val = local_idx_throw_val(e);
            B::mov_reg_local(&mut e.as_, B::REG_ARG_3, throw_val);
            B::mov_reg_imm(&mut e.as_, B::REG_ARG_2, obj::OBJ_NULL.0);
            B::mov_local_reg(&mut e.as_, throw_val, B::REG_ARG_2);
            let mut vtype = VType::PyObj;
            Self::emit_pre_pop_reg(e, &mut vtype, B::REG_ARG_2);
            let mut vtype_gen = VType::PyObj;
            Self::emit_access_stack(e, 1, &mut vtype_gen, B::REG_ARG_1);
            Self::emit_post_push_reg(e, VType::PyObj, B::REG_ARG_3);
            Self::emit_get_stack_pointer_to_reg_for_pop(e, B::REG_ARG_3, 1);
            if need_gen_return_obj(e) {
                if let Some(x_slot) = generator_return_x_slot_if_active(e) {
                    Self::mov_reg_state_addr(e, B::REG_ARG_4, x_slot);
                } else {
                    B::mov_reg_imm(&mut e.as_, B::REG_ARG_4, 0);
                }
            } else {
                B::mov_reg_imm(&mut e.as_, B::REG_ARG_4, 0);
            }
            Self::emit_call(e, mp_f::NATIVE_YIELD_FROM);
            Self::emit_yield_from_handle_delegate_result(e, label_slot);
        }
    }

    pub fn start_except_handler(emit: *mut crate::emit::Emit) {
        let emit = emit_mut::<B>(emit);
        Self::leave_exc_stack(emit, true);
        let exc_val = local_idx_exc_val(emit);
        B::mov_reg_local(&mut emit.as_, B::REG_TEMP0, exc_val);
        Self::emit_post_push_reg(emit, VType::PyObj, B::REG_TEMP0);
    }

    pub fn end_except_handler(_emit: *mut crate::emit::Emit) {}
}

fn viper_compare_op_idx(op: BinaryOp, unsigned_lhs: bool) -> Option<usize> {
    let base = op as u8;
    if base < BinaryOp::Less as u8 || base > BinaryOp::MoreEqual as u8 {
        return None;
    }
    Some((base - BinaryOp::Less as u8) as usize + if unsigned_lhs { 0 } else { 6 })
}

#[cfg(test)]
mod emitnative_impl_tests {
    use super::*;
    use crate::runtime0::BinaryOp;

    #[test]
    fn viper_compare_op_idx_maps_relational_ops() {
        assert_eq!(viper_compare_op_idx(BinaryOp::Less, true), Some(0));
        assert_eq!(viper_compare_op_idx(BinaryOp::MoreEqual, true), Some(5));
        assert_eq!(viper_compare_op_idx(BinaryOp::Less, false), Some(6));
        assert_eq!(viper_compare_op_idx(BinaryOp::NotEqual, false), Some(9));
        assert_eq!(viper_compare_op_idx(BinaryOp::MoreEqual, false), Some(11));
        assert_eq!(viper_compare_op_idx(BinaryOp::Add, true), None);
    }

    #[test]
    fn viper_binary_power_not_implemented_for_int() {
        // py/emitnative.c rejects int**int via EMIT_NATIVE_VIPER_TYPE_ERROR (no asm path).
        assert_eq!(viper_compare_op_idx(BinaryOp::Power, true), None);
        assert_eq!(viper_compare_op_idx(BinaryOp::Power, false), None);
    }

    #[test]
    fn vtype_name_matches_c_vtype_to_qstr() {
        assert_eq!(vtype_name(VType::Int), b"int");
        assert_eq!(vtype_name(VType::Uint), b"uint");
        assert_eq!(vtype_name(VType::Ptr8), b"ptr8");
        assert_eq!(vtype_name(VType::Unbound), b"None");
    }
}

impl VType {
    fn from_u8(v: u8) -> Self {
        match v & 0x7f {
            x if x == NATIVE_TYPE_OBJ as u8 => VType::PyObj,
            x if x == NATIVE_TYPE_BOOL as u8 => VType::Bool,
            x if x == NATIVE_TYPE_INT as u8 => VType::Int,
            x if x == NATIVE_TYPE_UINT as u8 => VType::Uint,
            x if x == NATIVE_TYPE_PTR as u8 => VType::Ptr,
            _ => VType::PyObj,
        }
    }
}

impl BinaryOp {
    fn from_u8(v: u8) -> Self {
        match v {
            0 => BinaryOp::Less,
            1 => BinaryOp::More,
            2 => BinaryOp::Equal,
            3 => BinaryOp::NotEqual,
            4 => BinaryOp::LessEqual,
            5 => BinaryOp::MoreEqual,
            6 => BinaryOp::In,
            7 => BinaryOp::Is,
            8 => BinaryOp::ExceptionMatch,
            9 => BinaryOp::InplaceOr,
            10 => BinaryOp::InplaceXor,
            11 => BinaryOp::InplaceAnd,
            12 => BinaryOp::InplaceLshift,
            13 => BinaryOp::InplaceRshift,
            14 => BinaryOp::InplaceAdd,
            15 => BinaryOp::InplaceSubtract,
            16 => BinaryOp::InplaceMultiply,
            17 => BinaryOp::InplaceMatMult,
            18 => BinaryOp::InplaceFloorDivide,
            19 => BinaryOp::InplaceTrueDivide,
            20 => BinaryOp::InplaceModulo,
            21 => BinaryOp::InplacePower,
            22 => BinaryOp::Or,
            23 => BinaryOp::Xor,
            24 => BinaryOp::And,
            25 => BinaryOp::Lshift,
            26 => BinaryOp::Rshift,
            27 => BinaryOp::Add,
            28 => BinaryOp::Subtract,
            29 => BinaryOp::Multiply,
            30 => BinaryOp::MatMult,
            31 => BinaryOp::FloorDivide,
            32 => BinaryOp::TrueDivide,
            33 => BinaryOp::Modulo,
            34 => BinaryOp::Power,
            _ => BinaryOp::Add,
        }
    }
}
