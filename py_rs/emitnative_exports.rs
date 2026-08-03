//! C-compatible `emit_native_<arch>_*` entry points for native emitters.
// symmetry: done

/// Export `emit_native_*` wrappers for a backend type (unprefixed helper names).
#[macro_export]
macro_rules! export_emit_native {
    ($Backend:ty) => {
        pub type EmitNativeBackend = $crate::emitnative::EmitNative<$Backend>;

        pub fn paste_emit_native_new(
            emit_common: *mut $crate::emit::EmitCommon,
            error_slot: *mut $crate::obj::Obj,
            label_slot: *mut usize,
            max_num_labels: usize,
        ) -> *mut $crate::emit::Emit {
            EmitNativeBackend::new(emit_common, error_slot, label_slot, max_num_labels)
                as *mut $crate::emit::Emit
        }

        pub fn paste_emit_native_free(emit: *mut $crate::emit::Emit) {
            EmitNativeBackend::free(emit as *mut EmitNativeBackend);
        }

        pub fn paste_emit_native_end_pass(emit: *mut $crate::emit::Emit) -> bool {
            EmitNativeBackend::end_pass(emit)
        }

        pub fn paste_emit_native_start_pass(
            emit: *mut $crate::emit::Emit,
            pass: $crate::emit::PassKind,
            scope: *mut $crate::scope::Scope,
        ) {
            EmitNativeBackend::start_pass(emit, pass, scope);
        }

        pub fn paste_emit_native_set_source_line(
            emit: *mut $crate::emit::Emit,
            source_line: usize,
        ) {
            EmitNativeBackend::set_source_line(emit, source_line);
        }

        pub fn paste_emit_native_adjust_stack_size(emit: *mut $crate::emit::Emit, delta: i64) {
            EmitNativeBackend::adjust_stack_size(emit, delta);
        }

        pub fn paste_emit_native_load_local(
            emit: *mut $crate::emit::Emit,
            qst: $crate::qstr::Qstr,
            local_num: usize,
            kind: i32,
        ) {
            EmitNativeBackend::load_local(emit, qst, local_num, kind);
        }

        pub fn paste_emit_native_load_global(
            emit: *mut $crate::emit::Emit,
            qst: $crate::qstr::Qstr,
            kind: i32,
        ) {
            EmitNativeBackend::load_global(emit, qst, kind);
        }

        pub fn paste_emit_native_store_local(
            emit: *mut $crate::emit::Emit,
            qst: $crate::qstr::Qstr,
            local_num: usize,
            kind: i32,
        ) {
            EmitNativeBackend::store_local(emit, qst, local_num, kind);
        }

        pub fn paste_emit_native_store_global(
            emit: *mut $crate::emit::Emit,
            qst: $crate::qstr::Qstr,
            kind: i32,
        ) {
            EmitNativeBackend::store_global(emit, qst, kind);
        }

        pub fn paste_emit_native_delete_local(
            emit: *mut $crate::emit::Emit,
            qst: $crate::qstr::Qstr,
            local_num: usize,
            kind: i32,
        ) {
            EmitNativeBackend::delete_local(emit, qst, local_num, kind);
        }

        pub fn paste_emit_native_delete_global(
            emit: *mut $crate::emit::Emit,
            qst: $crate::qstr::Qstr,
            kind: i32,
        ) {
            EmitNativeBackend::delete_global(emit, qst, kind);
        }

        pub fn paste_emit_native_label_assign(emit: *mut $crate::emit::Emit, l: usize) {
            EmitNativeBackend::label_assign(emit, l);
        }

        pub fn paste_emit_native_import(
            emit: *mut $crate::emit::Emit,
            qst: $crate::qstr::Qstr,
            kind: i32,
        ) {
            EmitNativeBackend::import(emit, qst, kind);
        }

        pub fn paste_emit_native_load_const_tok(
            emit: *mut $crate::emit::Emit,
            tok: $crate::lexer::TokenKind,
        ) {
            EmitNativeBackend::load_const_tok(emit, tok);
        }

        pub fn paste_emit_native_load_const_small_int(emit: *mut $crate::emit::Emit, arg: i64) {
            EmitNativeBackend::load_const_small_int(emit, arg);
        }

        pub fn paste_emit_native_load_const_str(
            emit: *mut $crate::emit::Emit,
            qst: $crate::qstr::Qstr,
        ) {
            EmitNativeBackend::load_const_str(emit, qst);
        }

        pub fn paste_emit_native_load_const_obj(
            emit: *mut $crate::emit::Emit,
            obj_in: $crate::obj::Obj,
        ) {
            EmitNativeBackend::load_const_obj(emit, obj_in);
        }

        pub fn paste_emit_native_load_null(emit: *mut $crate::emit::Emit) {
            EmitNativeBackend::load_null(emit);
        }

        pub fn paste_emit_native_load_method(
            emit: *mut $crate::emit::Emit,
            qst: $crate::qstr::Qstr,
            is_super: bool,
        ) {
            EmitNativeBackend::load_method(emit, qst, is_super);
        }

        pub fn paste_emit_native_load_build_class(emit: *mut $crate::emit::Emit) {
            EmitNativeBackend::load_build_class(emit);
        }

        pub fn paste_emit_native_subscr(emit: *mut $crate::emit::Emit, kind: i32) {
            EmitNativeBackend::subscr(emit, kind);
        }

        pub fn paste_emit_native_attr(
            emit: *mut $crate::emit::Emit,
            qst: $crate::qstr::Qstr,
            kind: i32,
        ) {
            EmitNativeBackend::attr(emit, qst, kind);
        }

        pub fn paste_emit_native_dup_top(emit: *mut $crate::emit::Emit) {
            EmitNativeBackend::dup_top(emit);
        }

        pub fn paste_emit_native_dup_top_two(emit: *mut $crate::emit::Emit) {
            EmitNativeBackend::dup_top_two(emit);
        }

        pub fn paste_emit_native_pop_top(emit: *mut $crate::emit::Emit) {
            EmitNativeBackend::pop_top(emit);
        }

        pub fn paste_emit_native_rot_two(emit: *mut $crate::emit::Emit) {
            EmitNativeBackend::rot_two(emit);
        }

        pub fn paste_emit_native_rot_three(emit: *mut $crate::emit::Emit) {
            EmitNativeBackend::rot_three(emit);
        }

        pub fn paste_emit_native_jump(emit: *mut $crate::emit::Emit, label: usize) {
            EmitNativeBackend::jump(emit, label);
        }

        pub fn paste_emit_native_pop_jump_if(
            emit: *mut $crate::emit::Emit,
            cond: bool,
            label: usize,
        ) {
            EmitNativeBackend::pop_jump_if(emit, cond, label);
        }

        pub fn paste_emit_native_jump_if_or_pop(
            emit: *mut $crate::emit::Emit,
            cond: bool,
            label: usize,
        ) {
            EmitNativeBackend::jump_if_or_pop(emit, cond, label);
        }

        pub fn paste_emit_native_unwind_jump(
            emit: *mut $crate::emit::Emit,
            label: usize,
            except_depth: usize,
        ) {
            EmitNativeBackend::unwind_jump(emit, label, except_depth);
        }

        pub fn paste_emit_native_setup_block(
            emit: *mut $crate::emit::Emit,
            label: usize,
            kind: i32,
        ) {
            EmitNativeBackend::setup_block(emit, label, kind);
        }

        pub fn paste_emit_native_with_cleanup(emit: *mut $crate::emit::Emit, label: usize) {
            EmitNativeBackend::with_cleanup(emit, label);
        }

        pub fn paste_emit_native_async_with_setup_finally(
            emit: *mut $crate::emit::Emit,
            label_aexit_no_exc: usize,
            label_finally_block: usize,
            label_ret_unwind_jump: usize,
        ) {
            EmitNativeBackend::async_with_setup_finally(
                emit,
                label_aexit_no_exc,
                label_finally_block,
                label_ret_unwind_jump,
            );
        }

        pub fn paste_emit_native_async_with_ret_unwind_enter(emit: *mut $crate::emit::Emit) {
            EmitNativeBackend::async_with_ret_unwind_enter(emit);
        }

        pub fn paste_emit_native_get_iter(emit: *mut $crate::emit::Emit, use_stack: bool) {
            EmitNativeBackend::get_iter(emit, use_stack);
        }

        pub fn paste_emit_native_for_iter(emit: *mut $crate::emit::Emit, label: usize) {
            EmitNativeBackend::for_iter(emit, label);
        }

        pub fn paste_emit_native_for_iter_end(emit: *mut $crate::emit::Emit) {
            EmitNativeBackend::for_iter_end(emit);
        }

        pub fn paste_emit_native_pop_except_jump(
            emit: *mut $crate::emit::Emit,
            label: usize,
            within_exc_handler: bool,
        ) {
            EmitNativeBackend::pop_except_jump(emit, label, within_exc_handler);
        }

        pub fn paste_emit_native_end_finally(emit: *mut $crate::emit::Emit) {
            EmitNativeBackend::end_finally(emit);
        }

        pub fn paste_emit_native_unary_op(
            emit: *mut $crate::emit::Emit,
            op: $crate::runtime0::UnaryOp,
        ) {
            EmitNativeBackend::unary_op(emit, op);
        }

        pub fn paste_emit_native_binary_op(
            emit: *mut $crate::emit::Emit,
            op: $crate::runtime0::BinaryOp,
        ) {
            EmitNativeBackend::binary_op(emit, op);
        }

        pub fn paste_emit_native_build(emit: *mut $crate::emit::Emit, n_args: usize, kind: i32) {
            EmitNativeBackend::build(emit, n_args, kind);
        }

        pub fn paste_emit_native_store_map(emit: *mut $crate::emit::Emit) {
            EmitNativeBackend::store_map(emit);
        }

        pub fn paste_emit_native_store_comp(
            emit: *mut $crate::emit::Emit,
            kind: $crate::scope::ScopeKind,
            set_stack_index: usize,
        ) {
            EmitNativeBackend::store_comp(emit, kind, set_stack_index);
        }

        pub fn paste_emit_native_unpack_sequence(emit: *mut $crate::emit::Emit, n_args: usize) {
            EmitNativeBackend::unpack_sequence(emit, n_args);
        }

        pub fn paste_emit_native_unpack_ex(
            emit: *mut $crate::emit::Emit,
            n_left: usize,
            n_right: usize,
        ) {
            EmitNativeBackend::unpack_ex(emit, n_left, n_right);
        }

        pub fn paste_emit_native_make_function(
            emit: *mut $crate::emit::Emit,
            scope: *mut $crate::scope::Scope,
            n_pos_defaults: usize,
            n_kw_defaults: usize,
        ) {
            EmitNativeBackend::make_function(emit, scope, n_pos_defaults, n_kw_defaults);
        }

        pub fn paste_emit_native_make_closure(
            emit: *mut $crate::emit::Emit,
            scope: *mut $crate::scope::Scope,
            n_closed_over: usize,
            n_pos_defaults: usize,
            n_kw_defaults: usize,
        ) {
            EmitNativeBackend::make_closure(
                emit,
                scope,
                n_closed_over,
                n_pos_defaults,
                n_kw_defaults,
            );
        }

        pub fn paste_emit_native_call_function(
            emit: *mut $crate::emit::Emit,
            n_positional: usize,
            n_keyword: usize,
            star_flags: u8,
        ) {
            EmitNativeBackend::call_function(emit, n_positional, n_keyword, star_flags);
        }

        pub fn paste_emit_native_call_method(
            emit: *mut $crate::emit::Emit,
            n_positional: usize,
            n_keyword: usize,
            star_flags: u8,
        ) {
            EmitNativeBackend::call_method(emit, n_positional, n_keyword, star_flags);
        }

        pub fn paste_emit_native_return_value(emit: *mut $crate::emit::Emit) {
            EmitNativeBackend::return_value(emit);
        }

        pub fn paste_emit_native_raise_varargs(emit: *mut $crate::emit::Emit, n_args: usize) {
            EmitNativeBackend::raise_varargs(emit, n_args);
        }

        pub fn paste_emit_native_yield(emit: *mut $crate::emit::Emit, kind: i32) {
            EmitNativeBackend::yield_(emit, kind);
        }

        pub fn paste_emit_native_start_except_handler(emit: *mut $crate::emit::Emit) {
            EmitNativeBackend::start_except_handler(emit);
        }

        pub fn paste_emit_native_end_except_handler(emit: *mut $crate::emit::Emit) {
            EmitNativeBackend::end_except_handler(emit);
        }
    };
}

