//! Bytecode vs native emitter dispatch for the compiler (`compile.c` method tables).
// symmetry: done

macro_rules! define_emit_dispatch {
    ($bc:ident, $($name:ident => $native:ident),* $(,)?) => {
        pub mod $bc {
            pub use crate::emitbc::{
                $($name),*
            };
        }

        pub mod native {
            $(
                pub use crate::emitnx64::$native as $name;
            )*
        }
    };
}

define_emit_dispatch! {
    bc,
    adjust_stack_size => emit_native_x64_adjust_stack_size,
    attr => emit_native_x64_attr,
    binary_op => emit_native_x64_binary_op,
    build => emit_native_x64_build,
    call_function => emit_native_x64_call_function,
    call_method => emit_native_x64_call_method,
    delete_global => emit_native_x64_delete_global,
    delete_local => emit_native_x64_delete_local,
    dup_top => emit_native_x64_dup_top,
    dup_top_two => emit_native_x64_dup_top_two,
    end_except_handler => emit_native_x64_end_except_handler,
    end_finally => emit_native_x64_end_finally,
    end_pass => emit_native_x64_end_pass,
    for_iter => emit_native_x64_for_iter,
    for_iter_end => emit_native_x64_for_iter_end,
    get_iter => emit_native_x64_get_iter,
    import => emit_native_x64_import,
    jump => emit_native_x64_jump,
    jump_if_or_pop => emit_native_x64_jump_if_or_pop,
    label_assign => emit_native_x64_label_assign,
    load_build_class => emit_native_x64_load_build_class,
    load_const_obj => emit_native_x64_load_const_obj,
    load_const_small_int => emit_native_x64_load_const_small_int,
    load_const_str => emit_native_x64_load_const_str,
    load_const_tok => emit_native_x64_load_const_tok,
    load_global => emit_native_x64_load_global,
    load_local => emit_native_x64_load_local,
    load_method => emit_native_x64_load_method,
    load_null => emit_native_x64_load_null,
    make_closure => emit_native_x64_make_closure,
    make_function => emit_native_x64_make_function,
    pop_except_jump => emit_native_x64_pop_except_jump,
    pop_jump_if => emit_native_x64_pop_jump_if,
    pop_top => emit_native_x64_pop_top,
    raise_varargs => emit_native_x64_raise_varargs,
    return_value => emit_native_x64_return_value,
    rot_three => emit_native_x64_rot_three,
    rot_two => emit_native_x64_rot_two,
    set_source_line => emit_native_x64_set_source_line,
    setup_block => emit_native_x64_setup_block,
    async_with_setup_finally => emit_native_x64_async_with_setup_finally,
    start_except_handler => emit_native_x64_start_except_handler,
    start_pass => emit_native_x64_start_pass,
    store_comp => emit_native_x64_store_comp,
    store_global => emit_native_x64_store_global,
    store_local => emit_native_x64_store_local,
    store_map => emit_native_x64_store_map,
    subscr => emit_native_x64_subscr,
    unary_op => emit_native_x64_unary_op,
    unpack_ex => emit_native_x64_unpack_ex,
    unpack_sequence => emit_native_x64_unpack_sequence,
    unwind_jump => emit_native_x64_unwind_jump,
    with_cleanup => emit_native_x64_with_cleanup,
    yield_ => emit_native_x64_yield,
}