/// Rename helpers to `emit_native_<prefix>_*` (explicit per-arch aliases, no paste crate).
#[macro_export]
macro_rules! alias_emit_native_exports {
    (arm) => {
        pub use paste_emit_native_new as emit_native_arm_new;
        pub use paste_emit_native_free as emit_native_arm_free;
        pub use paste_emit_native_end_pass as emit_native_arm_end_pass;
        pub use paste_emit_native_start_pass as emit_native_arm_start_pass;
        pub use paste_emit_native_set_source_line as emit_native_arm_set_source_line;
        pub use paste_emit_native_adjust_stack_size as emit_native_arm_adjust_stack_size;
        pub use paste_emit_native_load_local as emit_native_arm_load_local;
        pub use paste_emit_native_load_global as emit_native_arm_load_global;
        pub use paste_emit_native_store_local as emit_native_arm_store_local;
        pub use paste_emit_native_store_global as emit_native_arm_store_global;
        pub use paste_emit_native_delete_local as emit_native_arm_delete_local;
        pub use paste_emit_native_delete_global as emit_native_arm_delete_global;
        pub use paste_emit_native_label_assign as emit_native_arm_label_assign;
        pub use paste_emit_native_import as emit_native_arm_import;
        pub use paste_emit_native_load_const_tok as emit_native_arm_load_const_tok;
        pub use paste_emit_native_load_const_small_int as emit_native_arm_load_const_small_int;
        pub use paste_emit_native_load_const_str as emit_native_arm_load_const_str;
        pub use paste_emit_native_load_const_obj as emit_native_arm_load_const_obj;
        pub use paste_emit_native_load_null as emit_native_arm_load_null;
        pub use paste_emit_native_load_method as emit_native_arm_load_method;
        pub use paste_emit_native_load_build_class as emit_native_arm_load_build_class;
        pub use paste_emit_native_subscr as emit_native_arm_subscr;
        pub use paste_emit_native_attr as emit_native_arm_attr;
        pub use paste_emit_native_dup_top as emit_native_arm_dup_top;
        pub use paste_emit_native_dup_top_two as emit_native_arm_dup_top_two;
        pub use paste_emit_native_pop_top as emit_native_arm_pop_top;
        pub use paste_emit_native_rot_two as emit_native_arm_rot_two;
        pub use paste_emit_native_rot_three as emit_native_arm_rot_three;
        pub use paste_emit_native_jump as emit_native_arm_jump;
        pub use paste_emit_native_pop_jump_if as emit_native_arm_pop_jump_if;
        pub use paste_emit_native_jump_if_or_pop as emit_native_arm_jump_if_or_pop;
        pub use paste_emit_native_unwind_jump as emit_native_arm_unwind_jump;
        pub use paste_emit_native_setup_block as emit_native_arm_setup_block;
        pub use paste_emit_native_with_cleanup as emit_native_arm_with_cleanup;
        pub use paste_emit_native_async_with_setup_finally as emit_native_arm_async_with_setup_finally;
        pub use paste_emit_native_get_iter as emit_native_arm_get_iter;
        pub use paste_emit_native_for_iter as emit_native_arm_for_iter;
        pub use paste_emit_native_for_iter_end as emit_native_arm_for_iter_end;
        pub use paste_emit_native_pop_except_jump as emit_native_arm_pop_except_jump;
        pub use paste_emit_native_end_finally as emit_native_arm_end_finally;
        pub use paste_emit_native_unary_op as emit_native_arm_unary_op;
        pub use paste_emit_native_binary_op as emit_native_arm_binary_op;
        pub use paste_emit_native_build as emit_native_arm_build;
        pub use paste_emit_native_store_map as emit_native_arm_store_map;
        pub use paste_emit_native_store_comp as emit_native_arm_store_comp;
        pub use paste_emit_native_unpack_sequence as emit_native_arm_unpack_sequence;
        pub use paste_emit_native_unpack_ex as emit_native_arm_unpack_ex;
        pub use paste_emit_native_make_function as emit_native_arm_make_function;
        pub use paste_emit_native_make_closure as emit_native_arm_make_closure;
        pub use paste_emit_native_call_function as emit_native_arm_call_function;
        pub use paste_emit_native_call_method as emit_native_arm_call_method;
        pub use paste_emit_native_return_value as emit_native_arm_return_value;
        pub use paste_emit_native_raise_varargs as emit_native_arm_raise_varargs;
        pub use paste_emit_native_yield as emit_native_arm_yield;
        pub use paste_emit_native_start_except_handler as emit_native_arm_start_except_handler;
        pub use paste_emit_native_end_except_handler as emit_native_arm_end_except_handler;
    };
    (debug) => {
        pub use paste_emit_native_new as emit_native_debug_new;
        pub use paste_emit_native_free as emit_native_debug_free;
        pub use paste_emit_native_end_pass as emit_native_debug_end_pass;
        pub use paste_emit_native_start_pass as emit_native_debug_start_pass;
        pub use paste_emit_native_set_source_line as emit_native_debug_set_source_line;
        pub use paste_emit_native_adjust_stack_size as emit_native_debug_adjust_stack_size;
        pub use paste_emit_native_load_local as emit_native_debug_load_local;
        pub use paste_emit_native_load_global as emit_native_debug_load_global;
        pub use paste_emit_native_store_local as emit_native_debug_store_local;
        pub use paste_emit_native_store_global as emit_native_debug_store_global;
        pub use paste_emit_native_delete_local as emit_native_debug_delete_local;
        pub use paste_emit_native_delete_global as emit_native_debug_delete_global;
        pub use paste_emit_native_label_assign as emit_native_debug_label_assign;
        pub use paste_emit_native_import as emit_native_debug_import;
        pub use paste_emit_native_load_const_tok as emit_native_debug_load_const_tok;
        pub use paste_emit_native_load_const_small_int as emit_native_debug_load_const_small_int;
        pub use paste_emit_native_load_const_str as emit_native_debug_load_const_str;
        pub use paste_emit_native_load_const_obj as emit_native_debug_load_const_obj;
        pub use paste_emit_native_load_null as emit_native_debug_load_null;
        pub use paste_emit_native_load_method as emit_native_debug_load_method;
        pub use paste_emit_native_load_build_class as emit_native_debug_load_build_class;
        pub use paste_emit_native_subscr as emit_native_debug_subscr;
        pub use paste_emit_native_attr as emit_native_debug_attr;
        pub use paste_emit_native_dup_top as emit_native_debug_dup_top;
        pub use paste_emit_native_dup_top_two as emit_native_debug_dup_top_two;
        pub use paste_emit_native_pop_top as emit_native_debug_pop_top;
        pub use paste_emit_native_rot_two as emit_native_debug_rot_two;
        pub use paste_emit_native_rot_three as emit_native_debug_rot_three;
        pub use paste_emit_native_jump as emit_native_debug_jump;
        pub use paste_emit_native_pop_jump_if as emit_native_debug_pop_jump_if;
        pub use paste_emit_native_jump_if_or_pop as emit_native_debug_jump_if_or_pop;
        pub use paste_emit_native_unwind_jump as emit_native_debug_unwind_jump;
        pub use paste_emit_native_setup_block as emit_native_debug_setup_block;
        pub use paste_emit_native_with_cleanup as emit_native_debug_with_cleanup;
        pub use paste_emit_native_async_with_setup_finally as emit_native_debug_async_with_setup_finally;
        pub use paste_emit_native_get_iter as emit_native_debug_get_iter;
        pub use paste_emit_native_for_iter as emit_native_debug_for_iter;
        pub use paste_emit_native_for_iter_end as emit_native_debug_for_iter_end;
        pub use paste_emit_native_pop_except_jump as emit_native_debug_pop_except_jump;
        pub use paste_emit_native_end_finally as emit_native_debug_end_finally;
        pub use paste_emit_native_unary_op as emit_native_debug_unary_op;
        pub use paste_emit_native_binary_op as emit_native_debug_binary_op;
        pub use paste_emit_native_build as emit_native_debug_build;
        pub use paste_emit_native_store_map as emit_native_debug_store_map;
        pub use paste_emit_native_store_comp as emit_native_debug_store_comp;
        pub use paste_emit_native_unpack_sequence as emit_native_debug_unpack_sequence;
        pub use paste_emit_native_unpack_ex as emit_native_debug_unpack_ex;
        pub use paste_emit_native_make_function as emit_native_debug_make_function;
        pub use paste_emit_native_make_closure as emit_native_debug_make_closure;
        pub use paste_emit_native_call_function as emit_native_debug_call_function;
        pub use paste_emit_native_call_method as emit_native_debug_call_method;
        pub use paste_emit_native_return_value as emit_native_debug_return_value;
        pub use paste_emit_native_raise_varargs as emit_native_debug_raise_varargs;
        pub use paste_emit_native_yield as emit_native_debug_yield;
        pub use paste_emit_native_start_except_handler as emit_native_debug_start_except_handler;
        pub use paste_emit_native_end_except_handler as emit_native_debug_end_except_handler;
    };
    (rv32) => {
        pub use paste_emit_native_new as emit_native_rv32_new;
        pub use paste_emit_native_free as emit_native_rv32_free;
        pub use paste_emit_native_end_pass as emit_native_rv32_end_pass;
        pub use paste_emit_native_start_pass as emit_native_rv32_start_pass;
        pub use paste_emit_native_set_source_line as emit_native_rv32_set_source_line;
        pub use paste_emit_native_adjust_stack_size as emit_native_rv32_adjust_stack_size;
        pub use paste_emit_native_load_local as emit_native_rv32_load_local;
        pub use paste_emit_native_load_global as emit_native_rv32_load_global;
        pub use paste_emit_native_store_local as emit_native_rv32_store_local;
        pub use paste_emit_native_store_global as emit_native_rv32_store_global;
        pub use paste_emit_native_delete_local as emit_native_rv32_delete_local;
        pub use paste_emit_native_delete_global as emit_native_rv32_delete_global;
        pub use paste_emit_native_label_assign as emit_native_rv32_label_assign;
        pub use paste_emit_native_import as emit_native_rv32_import;
        pub use paste_emit_native_load_const_tok as emit_native_rv32_load_const_tok;
        pub use paste_emit_native_load_const_small_int as emit_native_rv32_load_const_small_int;
        pub use paste_emit_native_load_const_str as emit_native_rv32_load_const_str;
        pub use paste_emit_native_load_const_obj as emit_native_rv32_load_const_obj;
        pub use paste_emit_native_load_null as emit_native_rv32_load_null;
        pub use paste_emit_native_load_method as emit_native_rv32_load_method;
        pub use paste_emit_native_load_build_class as emit_native_rv32_load_build_class;
        pub use paste_emit_native_subscr as emit_native_rv32_subscr;
        pub use paste_emit_native_attr as emit_native_rv32_attr;
        pub use paste_emit_native_dup_top as emit_native_rv32_dup_top;
        pub use paste_emit_native_dup_top_two as emit_native_rv32_dup_top_two;
        pub use paste_emit_native_pop_top as emit_native_rv32_pop_top;
        pub use paste_emit_native_rot_two as emit_native_rv32_rot_two;
        pub use paste_emit_native_rot_three as emit_native_rv32_rot_three;
        pub use paste_emit_native_jump as emit_native_rv32_jump;
        pub use paste_emit_native_pop_jump_if as emit_native_rv32_pop_jump_if;
        pub use paste_emit_native_jump_if_or_pop as emit_native_rv32_jump_if_or_pop;
        pub use paste_emit_native_unwind_jump as emit_native_rv32_unwind_jump;
        pub use paste_emit_native_setup_block as emit_native_rv32_setup_block;
        pub use paste_emit_native_with_cleanup as emit_native_rv32_with_cleanup;
        pub use paste_emit_native_async_with_setup_finally as emit_native_rv32_async_with_setup_finally;
        pub use paste_emit_native_get_iter as emit_native_rv32_get_iter;
        pub use paste_emit_native_for_iter as emit_native_rv32_for_iter;
        pub use paste_emit_native_for_iter_end as emit_native_rv32_for_iter_end;
        pub use paste_emit_native_pop_except_jump as emit_native_rv32_pop_except_jump;
        pub use paste_emit_native_end_finally as emit_native_rv32_end_finally;
        pub use paste_emit_native_unary_op as emit_native_rv32_unary_op;
        pub use paste_emit_native_binary_op as emit_native_rv32_binary_op;
        pub use paste_emit_native_build as emit_native_rv32_build;
        pub use paste_emit_native_store_map as emit_native_rv32_store_map;
        pub use paste_emit_native_store_comp as emit_native_rv32_store_comp;
        pub use paste_emit_native_unpack_sequence as emit_native_rv32_unpack_sequence;
        pub use paste_emit_native_unpack_ex as emit_native_rv32_unpack_ex;
        pub use paste_emit_native_make_function as emit_native_rv32_make_function;
        pub use paste_emit_native_make_closure as emit_native_rv32_make_closure;
        pub use paste_emit_native_call_function as emit_native_rv32_call_function;
        pub use paste_emit_native_call_method as emit_native_rv32_call_method;
        pub use paste_emit_native_return_value as emit_native_rv32_return_value;
        pub use paste_emit_native_raise_varargs as emit_native_rv32_raise_varargs;
        pub use paste_emit_native_yield as emit_native_rv32_yield;
        pub use paste_emit_native_start_except_handler as emit_native_rv32_start_except_handler;
        pub use paste_emit_native_end_except_handler as emit_native_rv32_end_except_handler;
    };
    (thumb) => {
        pub use paste_emit_native_new as emit_native_thumb_new;
        pub use paste_emit_native_free as emit_native_thumb_free;
        pub use paste_emit_native_end_pass as emit_native_thumb_end_pass;
        pub use paste_emit_native_start_pass as emit_native_thumb_start_pass;
        pub use paste_emit_native_set_source_line as emit_native_thumb_set_source_line;
        pub use paste_emit_native_adjust_stack_size as emit_native_thumb_adjust_stack_size;
        pub use paste_emit_native_load_local as emit_native_thumb_load_local;
        pub use paste_emit_native_load_global as emit_native_thumb_load_global;
        pub use paste_emit_native_store_local as emit_native_thumb_store_local;
        pub use paste_emit_native_store_global as emit_native_thumb_store_global;
        pub use paste_emit_native_delete_local as emit_native_thumb_delete_local;
        pub use paste_emit_native_delete_global as emit_native_thumb_delete_global;
        pub use paste_emit_native_label_assign as emit_native_thumb_label_assign;
        pub use paste_emit_native_import as emit_native_thumb_import;
        pub use paste_emit_native_load_const_tok as emit_native_thumb_load_const_tok;
        pub use paste_emit_native_load_const_small_int as emit_native_thumb_load_const_small_int;
        pub use paste_emit_native_load_const_str as emit_native_thumb_load_const_str;
        pub use paste_emit_native_load_const_obj as emit_native_thumb_load_const_obj;
        pub use paste_emit_native_load_null as emit_native_thumb_load_null;
        pub use paste_emit_native_load_method as emit_native_thumb_load_method;
        pub use paste_emit_native_load_build_class as emit_native_thumb_load_build_class;
        pub use paste_emit_native_subscr as emit_native_thumb_subscr;
        pub use paste_emit_native_attr as emit_native_thumb_attr;
        pub use paste_emit_native_dup_top as emit_native_thumb_dup_top;
        pub use paste_emit_native_dup_top_two as emit_native_thumb_dup_top_two;
        pub use paste_emit_native_pop_top as emit_native_thumb_pop_top;
        pub use paste_emit_native_rot_two as emit_native_thumb_rot_two;
        pub use paste_emit_native_rot_three as emit_native_thumb_rot_three;
        pub use paste_emit_native_jump as emit_native_thumb_jump;
        pub use paste_emit_native_pop_jump_if as emit_native_thumb_pop_jump_if;
        pub use paste_emit_native_jump_if_or_pop as emit_native_thumb_jump_if_or_pop;
        pub use paste_emit_native_unwind_jump as emit_native_thumb_unwind_jump;
        pub use paste_emit_native_setup_block as emit_native_thumb_setup_block;
        pub use paste_emit_native_with_cleanup as emit_native_thumb_with_cleanup;
        pub use paste_emit_native_async_with_setup_finally as emit_native_thumb_async_with_setup_finally;
        pub use paste_emit_native_get_iter as emit_native_thumb_get_iter;
        pub use paste_emit_native_for_iter as emit_native_thumb_for_iter;
        pub use paste_emit_native_for_iter_end as emit_native_thumb_for_iter_end;
        pub use paste_emit_native_pop_except_jump as emit_native_thumb_pop_except_jump;
        pub use paste_emit_native_end_finally as emit_native_thumb_end_finally;
        pub use paste_emit_native_unary_op as emit_native_thumb_unary_op;
        pub use paste_emit_native_binary_op as emit_native_thumb_binary_op;
        pub use paste_emit_native_build as emit_native_thumb_build;
        pub use paste_emit_native_store_map as emit_native_thumb_store_map;
        pub use paste_emit_native_store_comp as emit_native_thumb_store_comp;
        pub use paste_emit_native_unpack_sequence as emit_native_thumb_unpack_sequence;
        pub use paste_emit_native_unpack_ex as emit_native_thumb_unpack_ex;
        pub use paste_emit_native_make_function as emit_native_thumb_make_function;
        pub use paste_emit_native_make_closure as emit_native_thumb_make_closure;
        pub use paste_emit_native_call_function as emit_native_thumb_call_function;
        pub use paste_emit_native_call_method as emit_native_thumb_call_method;
        pub use paste_emit_native_return_value as emit_native_thumb_return_value;
        pub use paste_emit_native_raise_varargs as emit_native_thumb_raise_varargs;
        pub use paste_emit_native_yield as emit_native_thumb_yield;
        pub use paste_emit_native_start_except_handler as emit_native_thumb_start_except_handler;
        pub use paste_emit_native_end_except_handler as emit_native_thumb_end_except_handler;
    };
    (x64) => {
        pub use paste_emit_native_new as emit_native_x64_new;
        pub use paste_emit_native_free as emit_native_x64_free;
        pub use paste_emit_native_end_pass as emit_native_x64_end_pass;
        pub use paste_emit_native_start_pass as emit_native_x64_start_pass;
        pub use paste_emit_native_set_source_line as emit_native_x64_set_source_line;
        pub use paste_emit_native_adjust_stack_size as emit_native_x64_adjust_stack_size;
        pub use paste_emit_native_load_local as emit_native_x64_load_local;
        pub use paste_emit_native_load_global as emit_native_x64_load_global;
        pub use paste_emit_native_store_local as emit_native_x64_store_local;
        pub use paste_emit_native_store_global as emit_native_x64_store_global;
        pub use paste_emit_native_delete_local as emit_native_x64_delete_local;
        pub use paste_emit_native_delete_global as emit_native_x64_delete_global;
        pub use paste_emit_native_label_assign as emit_native_x64_label_assign;
        pub use paste_emit_native_import as emit_native_x64_import;
        pub use paste_emit_native_load_const_tok as emit_native_x64_load_const_tok;
        pub use paste_emit_native_load_const_small_int as emit_native_x64_load_const_small_int;
        pub use paste_emit_native_load_const_str as emit_native_x64_load_const_str;
        pub use paste_emit_native_load_const_obj as emit_native_x64_load_const_obj;
        pub use paste_emit_native_load_null as emit_native_x64_load_null;
        pub use paste_emit_native_load_method as emit_native_x64_load_method;
        pub use paste_emit_native_load_build_class as emit_native_x64_load_build_class;
        pub use paste_emit_native_subscr as emit_native_x64_subscr;
        pub use paste_emit_native_attr as emit_native_x64_attr;
        pub use paste_emit_native_dup_top as emit_native_x64_dup_top;
        pub use paste_emit_native_dup_top_two as emit_native_x64_dup_top_two;
        pub use paste_emit_native_pop_top as emit_native_x64_pop_top;
        pub use paste_emit_native_rot_two as emit_native_x64_rot_two;
        pub use paste_emit_native_rot_three as emit_native_x64_rot_three;
        pub use paste_emit_native_jump as emit_native_x64_jump;
        pub use paste_emit_native_pop_jump_if as emit_native_x64_pop_jump_if;
        pub use paste_emit_native_jump_if_or_pop as emit_native_x64_jump_if_or_pop;
        pub use paste_emit_native_unwind_jump as emit_native_x64_unwind_jump;
        pub use paste_emit_native_setup_block as emit_native_x64_setup_block;
        pub use paste_emit_native_with_cleanup as emit_native_x64_with_cleanup;
        pub use paste_emit_native_async_with_setup_finally as emit_native_x64_async_with_setup_finally;
        pub use paste_emit_native_async_with_ret_unwind_enter as emit_native_x64_async_with_ret_unwind_enter;
        pub use paste_emit_native_get_iter as emit_native_x64_get_iter;
        pub use paste_emit_native_for_iter as emit_native_x64_for_iter;
        pub use paste_emit_native_for_iter_end as emit_native_x64_for_iter_end;
        pub use paste_emit_native_pop_except_jump as emit_native_x64_pop_except_jump;
        pub use paste_emit_native_end_finally as emit_native_x64_end_finally;
        pub use paste_emit_native_unary_op as emit_native_x64_unary_op;
        pub use paste_emit_native_binary_op as emit_native_x64_binary_op;
        pub use paste_emit_native_build as emit_native_x64_build;
        pub use paste_emit_native_store_map as emit_native_x64_store_map;
        pub use paste_emit_native_store_comp as emit_native_x64_store_comp;
        pub use paste_emit_native_unpack_sequence as emit_native_x64_unpack_sequence;
        pub use paste_emit_native_unpack_ex as emit_native_x64_unpack_ex;
        pub use paste_emit_native_make_function as emit_native_x64_make_function;
        pub use paste_emit_native_make_closure as emit_native_x64_make_closure;
        pub use paste_emit_native_call_function as emit_native_x64_call_function;
        pub use paste_emit_native_call_method as emit_native_x64_call_method;
        pub use paste_emit_native_return_value as emit_native_x64_return_value;
        pub use paste_emit_native_raise_varargs as emit_native_x64_raise_varargs;
        pub use paste_emit_native_yield as emit_native_x64_yield;
        pub use paste_emit_native_start_except_handler as emit_native_x64_start_except_handler;
        pub use paste_emit_native_end_except_handler as emit_native_x64_end_except_handler;
    };
    (x86) => {
        pub use paste_emit_native_new as emit_native_x86_new;
        pub use paste_emit_native_free as emit_native_x86_free;
        pub use paste_emit_native_end_pass as emit_native_x86_end_pass;
        pub use paste_emit_native_start_pass as emit_native_x86_start_pass;
        pub use paste_emit_native_set_source_line as emit_native_x86_set_source_line;
        pub use paste_emit_native_adjust_stack_size as emit_native_x86_adjust_stack_size;
        pub use paste_emit_native_load_local as emit_native_x86_load_local;
        pub use paste_emit_native_load_global as emit_native_x86_load_global;
        pub use paste_emit_native_store_local as emit_native_x86_store_local;
        pub use paste_emit_native_store_global as emit_native_x86_store_global;
        pub use paste_emit_native_delete_local as emit_native_x86_delete_local;
        pub use paste_emit_native_delete_global as emit_native_x86_delete_global;
        pub use paste_emit_native_label_assign as emit_native_x86_label_assign;
        pub use paste_emit_native_import as emit_native_x86_import;
        pub use paste_emit_native_load_const_tok as emit_native_x86_load_const_tok;
        pub use paste_emit_native_load_const_small_int as emit_native_x86_load_const_small_int;
        pub use paste_emit_native_load_const_str as emit_native_x86_load_const_str;
        pub use paste_emit_native_load_const_obj as emit_native_x86_load_const_obj;
        pub use paste_emit_native_load_null as emit_native_x86_load_null;
        pub use paste_emit_native_load_method as emit_native_x86_load_method;
        pub use paste_emit_native_load_build_class as emit_native_x86_load_build_class;
        pub use paste_emit_native_subscr as emit_native_x86_subscr;
        pub use paste_emit_native_attr as emit_native_x86_attr;
        pub use paste_emit_native_dup_top as emit_native_x86_dup_top;
        pub use paste_emit_native_dup_top_two as emit_native_x86_dup_top_two;
        pub use paste_emit_native_pop_top as emit_native_x86_pop_top;
        pub use paste_emit_native_rot_two as emit_native_x86_rot_two;
        pub use paste_emit_native_rot_three as emit_native_x86_rot_three;
        pub use paste_emit_native_jump as emit_native_x86_jump;
        pub use paste_emit_native_pop_jump_if as emit_native_x86_pop_jump_if;
        pub use paste_emit_native_jump_if_or_pop as emit_native_x86_jump_if_or_pop;
        pub use paste_emit_native_unwind_jump as emit_native_x86_unwind_jump;
        pub use paste_emit_native_setup_block as emit_native_x86_setup_block;
        pub use paste_emit_native_with_cleanup as emit_native_x86_with_cleanup;
        pub use paste_emit_native_async_with_setup_finally as emit_native_x86_async_with_setup_finally;
        pub use paste_emit_native_get_iter as emit_native_x86_get_iter;
        pub use paste_emit_native_for_iter as emit_native_x86_for_iter;
        pub use paste_emit_native_for_iter_end as emit_native_x86_for_iter_end;
        pub use paste_emit_native_pop_except_jump as emit_native_x86_pop_except_jump;
        pub use paste_emit_native_end_finally as emit_native_x86_end_finally;
        pub use paste_emit_native_unary_op as emit_native_x86_unary_op;
        pub use paste_emit_native_binary_op as emit_native_x86_binary_op;
        pub use paste_emit_native_build as emit_native_x86_build;
        pub use paste_emit_native_store_map as emit_native_x86_store_map;
        pub use paste_emit_native_store_comp as emit_native_x86_store_comp;
        pub use paste_emit_native_unpack_sequence as emit_native_x86_unpack_sequence;
        pub use paste_emit_native_unpack_ex as emit_native_x86_unpack_ex;
        pub use paste_emit_native_make_function as emit_native_x86_make_function;
        pub use paste_emit_native_make_closure as emit_native_x86_make_closure;
        pub use paste_emit_native_call_function as emit_native_x86_call_function;
        pub use paste_emit_native_call_method as emit_native_x86_call_method;
        pub use paste_emit_native_return_value as emit_native_x86_return_value;
        pub use paste_emit_native_raise_varargs as emit_native_x86_raise_varargs;
        pub use paste_emit_native_yield as emit_native_x86_yield;
        pub use paste_emit_native_start_except_handler as emit_native_x86_start_except_handler;
        pub use paste_emit_native_end_except_handler as emit_native_x86_end_except_handler;
    };
    (xtensa) => {
        pub use paste_emit_native_new as emit_native_xtensa_new;
        pub use paste_emit_native_free as emit_native_xtensa_free;
        pub use paste_emit_native_end_pass as emit_native_xtensa_end_pass;
        pub use paste_emit_native_start_pass as emit_native_xtensa_start_pass;
        pub use paste_emit_native_set_source_line as emit_native_xtensa_set_source_line;
        pub use paste_emit_native_adjust_stack_size as emit_native_xtensa_adjust_stack_size;
        pub use paste_emit_native_load_local as emit_native_xtensa_load_local;
        pub use paste_emit_native_load_global as emit_native_xtensa_load_global;
        pub use paste_emit_native_store_local as emit_native_xtensa_store_local;
        pub use paste_emit_native_store_global as emit_native_xtensa_store_global;
        pub use paste_emit_native_delete_local as emit_native_xtensa_delete_local;
        pub use paste_emit_native_delete_global as emit_native_xtensa_delete_global;
        pub use paste_emit_native_label_assign as emit_native_xtensa_label_assign;
        pub use paste_emit_native_import as emit_native_xtensa_import;
        pub use paste_emit_native_load_const_tok as emit_native_xtensa_load_const_tok;
        pub use paste_emit_native_load_const_small_int as emit_native_xtensa_load_const_small_int;
        pub use paste_emit_native_load_const_str as emit_native_xtensa_load_const_str;
        pub use paste_emit_native_load_const_obj as emit_native_xtensa_load_const_obj;
        pub use paste_emit_native_load_null as emit_native_xtensa_load_null;
        pub use paste_emit_native_load_method as emit_native_xtensa_load_method;
        pub use paste_emit_native_load_build_class as emit_native_xtensa_load_build_class;
        pub use paste_emit_native_subscr as emit_native_xtensa_subscr;
        pub use paste_emit_native_attr as emit_native_xtensa_attr;
        pub use paste_emit_native_dup_top as emit_native_xtensa_dup_top;
        pub use paste_emit_native_dup_top_two as emit_native_xtensa_dup_top_two;
        pub use paste_emit_native_pop_top as emit_native_xtensa_pop_top;
        pub use paste_emit_native_rot_two as emit_native_xtensa_rot_two;
        pub use paste_emit_native_rot_three as emit_native_xtensa_rot_three;
        pub use paste_emit_native_jump as emit_native_xtensa_jump;
        pub use paste_emit_native_pop_jump_if as emit_native_xtensa_pop_jump_if;
        pub use paste_emit_native_jump_if_or_pop as emit_native_xtensa_jump_if_or_pop;
        pub use paste_emit_native_unwind_jump as emit_native_xtensa_unwind_jump;
        pub use paste_emit_native_setup_block as emit_native_xtensa_setup_block;
        pub use paste_emit_native_with_cleanup as emit_native_xtensa_with_cleanup;
        pub use paste_emit_native_async_with_setup_finally as emit_native_xtensa_async_with_setup_finally;
        pub use paste_emit_native_get_iter as emit_native_xtensa_get_iter;
        pub use paste_emit_native_for_iter as emit_native_xtensa_for_iter;
        pub use paste_emit_native_for_iter_end as emit_native_xtensa_for_iter_end;
        pub use paste_emit_native_pop_except_jump as emit_native_xtensa_pop_except_jump;
        pub use paste_emit_native_end_finally as emit_native_xtensa_end_finally;
        pub use paste_emit_native_unary_op as emit_native_xtensa_unary_op;
        pub use paste_emit_native_binary_op as emit_native_xtensa_binary_op;
        pub use paste_emit_native_build as emit_native_xtensa_build;
        pub use paste_emit_native_store_map as emit_native_xtensa_store_map;
        pub use paste_emit_native_store_comp as emit_native_xtensa_store_comp;
        pub use paste_emit_native_unpack_sequence as emit_native_xtensa_unpack_sequence;
        pub use paste_emit_native_unpack_ex as emit_native_xtensa_unpack_ex;
        pub use paste_emit_native_make_function as emit_native_xtensa_make_function;
        pub use paste_emit_native_make_closure as emit_native_xtensa_make_closure;
        pub use paste_emit_native_call_function as emit_native_xtensa_call_function;
        pub use paste_emit_native_call_method as emit_native_xtensa_call_method;
        pub use paste_emit_native_return_value as emit_native_xtensa_return_value;
        pub use paste_emit_native_raise_varargs as emit_native_xtensa_raise_varargs;
        pub use paste_emit_native_yield as emit_native_xtensa_yield;
        pub use paste_emit_native_start_except_handler as emit_native_xtensa_start_except_handler;
        pub use paste_emit_native_end_except_handler as emit_native_xtensa_end_except_handler;
    };
    (xtensawin) => {
        pub use paste_emit_native_new as emit_native_xtensawin_new;
        pub use paste_emit_native_free as emit_native_xtensawin_free;
        pub use paste_emit_native_end_pass as emit_native_xtensawin_end_pass;
        pub use paste_emit_native_start_pass as emit_native_xtensawin_start_pass;
        pub use paste_emit_native_set_source_line as emit_native_xtensawin_set_source_line;
        pub use paste_emit_native_adjust_stack_size as emit_native_xtensawin_adjust_stack_size;
        pub use paste_emit_native_load_local as emit_native_xtensawin_load_local;
        pub use paste_emit_native_load_global as emit_native_xtensawin_load_global;
        pub use paste_emit_native_store_local as emit_native_xtensawin_store_local;
        pub use paste_emit_native_store_global as emit_native_xtensawin_store_global;
        pub use paste_emit_native_delete_local as emit_native_xtensawin_delete_local;
        pub use paste_emit_native_delete_global as emit_native_xtensawin_delete_global;
        pub use paste_emit_native_label_assign as emit_native_xtensawin_label_assign;
        pub use paste_emit_native_import as emit_native_xtensawin_import;
        pub use paste_emit_native_load_const_tok as emit_native_xtensawin_load_const_tok;
        pub use paste_emit_native_load_const_small_int as emit_native_xtensawin_load_const_small_int;
        pub use paste_emit_native_load_const_str as emit_native_xtensawin_load_const_str;
        pub use paste_emit_native_load_const_obj as emit_native_xtensawin_load_const_obj;
        pub use paste_emit_native_load_null as emit_native_xtensawin_load_null;
        pub use paste_emit_native_load_method as emit_native_xtensawin_load_method;
        pub use paste_emit_native_load_build_class as emit_native_xtensawin_load_build_class;
        pub use paste_emit_native_subscr as emit_native_xtensawin_subscr;
        pub use paste_emit_native_attr as emit_native_xtensawin_attr;
        pub use paste_emit_native_dup_top as emit_native_xtensawin_dup_top;
        pub use paste_emit_native_dup_top_two as emit_native_xtensawin_dup_top_two;
        pub use paste_emit_native_pop_top as emit_native_xtensawin_pop_top;
        pub use paste_emit_native_rot_two as emit_native_xtensawin_rot_two;
        pub use paste_emit_native_rot_three as emit_native_xtensawin_rot_three;
        pub use paste_emit_native_jump as emit_native_xtensawin_jump;
        pub use paste_emit_native_pop_jump_if as emit_native_xtensawin_pop_jump_if;
        pub use paste_emit_native_jump_if_or_pop as emit_native_xtensawin_jump_if_or_pop;
        pub use paste_emit_native_unwind_jump as emit_native_xtensawin_unwind_jump;
        pub use paste_emit_native_setup_block as emit_native_xtensawin_setup_block;
        pub use paste_emit_native_with_cleanup as emit_native_xtensawin_with_cleanup;
        pub use paste_emit_native_async_with_setup_finally as emit_native_xtensawin_async_with_setup_finally;
        pub use paste_emit_native_get_iter as emit_native_xtensawin_get_iter;
        pub use paste_emit_native_for_iter as emit_native_xtensawin_for_iter;
        pub use paste_emit_native_for_iter_end as emit_native_xtensawin_for_iter_end;
        pub use paste_emit_native_pop_except_jump as emit_native_xtensawin_pop_except_jump;
        pub use paste_emit_native_end_finally as emit_native_xtensawin_end_finally;
        pub use paste_emit_native_unary_op as emit_native_xtensawin_unary_op;
        pub use paste_emit_native_binary_op as emit_native_xtensawin_binary_op;
        pub use paste_emit_native_build as emit_native_xtensawin_build;
        pub use paste_emit_native_store_map as emit_native_xtensawin_store_map;
        pub use paste_emit_native_store_comp as emit_native_xtensawin_store_comp;
        pub use paste_emit_native_unpack_sequence as emit_native_xtensawin_unpack_sequence;
        pub use paste_emit_native_unpack_ex as emit_native_xtensawin_unpack_ex;
        pub use paste_emit_native_make_function as emit_native_xtensawin_make_function;
        pub use paste_emit_native_make_closure as emit_native_xtensawin_make_closure;
        pub use paste_emit_native_call_function as emit_native_xtensawin_call_function;
        pub use paste_emit_native_call_method as emit_native_xtensawin_call_method;
        pub use paste_emit_native_return_value as emit_native_xtensawin_return_value;
        pub use paste_emit_native_raise_varargs as emit_native_xtensawin_raise_varargs;
        pub use paste_emit_native_yield as emit_native_xtensawin_yield;
        pub use paste_emit_native_start_except_handler as emit_native_xtensawin_start_except_handler;
        pub use paste_emit_native_end_except_handler as emit_native_xtensawin_end_except_handler;
    };
}

/// Export `emit_native_<prefix>_new/free/...` wrappers for a backend type.
#[macro_export]
macro_rules! export_emit_native_prefixed {
    ($prefix:ident, $Backend:ty) => {
        $crate::export_emit_native!($Backend);
        $crate::alias_emit_native_exports!($prefix);
    };
}
