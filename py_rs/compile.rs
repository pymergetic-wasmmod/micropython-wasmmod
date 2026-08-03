//! rewrite of py/compile.c + py/compile.h
// symmetry: done

#![allow(unused_labels, non_snake_case, clippy::too_many_arguments)]

use crate::asmbase;
use crate::bc::ModuleContext;
use crate::bc0;
use crate::emit::{self, EmitCommon, EmitIdOps, PassKind};
use crate::emitbc;
use crate::emitdispatch;
use crate::emitglue::{self, CompiledModule, EMIT_OPT_BYTECODE, EMIT_OPT_NATIVE_PYTHON};
use crate::emitnative::{self, EMIT_OPT_VIPER};
use crate::emitnx64;
use crate::grammar::Rule;
use crate::lexer::TokenKind;
use crate::malloc;
use crate::map::{self, LookupKind, Map};
use crate::mpconfig;
use crate::mpstate;
use crate::nativeglue;
use crate::nlr;
use crate::obj::{self, Int, Obj};
use crate::objdict;
use crate::objexcept;
use crate::objstr;
use crate::parse::{self, ParseNode, ParseNodeStruct};
use crate::qstr::{self, Qstr};
use crate::raise::{self, MpRaise};
use crate::runtime0::{BinaryOp, UnaryOp};
use crate::scope::{self, IdInfo, IdInfoKind, Scope, ScopeKind};

const INVALID_LABEL: u16 = 0xffff;

pub const EXPOSE_MP_COMPILE_TO_RAW_CODE: bool = mpconfig::PY_BUILTINS_CODE
    >= mpconfig::PY_BUILTINS_CODE_BASIC
    || mpconfig::PERSISTENT_CODE_SAVE;

macro_rules! EMIT {
    ($comp:expr, $fun:ident) => {
        if $comp.use_native_emit {
            emitdispatch::native::$fun($comp.emit)
        } else {
            emitbc::$fun($comp.emit)
        }
    };
}
macro_rules! EMIT_ARG {
    ($comp:expr, $fun:ident $(, $args:expr)* $(,)?) => {
        if $comp.use_native_emit {
            emitdispatch::native::$fun($comp.emit $(, $args)*)
        } else {
            emitbc::$fun($comp.emit $(, $args)*)
        }
    };
}
macro_rules! EMIT_LOAD_FAST {
    ($comp:expr, $qst:expr, $local:expr) => {
        if $comp.use_native_emit {
            emitdispatch::native::load_local($comp.emit, $qst, $local, emit::EMIT_IDOP_LOCAL_FAST)
        } else {
            emitbc::load_local($comp.emit, $qst, $local, emit::EMIT_IDOP_LOCAL_FAST)
        }
    };
}
macro_rules! EMIT_LOAD_GLOBAL {
    ($comp:expr, $qst:expr) => {
        if $comp.use_native_emit {
            emitdispatch::native::load_global($comp.emit, $qst, emit::EMIT_IDOP_GLOBAL_GLOBAL)
        } else {
            emitbc::load_global($comp.emit, $qst, emit::EMIT_IDOP_GLOBAL_GLOBAL)
        }
    };
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum AssignKind {
    Store,
    AugLoad,
    AugStore,
}

struct Compiler {
    is_repl: bool,
    pass: PassKind,
    have_star: bool,
    compile_error: Obj,
    compile_error_line: usize,
    next_label: usize,
    num_dict_params: u16,
    num_default_params: u16,
    break_label: u16,
    continue_label: u16,
    cur_except_level: u16,
    break_continue_except_level: u16,
    scope_head: Option<*mut Scope>,
    scope_cur: Option<*mut Scope>,
    emit: *mut emit::Emit,
    emit_native: *mut emit::Emit,
    use_native_emit: bool,
    emit_common: EmitCommon,
}

fn comp_has_error(comp: &Compiler) -> bool {
    comp.compile_error != obj::OBJ_NULL
}

fn parse_node_testlist_comp_has_comp_for(pns: *mut ParseNodeStruct) -> bool {
    parse::parse_node_struct_num_nodes(pns) == 2
        && parse::parse_node_is_struct_kind(parse::parse_node_struct_node(pns, 1), Rule::CompFor)
}

fn emit_common_init(emit: &mut EmitCommon, source_file: Qstr) {
    if mpconfig::EMIT_BYTECODE_USES_QSTR_TABLE {
        map::init(&mut emit.qstr_map, 1);
        let elem = map::lookup(
            &mut emit.qstr_map,
            obj::new_qstr(source_file),
            LookupKind::AddIfNotFound,
        )
        .expect("qstr");
        elem.value = obj::new_small_int(0);
    }
    emit.const_obj_list.clear();
}

fn emit_common_start_pass(emit: &mut EmitCommon, pass: PassKind) {
    emit.pass = pass;
    if pass == PassKind::CodeSize {
        emit.children = if emit.ct_cur_child == 0 {
            core::ptr::null_mut()
        } else {
            malloc::new(emit.ct_cur_child).unwrap()
        };
    }
    emit.ct_cur_child = 0;
}

fn emit_common_populate_module_context(
    emit: &mut EmitCommon,
    _source_file: Qstr,
    context: *mut ModuleContext,
) {
    if mpconfig::EMIT_BYTECODE_USES_QSTR_TABLE {
        emitglue::module_context_alloc_tables(
            context,
            emit.qstr_map.used,
            emit.const_obj_list.len(),
        );
        unsafe {
            let ctx = &mut *context;
            for (i, elem) in emit.qstr_map.table.iter().enumerate() {
                if map::slot_is_filled(&emit.qstr_map, i) {
                    let idx = obj::small_int_value(elem.value) as usize;
                    ctx.qstr_table_mut()[idx] = obj::qstr_value(elem.key);
                }
            }
        }
    } else {
        emitglue::module_context_alloc_tables(context, 0, emit.const_obj_list.len());
    }
    unsafe {
        let ctx = &mut *context;
        for (i, &v) in emit.const_obj_list.iter().enumerate() {
            ctx.obj_table_mut()[i] = v;
        }
    }
}

fn compile_error_set_line(comp: &mut Compiler, pn: ParseNode) {
    if comp.compile_error_line == 0 && parse::parse_node_is_struct(pn) {
        comp.compile_error_line = unsafe { (*(pn as *const ParseNodeStruct)).source_line as usize };
    }
}

fn compile_syntax_error(comp: &mut Compiler, pn: ParseNode, msg: &'static [u8]) {
    if comp.compile_error == obj::OBJ_NULL {
        comp.compile_error = objexcept::new_exception_args(
            objexcept::type_syntax_error(),
            1,
            &[objstr::new_str(msg)],
        );
        compile_error_set_line(comp, pn);
    }
}

fn comp_next_label(comp: &mut Compiler) -> usize {
    let l = comp.next_label;
    comp.next_label += 1;
    l
}

fn reserve_labels_for_native(comp: &mut Compiler, n: usize) {
    if unsafe { (*comp.scope_cur.unwrap()).emit_options != EMIT_OPT_BYTECODE } {
        comp.next_label += n;
    }
}

fn scope_new_and_link(
    comp: &mut Compiler,
    kind: ScopeKind,
    pn: ParseNode,
    emit_options: u16,
) -> *mut Scope {
    let scope = scope::new(kind, pn, emit_options);
    unsafe {
        (*scope).parent = comp.scope_cur;
        (*scope).next = None;
    }
    if comp.scope_head.is_none() {
        comp.scope_head = Some(scope);
    } else {
        let mut s = comp.scope_head.unwrap();
        loop {
            if unsafe { (*s).next.is_none() } {
                unsafe { (*s).next = Some(scope) };
                break;
            }
            s = unsafe { (*s).next.unwrap() };
        }
    }
    scope
}

type ApplyListFn = fn(&mut Compiler, ParseNode);

fn apply_to_single_or_list(comp: &mut Compiler, pn: ParseNode, list_rule: Rule, f: ApplyListFn) {
    if parse::parse_node_is_struct_kind(pn, list_rule) {
        let pns = pn as *mut ParseNodeStruct;
        let n = parse::parse_node_struct_num_nodes(pns);
        for i in 0..n {
            f(comp, parse::parse_node_struct_node(pns, i));
        }
    } else if !parse::parse_node_is_null(pn) {
        f(comp, pn);
    }
}

fn compile_generic_all_nodes(comp: &mut Compiler, pns: *mut ParseNodeStruct) {
    let n = parse::parse_node_struct_num_nodes(pns);
    for i in 0..n {
        let pn = parse::parse_node_struct_node(pns, i);
        compile_node(comp, pn);
        if comp_has_error(comp) {
            compile_error_set_line(comp, pn);
            return;
        }
    }
}

fn compile_load_id(comp: &mut Compiler, qst: Qstr) {
    if comp.pass == PassKind::Scope {
        emit::emit_common_get_id_for_load(unsafe { &mut *comp.scope_cur.unwrap() }, qst);
    } else {
        compile_id_op(comp, EmitIdOps::Load, qst);
    }
}

fn compile_store_id(comp: &mut Compiler, qst: Qstr) {
    if comp.pass == PassKind::Scope {
        emit::emit_common_get_id_for_modification(unsafe { &mut *comp.scope_cur.unwrap() }, qst);
    } else {
        compile_id_op(comp, EmitIdOps::Store, qst);
    }
}

fn compile_delete_id(comp: &mut Compiler, qst: Qstr) {
    if comp.pass == PassKind::Scope {
        emit::emit_common_get_id_for_modification(unsafe { &mut *comp.scope_cur.unwrap() }, qst);
    } else {
        compile_id_op(comp, EmitIdOps::Delete, qst);
    }
}

fn compile_id_op(comp: &mut Compiler, ops: EmitIdOps, qst: Qstr) {
    let scope = unsafe { &*comp.scope_cur.unwrap() };
    let id = scope::find(scope, qst).expect("identifier must exist");
    if comp.use_native_emit {
        match id.kind {
            scope::IdInfoKind::GlobalImplicit | scope::IdInfoKind::GlobalImplicitAssigned => {
                match ops {
                    EmitIdOps::Load => emitdispatch::native::load_global(
                        comp.emit,
                        qst,
                        emit::EMIT_IDOP_GLOBAL_NAME,
                    ),
                    EmitIdOps::Store => emitdispatch::native::store_global(
                        comp.emit,
                        qst,
                        emit::EMIT_IDOP_GLOBAL_NAME,
                    ),
                    EmitIdOps::Delete => emitdispatch::native::delete_global(
                        comp.emit,
                        qst,
                        emit::EMIT_IDOP_GLOBAL_NAME,
                    ),
                }
            }
            scope::IdInfoKind::GlobalExplicit => match ops {
                EmitIdOps::Load => {
                    emitdispatch::native::load_global(comp.emit, qst, emit::EMIT_IDOP_GLOBAL_GLOBAL)
                }
                EmitIdOps::Store => emitdispatch::native::store_global(
                    comp.emit,
                    qst,
                    emit::EMIT_IDOP_GLOBAL_GLOBAL,
                ),
                EmitIdOps::Delete => emitdispatch::native::delete_global(
                    comp.emit,
                    qst,
                    emit::EMIT_IDOP_GLOBAL_GLOBAL,
                ),
            },
            scope::IdInfoKind::Local => match ops {
                EmitIdOps::Load => emitdispatch::native::load_local(
                    comp.emit,
                    qst,
                    id.local_num as usize,
                    emit::EMIT_IDOP_LOCAL_FAST,
                ),
                EmitIdOps::Store => emitdispatch::native::store_local(
                    comp.emit,
                    qst,
                    id.local_num as usize,
                    emit::EMIT_IDOP_LOCAL_FAST,
                ),
                EmitIdOps::Delete => emitdispatch::native::delete_local(
                    comp.emit,
                    qst,
                    id.local_num as usize,
                    emit::EMIT_IDOP_LOCAL_FAST,
                ),
            },
            scope::IdInfoKind::Cell | scope::IdInfoKind::Free => match ops {
                EmitIdOps::Load => emitdispatch::native::load_local(
                    comp.emit,
                    qst,
                    id.local_num as usize,
                    emit::EMIT_IDOP_LOCAL_DEREF,
                ),
                EmitIdOps::Store => emitdispatch::native::store_local(
                    comp.emit,
                    qst,
                    id.local_num as usize,
                    emit::EMIT_IDOP_LOCAL_DEREF,
                ),
                EmitIdOps::Delete => emitdispatch::native::delete_local(
                    comp.emit,
                    qst,
                    id.local_num as usize,
                    emit::EMIT_IDOP_LOCAL_DEREF,
                ),
            },
            _ => unreachable!("unexpected id kind"),
        }
    } else {
        emit::emit_common_id_op(comp.emit, ops, scope, qst);
    }
}

fn compile_generic_tuple(comp: &mut Compiler, pns: *mut ParseNodeStruct) {
    let n = parse::parse_node_struct_num_nodes(pns);
    for i in 0..n {
        compile_node(comp, parse::parse_node_struct_node(pns, i));
    }
    EMIT_ARG!(comp, build, n, emit::EMIT_BUILD_TUPLE);
}

fn scope_compute_things(scope: *mut Scope) {
    unsafe {
        let scope = &mut *scope;
        if scope.scope_flags & bc0::SCOPE_FLAG_VARARGS as u16 != 0 {
            let mut id_param: Option<usize> = None;
            for i in (0..scope.id_info.len()).rev() {
                let id = &mut scope.id_info[i];
                if id.flags & scope::ID_FLAG_IS_STAR_PARAM != 0 {
                    if let Some(j) = id_param {
                        let temp = scope.id_info[j];
                        scope.id_info[j] = scope.id_info[i];
                        scope.id_info[i] = temp;
                    }
                    break;
                } else if id_param.is_none() && id.flags == scope::ID_FLAG_IS_PARAM {
                    // Exact match like C: plain params only (not *args/**kwargs).
                    id_param = Some(i);
                }
            }
        }
        scope.num_locals = 0;
        for id in &mut scope.id_info {
            if scope.kind == ScopeKind::Class && id.qst == qstr::from_str("__class__") {
                continue;
            }
            if scope::scope_is_func_like(scope.kind) && id.kind == IdInfoKind::GlobalImplicit {
                id.kind = IdInfoKind::GlobalExplicit;
            }
            if id.kind == IdInfoKind::Local || id.flags & scope::ID_FLAG_IS_PARAM != 0 {
                id.local_num = scope.num_locals;
                scope.num_locals += 1;
            }
        }
        for id in &mut scope.id_info {
            if id.kind == IdInfoKind::Cell && id.flags & scope::ID_FLAG_IS_PARAM == 0 {
                id.local_num = scope.num_locals;
                scope.num_locals += 1;
            }
        }
        if scope.parent.is_some() {
            let parent = scope.parent.unwrap();
            let parent = &*parent;
            let mut num_free = 0u16;
            for pid in &parent.id_info {
                if pid.kind == IdInfoKind::Cell || pid.kind == IdInfoKind::Free {
                    for id in &mut scope.id_info {
                        if id.kind == IdInfoKind::Free && pid.qst == id.qst {
                            id.local_num = num_free;
                            num_free += 1;
                        }
                    }
                }
            }
            if num_free > 0 {
                for id in &mut scope.id_info {
                    if id.kind != IdInfoKind::Free || id.flags & scope::ID_FLAG_IS_PARAM != 0 {
                        id.local_num += num_free;
                    }
                }
                scope.num_pos_args += num_free;
                scope.num_locals += num_free;
            }
        }
    }
}

fn compile_scope(comp: &mut Compiler, scope: *mut Scope, pass: PassKind) -> bool {
    comp.pass = pass;
    comp.scope_cur = Some(scope);
    comp.next_label = 0;
    emit_common_start_pass(&mut comp.emit_common, pass);
    EMIT_ARG!(comp, start_pass, pass, scope);
    reserve_labels_for_native(comp, 6);
    if pass == PassKind::Scope {
        unsafe {
            (*scope).stack_size = 0;
            (*scope).exc_stack_size = 0;
        }
    }
    unsafe {
        let s = &*scope;
        if parse::parse_node_is_struct_kind(s.pn, Rule::EvalInput) {
            debug_assert!(s.kind == ScopeKind::Module);
            let pns = s.pn as *mut ParseNodeStruct;
            compile_node(comp, parse::parse_node_struct_node(pns, 0));
            EMIT!(comp, return_value);
        } else if s.kind == ScopeKind::Module {
            compile_node(comp, s.pn);
            EMIT_ARG!(comp, load_const_tok, TokenKind::KwNone);
            EMIT!(comp, return_value);
        } else if s.kind == ScopeKind::Function {
            let pns = s.pn as *mut ParseNodeStruct;
            if comp.pass == PassKind::Scope {
                comp.have_star = false;
                apply_to_single_or_list(
                    comp,
                    parse::parse_node_struct_node(pns, 1),
                    Rule::Typedargslist,
                    compile_scope_func_param,
                );
                if mpconfig::ENABLE_NATIVE_CODE && s.emit_options == EMIT_OPT_VIPER {
                    let ret_type =
                        compile_viper_type_annotation(comp, parse::parse_node_struct_node(pns, 2));
                    (*scope).scope_flags |=
                        (ret_type as u16) << emitnative::MP_SCOPE_FLAG_VIPERRET_POS;
                }
            }
            compile_node(comp, parse::parse_node_struct_node(pns, 3));
            EMIT_ARG!(comp, load_const_tok, TokenKind::KwNone);
            EMIT!(comp, return_value);
        } else if s.kind == ScopeKind::Lambda {
            let pns = s.pn as *mut ParseNodeStruct;
            if comp.pass == PassKind::Scope {
                comp.have_star = false;
                apply_to_single_or_list(
                    comp,
                    parse::parse_node_struct_node(pns, 0),
                    Rule::Varargslist,
                    compile_scope_lambda_param,
                );
            }
            EMIT_ARG!(comp, set_source_line, (*pns).source_line as usize);
            compile_node(comp, parse::parse_node_struct_node(pns, 1));
            if s.scope_flags & bc0::SCOPE_FLAG_GENERATOR as u16 != 0 {
                EMIT!(comp, pop_top);
                EMIT_ARG!(comp, load_const_tok, TokenKind::KwNone);
            }
            EMIT!(comp, return_value);
        } else if scope::scope_is_comp_like(s.kind) {
            compile_comprehension_scope(comp, scope);
        } else {
            compile_class_scope(comp, scope);
        }
    }
    let pass_complete = EMIT!(comp, end_pass);
    debug_assert!(comp.cur_except_level == 0);
    pass_complete
}

fn compile_comprehension_scope(comp: &mut Compiler, scope: *mut Scope) {
    unsafe {
        let s = &mut *scope;
        let pns = s.pn as *mut ParseNodeStruct;
        let pns_comp_for = parse::parse_node_struct_node(pns, 1) as *mut ParseNodeStruct;
        let qstr_arg = qstr::from_str("");
        if comp.pass == PassKind::Scope {
            scope::find_or_add_id(s, qstr_arg, IdInfoKind::Local);
            s.num_pos_args = 1;
        }
        EMIT_ARG!(comp, set_source_line, (*pns).source_line as usize);
        match s.kind {
            ScopeKind::ListComp => EMIT_ARG!(comp, build, 0, emit::EMIT_BUILD_LIST),
            ScopeKind::DictComp => EMIT_ARG!(comp, build, 0, emit::EMIT_BUILD_MAP),
            ScopeKind::SetComp if mpconfig::PY_BUILTINS_SET => {
                EMIT_ARG!(comp, build, 0, emit::EMIT_BUILD_SET)
            }
            _ => {}
        }
        if s.kind == ScopeKind::GenExpr {
            EMIT!(comp, load_null);
            compile_load_id(comp, qstr_arg);
            EMIT!(comp, load_null);
            EMIT!(comp, load_null);
        } else {
            compile_load_id(comp, qstr_arg);
            EMIT_ARG!(comp, get_iter, true);
        }
        compile_scope_comp_iter(comp, pns_comp_for, parse::parse_node_struct_node(pns, 0), 0);
        if s.kind == ScopeKind::GenExpr {
            EMIT_ARG!(comp, load_const_tok, TokenKind::KwNone);
        }
        EMIT!(comp, return_value);
    }
}

fn compile_class_scope(comp: &mut Compiler, scope: *mut Scope) {
    unsafe {
        let s = &*scope;
        let pns = s.pn as *mut ParseNodeStruct;
        if comp.pass == PassKind::Scope {
            scope::find_or_add_id(&mut *scope, qstr::from_str("__class__"), IdInfoKind::Local);
        }
        compile_load_id(comp, qstr::from_str("__name__"));
        compile_store_id(comp, qstr::from_str("__module__"));
        EMIT_ARG!(
            comp,
            load_const_str,
            parse::parse_node_leaf_arg(parse::parse_node_struct_node(pns, 0))
        );
        compile_store_id(comp, qstr::from_str("__qualname__"));
        compile_node(comp, parse::parse_node_struct_node(pns, 2));
        if let Some(id) = scope::find(&*scope, qstr::from_str("__class__")) {
            if id.kind == IdInfoKind::Local {
                EMIT_ARG!(comp, load_const_tok, TokenKind::KwNone);
            } else {
                EMIT_LOAD_FAST!(comp, qstr::from_str("__class__"), id.local_num as usize);
            }
        }
        EMIT!(comp, return_value);
    }
}

fn compile_scope_comp_iter(
    comp: &mut Compiler,
    pns_comp_for: *mut ParseNodeStruct,
    pn_inner: ParseNode,
    for_depth: i32,
) {
    let l_top = comp_next_label(comp);
    let l_end = comp_next_label(comp);
    EMIT_ARG!(comp, label_assign, l_top);
    EMIT_ARG!(comp, for_iter, l_end);
    c_assign(
        comp,
        parse::parse_node_struct_node(pns_comp_for, 0),
        AssignKind::Store,
    );
    let mut pn_iter = parse::parse_node_struct_node(pns_comp_for, 2);
    // C uses `goto tail_recursion` after `comp_if` so we don't re-enter the same
    // `comp_for` (which would recurse forever on `[x for x in xs if cond]`).
    loop {
        if parse::parse_node_is_null(pn_iter) {
            compile_node(comp, pn_inner);
            if unsafe { (*comp.scope_cur.unwrap()).kind } == ScopeKind::GenExpr {
                EMIT_ARG!(comp, yield_, emit::EMIT_YIELD_VALUE);
                reserve_labels_for_native(comp, 2);
                EMIT!(comp, pop_top);
            } else {
                EMIT_ARG!(
                    comp,
                    store_comp,
                    unsafe { (*comp.scope_cur.unwrap()).kind },
                    (4 * for_depth + 5) as usize
                );
            }
            break;
        } else if parse::parse_node_is_struct_kind(pn_iter, Rule::CompIf) {
            let pns_if = pn_iter as *mut ParseNodeStruct;
            c_if_cond(comp, parse::parse_node_struct_node(pns_if, 0), false, l_top);
            pn_iter = parse::parse_node_struct_node(pns_if, 1);
            continue;
        } else {
            let pns_for2 = pn_iter as *mut ParseNodeStruct;
            compile_node(comp, parse::parse_node_struct_node(pns_for2, 1));
            EMIT_ARG!(comp, get_iter, true);
            compile_scope_comp_iter(comp, pns_for2, pn_inner, for_depth + 1);
            break;
        }
    }
    EMIT_ARG!(comp, jump, l_top);
    EMIT_ARG!(comp, label_assign, l_end);
    EMIT!(comp, for_iter_end);
}

fn c_if_cond(comp: &mut Compiler, pn: ParseNode, jump_if: bool, label: usize) {
    if parse::parse_node_is_const_false(pn) {
        if !jump_if {
            EMIT_ARG!(comp, jump, label);
        }
        return;
    }
    if parse::parse_node_is_const_true(pn) {
        if jump_if {
            EMIT_ARG!(comp, jump, label);
        }
        return;
    }
    if parse::parse_node_is_struct(pn) {
        let pns = pn as *mut ParseNodeStruct;
        let n = parse::parse_node_struct_num_nodes(pns);
        let kind = parse::parse_node_struct_kind(pns);
        if kind == Rule::OrTest as u32 {
            if !jump_if {
                let label2 = comp_next_label(comp);
                for i in 0..n - 1 {
                    c_if_cond(
                        comp,
                        parse::parse_node_struct_node(pns, i),
                        !jump_if,
                        label2,
                    );
                }
                c_if_cond(
                    comp,
                    parse::parse_node_struct_node(pns, n - 1),
                    jump_if,
                    label,
                );
                EMIT_ARG!(comp, label_assign, label2);
            } else {
                for i in 0..n {
                    c_if_cond(comp, parse::parse_node_struct_node(pns, i), jump_if, label);
                }
            }
            return;
        }
        if kind == Rule::AndTest as u32 {
            if !jump_if {
                for i in 0..n {
                    c_if_cond(comp, parse::parse_node_struct_node(pns, i), jump_if, label);
                }
            } else {
                let label2 = comp_next_label(comp);
                for i in 0..n - 1 {
                    c_if_cond(
                        comp,
                        parse::parse_node_struct_node(pns, i),
                        !jump_if,
                        label2,
                    );
                }
                c_if_cond(
                    comp,
                    parse::parse_node_struct_node(pns, n - 1),
                    jump_if,
                    label,
                );
                EMIT_ARG!(comp, label_assign, label2);
            }
            return;
        }
        if kind == Rule::NotTest2 as u32 {
            c_if_cond(comp, parse::parse_node_struct_node(pns, 0), !jump_if, label);
            return;
        }
    }
    compile_node(comp, pn);
    EMIT_ARG!(comp, pop_jump_if, jump_if, label);
}

fn c_assign(comp: &mut Compiler, pn: ParseNode, assign_kind: AssignKind) {
    if parse::parse_node_is_id(pn) {
        let q = parse::parse_node_leaf_arg(pn);
        match assign_kind {
            AssignKind::Store | AssignKind::AugStore => compile_store_id(comp, q),
            AssignKind::AugLoad => compile_load_id(comp, q),
        }
        return;
    }
    if parse::parse_node_is_leaf(pn) {
        compile_syntax_error(comp, pn, b"can't assign to expression");
        return;
    }
    let pns = pn as *mut ParseNodeStruct;
    match parse::parse_node_struct_kind(pns) as u8 {
        k if k == Rule::AtomExprNormal as u8 => c_assign_atom_expr(comp, pns, assign_kind),
        k if k == Rule::TestlistStarExpr as u8 || k == Rule::Exprlist as u8 => {
            // lhs is a tuple
            if assign_kind != AssignKind::Store {
                compile_syntax_error(comp, pn, b"can't assign to expression");
                return;
            }
            let n = parse::parse_node_struct_num_nodes(pns);
            let nodes: Vec<ParseNode> = (0..n)
                .map(|i| parse::parse_node_struct_node(pns, i))
                .collect();
            c_assign_tuple(comp, &nodes);
        }
        k if k == Rule::AtomParen as u8 => {
            // lhs is something in parenthesis
            let inner = parse::parse_node_struct_node(pns, 0);
            if parse::parse_node_is_null(inner) {
                // empty tuple
                compile_syntax_error(comp, pn, b"can't assign to expression");
            } else if assign_kind != AssignKind::Store {
                compile_syntax_error(comp, pn, b"can't assign to expression");
            } else if parse::parse_node_is_struct_kind(inner, Rule::TestlistComp) {
                let pns2 = inner as *mut ParseNodeStruct;
                c_assign_testlist_comp(comp, pn, pns2);
            } else {
                // parens around a single item (not const-folded into a tuple)
                c_assign(comp, inner, assign_kind);
            }
        }
        k if k == Rule::AtomBracket as u8 => {
            // lhs is something in brackets
            if assign_kind != AssignKind::Store {
                compile_syntax_error(comp, pn, b"can't assign to expression");
                return;
            }
            let inner = parse::parse_node_struct_node(pns, 0);
            if parse::parse_node_is_null(inner) {
                // empty list, assignment allowed
                c_assign_tuple(comp, &[]);
            } else if parse::parse_node_is_struct_kind(inner, Rule::TestlistComp) {
                let pns2 = inner as *mut ParseNodeStruct;
                c_assign_testlist_comp(comp, pn, pns2);
            } else {
                // brackets around 1 item
                c_assign_tuple(comp, &[inner]);
            }
        }
        _ => compile_syntax_error(comp, pn, b"can't assign to expression"),
    }
}

fn c_assign_testlist_comp(comp: &mut Compiler, pn: ParseNode, pns: *mut ParseNodeStruct) {
    // lhs is a sequence
    if parse_node_testlist_comp_has_comp_for(pns) {
        compile_syntax_error(comp, pn, b"can't assign to expression");
        return;
    }
    let n = parse::parse_node_struct_num_nodes(pns);
    let nodes: Vec<ParseNode> = (0..n)
        .map(|i| parse::parse_node_struct_node(pns, i))
        .collect();
    c_assign_tuple(comp, &nodes);
}

fn c_assign_atom_expr(comp: &mut Compiler, pns: *mut ParseNodeStruct, assign_kind: AssignKind) {
    if assign_kind != AssignKind::AugStore {
        compile_node(comp, parse::parse_node_struct_node(pns, 0));
    }
    let mut pns1 = parse::parse_node_struct_node(pns, 1);
    if parse::parse_node_is_struct(pns1) {
        let ps = pns1 as *mut ParseNodeStruct;
        if parse::parse_node_struct_kind(ps) == Rule::AtomExprTrailers as u32 {
            let n = parse::parse_node_struct_num_nodes(ps);
            if assign_kind != AssignKind::AugStore {
                for i in 0..n - 1 {
                    compile_node(comp, parse::parse_node_struct_node(ps, i));
                }
            }
            pns1 = parse::parse_node_struct_node(ps, n - 1);
        }
        if parse::parse_node_is_struct_kind(pns1, Rule::TrailerBracket) {
            let tb = pns1 as *mut ParseNodeStruct;
            if assign_kind == AssignKind::AugStore {
                EMIT!(comp, rot_three);
                EMIT_ARG!(comp, subscr, emit::EMIT_SUBSCR_STORE);
            } else {
                compile_node(comp, parse::parse_node_struct_node(tb, 0));
                if assign_kind == AssignKind::AugLoad {
                    EMIT!(comp, dup_top_two);
                    EMIT_ARG!(comp, subscr, emit::EMIT_SUBSCR_LOAD);
                } else {
                    EMIT_ARG!(comp, subscr, emit::EMIT_SUBSCR_STORE);
                }
            }
            return;
        }
        if parse::parse_node_is_struct_kind(pns1, Rule::TrailerPeriod) {
            let tp = pns1 as *mut ParseNodeStruct;
            let attr = parse::parse_node_leaf_arg(parse::parse_node_struct_node(tp, 0));
            if assign_kind == AssignKind::AugLoad {
                EMIT!(comp, dup_top);
                EMIT_ARG!(comp, attr, attr, emit::EMIT_ATTR_LOAD);
            } else {
                if assign_kind == AssignKind::AugStore {
                    EMIT!(comp, rot_two);
                }
                EMIT_ARG!(comp, attr, attr, emit::EMIT_ATTR_STORE);
            }
            return;
        }
    }
    compile_syntax_error(comp, pns as ParseNode, b"can't assign to expression");
}

fn c_assign_tuple(comp: &mut Compiler, nodes_tail: &[ParseNode]) {
    // look for star expression
    let n = nodes_tail.len();
    let mut have_star_index: Option<usize> = None;
    for (i, &node) in nodes_tail.iter().enumerate() {
        if parse::parse_node_is_struct_kind(node, Rule::StarExpr) {
            if have_star_index.is_none() {
                EMIT_ARG!(comp, unpack_ex, i, n - i - 1);
                have_star_index = Some(i);
            } else {
                compile_syntax_error(comp, node, b"multiple *x in assignment");
                return;
            }
        }
    }
    if have_star_index.is_none() {
        EMIT_ARG!(comp, unpack_sequence, n);
    }
    for (i, &node) in nodes_tail.iter().enumerate() {
        if Some(i) == have_star_index {
            let inner = parse::parse_node_struct_node(node as *mut ParseNodeStruct, 0);
            c_assign(comp, inner, AssignKind::Store);
        } else {
            c_assign(comp, node, AssignKind::Store);
        }
    }
}

fn compile_increase_except_level(comp: &mut Compiler, label: usize, kind: i32) {
    EMIT_ARG!(comp, setup_block, label, kind);
    comp.cur_except_level += 1;
    unsafe {
        let scope = comp.scope_cur.unwrap();
        if comp.cur_except_level > (*scope).exc_stack_size {
            (*scope).exc_stack_size = comp.cur_except_level;
        }
    }
}

fn compile_decrease_except_level(comp: &mut Compiler) {
    debug_assert!(comp.cur_except_level > 0);
    comp.cur_except_level -= 1;
    EMIT!(comp, end_finally);
    reserve_labels_for_native(comp, 2);
}

fn compile_yield_from(comp: &mut Compiler) {
    EMIT_ARG!(comp, get_iter, false);
    EMIT_ARG!(comp, load_const_tok, TokenKind::KwNone);
    EMIT_ARG!(comp, yield_, emit::EMIT_YIELD_FROM);
    reserve_labels_for_native(comp, 7);
}

fn compile_const_object(comp: &mut Compiler, pns: *mut ParseNodeStruct) {
    EMIT_ARG!(
        comp,
        load_const_obj,
        parse::parse_node_extract_const_object(pns)
    );
}

type CompileFn = fn(&mut Compiler, *mut ParseNodeStruct);

fn compile_dispatch(comp: &mut Compiler, pns: *mut ParseNodeStruct, kind: u8) {
    let rule = unsafe { core::mem::transmute::<u8, Rule>(kind) };
    match rule {
        Rule::FileInput
        | Rule::FileInput2
        | Rule::FileInput3
        | Rule::SimpleStmt2
        | Rule::PassStmt
        | Rule::SuiteBlockStmts => compile_generic_all_nodes(comp, pns),
        Rule::Testlist | Rule::Subscriptlist | Rule::TestlistStarExpr => {
            compile_generic_tuple(comp, pns)
        }
        Rule::ConstObject => compile_const_object(comp, pns),
        Rule::OrTest | Rule::AndTest => compile_or_and_test(comp, pns),
        Rule::NotTest2 => compile_not_test_2(comp, pns),
        Rule::Comparison => compile_comparison(comp, pns),
        Rule::StarExpr => compile_star_expr(comp, pns),
        Rule::Expr | Rule::XorExpr | Rule::AndExpr => compile_binary_op(comp, pns),
        Rule::ShiftExpr | Rule::ArithExpr | Rule::Term => compile_binop_from_tokens(comp, pns),
        Rule::Factor2 => compile_factor_2(comp, pns),
        Rule::Power => compile_power(comp, pns),
        Rule::AtomExprNormal => compile_atom_expr_normal(comp, pns),
        Rule::AtomExprAwait => compile_atom_expr_await(comp, pns),
        Rule::AtomParen => compile_atom_paren(comp, pns),
        Rule::AtomBracket => compile_atom_bracket(comp, pns),
        Rule::AtomBrace => compile_atom_brace(comp, pns),
        Rule::TrailerParen => compile_trailer_paren(comp, pns),
        Rule::TrailerBracket => compile_trailer_bracket(comp, pns),
        Rule::TrailerPeriod => compile_trailer_period(comp, pns),
        Rule::Subscript2 | Rule::Subscript3 => compile_subscript(comp, pns),
        Rule::DictorsetmakerItem => compile_dictorsetmaker_item(comp, pns),
        Rule::Funcdef => compile_funcdef(comp, pns),
        Rule::Classdef => compile_classdef(comp, pns),
        Rule::Decorated => compile_decorated(comp, pns),
        Rule::DelStmt => compile_del_stmt(comp, pns),
        Rule::BreakStmt | Rule::ContinueStmt => compile_break_cont_stmt(comp, pns),
        Rule::ReturnStmt => compile_return_stmt(comp, pns),
        Rule::YieldStmt => compile_yield_stmt(comp, pns),
        Rule::YieldExpr => compile_yield_expr(comp, pns),
        Rule::RaiseStmt => compile_raise_stmt(comp, pns),
        Rule::ImportName => compile_import_name(comp, pns),
        Rule::ImportFrom => compile_import_from(comp, pns),
        Rule::GlobalStmt | Rule::NonlocalStmt => compile_global_nonlocal_stmt(comp, pns),
        Rule::AssertStmt => compile_assert_stmt(comp, pns),
        Rule::IfStmt => compile_if_stmt(comp, pns),
        Rule::WhileStmt => compile_while_stmt(comp, pns),
        Rule::ForStmt => compile_for_stmt(comp, pns),
        Rule::TryStmt => compile_try_stmt(comp, pns),
        Rule::WithStmt => compile_with_stmt(comp, pns),
        Rule::AsyncStmt => compile_async_stmt(comp, pns),
        Rule::ExprStmt => compile_expr_stmt(comp, pns),
        Rule::TestIfExpr => compile_test_if_expr(comp, pns),
        Rule::Lambdef | Rule::LambdefNocond => compile_lambdef(comp, pns),
        Rule::NamedexprTest => compile_namedexpr(comp, pns),
        _ => compile_generic_all_nodes(comp, pns),
    }
}

fn compile_node(comp: &mut Compiler, pn: ParseNode) {
    if parse::parse_node_is_null(pn) {
        return;
    }
    if parse::parse_node_is_small_int(pn) {
        EMIT_ARG!(
            comp,
            load_const_small_int,
            parse::parse_node_leaf_small_int(pn) as i64
        );
        return;
    }
    if parse::parse_node_is_leaf(pn) {
        match parse::parse_node_leaf_kind(pn) {
            parse::PARSE_NODE_ID => compile_load_id(comp, parse::parse_node_leaf_arg(pn)),
            parse::PARSE_NODE_STRING => {
                EMIT_ARG!(comp, load_const_str, parse::parse_node_leaf_arg(pn))
            }
            parse::PARSE_NODE_TOKEN => {
                let arg = parse::parse_node_leaf_arg(pn);
                if arg != TokenKind::Newline as usize {
                    EMIT_ARG!(comp, load_const_tok, unsafe {
                        core::mem::transmute::<u8, TokenKind>(arg as u8)
                    });
                }
            }
            _ => {}
        }
        return;
    }
    let pns = pn as *mut ParseNodeStruct;
    unsafe {
        EMIT_ARG!(comp, set_source_line, (*pns).source_line as usize);
    }
    let kind = parse::parse_node_struct_kind(pns) as u8;
    debug_assert!(kind <= Rule::YieldArgFrom as u8);
    compile_dispatch(comp, pns, kind);
}

// --- statement/expression compilers (bytecode path) ---

fn compile_or_and_test(comp: &mut Compiler, pns: *mut ParseNodeStruct) {
    let cond = parse::parse_node_struct_kind(pns) == Rule::OrTest as u32;
    let l_end = comp_next_label(comp);
    let n = parse::parse_node_struct_num_nodes(pns);
    for i in 0..n {
        compile_node(comp, parse::parse_node_struct_node(pns, i));
        if i + 1 < n {
            EMIT_ARG!(comp, jump_if_or_pop, cond, l_end);
        }
    }
    EMIT_ARG!(comp, label_assign, l_end);
}

fn compile_not_test_2(comp: &mut Compiler, pns: *mut ParseNodeStruct) {
    compile_node(comp, parse::parse_node_struct_node(pns, 0));
    EMIT_ARG!(comp, unary_op, UnaryOp::Not);
}

fn compile_comparison(comp: &mut Compiler, pns: *mut ParseNodeStruct) {
    let num_nodes = parse::parse_node_struct_num_nodes(pns);
    compile_node(comp, parse::parse_node_struct_node(pns, 0));
    let multi = num_nodes > 3;
    let l_fail = if multi { comp_next_label(comp) } else { 0 };
    let mut i = 1;
    while i + 1 < num_nodes {
        compile_node(comp, parse::parse_node_struct_node(pns, i + 1));
        if i + 2 < num_nodes {
            EMIT!(comp, dup_top);
            EMIT!(comp, rot_three);
        }
        let op_pn = parse::parse_node_struct_node(pns, i);
        if parse::parse_node_is_token(op_pn) {
            let tok = parse::parse_node_leaf_arg(op_pn);
            let op = if tok == TokenKind::KwIn as usize {
                BinaryOp::In
            } else {
                unsafe {
                    core::mem::transmute::<u8, BinaryOp>(
                        (tok - TokenKind::OpLess as usize) as u8 + BinaryOp::Less as u8,
                    )
                }
            };
            EMIT_ARG!(comp, binary_op, op);
        } else {
            let pns2 = op_pn as *mut ParseNodeStruct;
            if parse::parse_node_struct_kind(pns2) == Rule::CompOpNotIn as u32 {
                EMIT_ARG!(comp, binary_op, BinaryOp::NotIn);
            } else if parse::parse_node_is_null(parse::parse_node_struct_node(pns2, 0)) {
                EMIT_ARG!(comp, binary_op, BinaryOp::Is);
            } else {
                EMIT_ARG!(comp, binary_op, BinaryOp::IsNot);
            }
        }
        if i + 2 < num_nodes {
            EMIT_ARG!(comp, jump_if_or_pop, false, l_fail);
        }
        i += 2;
    }
    if multi {
        let l_end = comp_next_label(comp);
        EMIT_ARG!(comp, jump, l_end);
        EMIT_ARG!(comp, label_assign, l_fail);
        EMIT_ARG!(comp, adjust_stack_size, 1);
        EMIT!(comp, rot_two);
        EMIT!(comp, pop_top);
        EMIT_ARG!(comp, label_assign, l_end);
    }
}

fn compile_star_expr(comp: &mut Compiler, pns: *mut ParseNodeStruct) {
    compile_syntax_error(comp, pns as ParseNode, b"*x must be assignment target");
}

fn binary_op_for_rule(kind: u32) -> BinaryOp {
    unsafe {
        core::mem::transmute::<u8, BinaryOp>(
            (BinaryOp::Or as u8) + (kind - Rule::Expr as u32) as u8,
        )
    }
}

fn compile_binary_op(comp: &mut Compiler, pns: *mut ParseNodeStruct) {
    let binary_op = binary_op_for_rule(parse::parse_node_struct_kind(pns));
    let num_nodes = parse::parse_node_struct_num_nodes(pns);
    compile_node(comp, parse::parse_node_struct_node(pns, 0));
    for i in 1..num_nodes {
        compile_node(comp, parse::parse_node_struct_node(pns, i));
        EMIT_ARG!(comp, binary_op, binary_op);
    }
}

fn compile_binop_from_tokens(comp: &mut Compiler, pns: *mut ParseNodeStruct) {
    let num_nodes = parse::parse_node_struct_num_nodes(pns);
    compile_node(comp, parse::parse_node_struct_node(pns, 0));
    let mut i = 1;
    while i + 1 < num_nodes {
        compile_node(comp, parse::parse_node_struct_node(pns, i + 1));
        let tok = parse::parse_node_leaf_arg(parse::parse_node_struct_node(pns, i));
        let op = unsafe {
            core::mem::transmute::<u8, BinaryOp>(
                (tok - TokenKind::OpDblLess as usize) as u8 + BinaryOp::Lshift as u8,
            )
        };
        EMIT_ARG!(comp, binary_op, op);
        i += 2;
    }
}

fn compile_factor_2(comp: &mut Compiler, pns: *mut ParseNodeStruct) {
    compile_node(comp, parse::parse_node_struct_node(pns, 1));
    let tok = parse::parse_node_leaf_arg(parse::parse_node_struct_node(pns, 0));
    let op = if tok == TokenKind::OpTilde as usize {
        UnaryOp::Invert
    } else {
        unsafe { core::mem::transmute::<u8, UnaryOp>((tok - TokenKind::OpPlus as usize) as u8) }
    };
    EMIT_ARG!(comp, unary_op, op);
}

fn compile_power(comp: &mut Compiler, pns: *mut ParseNodeStruct) {
    compile_generic_all_nodes(comp, pns);
    EMIT_ARG!(comp, binary_op, BinaryOp::Power);
}

fn compile_trailer_paren_helper(
    comp: &mut Compiler,
    mut pn_arglist: ParseNode,
    is_method_call: bool,
    n_positional_extra: usize,
) {
    let mut nodes_ptr: *mut ParseNode = core::ptr::null_mut();
    let n_args = parse::parse_node_extract_list(&mut pn_arglist, Rule::Arglist, &mut nodes_ptr);
    let mut n_positional = n_positional_extra;
    let mut n_keyword = 0usize;
    let mut star_flags = 0u8;
    let mut star_args = 0u32;
    for i in 0..n_args {
        let arg = unsafe { *nodes_ptr.add(i) };
        if parse::parse_node_is_struct(arg) {
            let pns_arg = arg as *mut ParseNodeStruct;
            match parse::parse_node_struct_kind(pns_arg) {
                k if k == Rule::ArglistStar as u32 => {
                    if star_flags & emit::EMIT_STAR_FLAG_DOUBLE != 0 {
                        compile_syntax_error(comp, arg, b"* arg after **");
                        return;
                    }
                    if n_keyword > 0 {
                        compile_syntax_error(comp, arg, b"* arg after kwarg");
                        return;
                    }
                    star_flags |= emit::EMIT_STAR_FLAG_SINGLE;
                    star_args |= 1u32 << i;
                    compile_node(comp, parse::parse_node_struct_node(pns_arg, 0));
                    n_positional += 1;
                }
                k if k == Rule::ArglistDblStar as u32 => {
                    star_flags |= emit::EMIT_STAR_FLAG_DOUBLE;
                    EMIT!(comp, load_null);
                    compile_node(comp, parse::parse_node_struct_node(pns_arg, 0));
                    n_keyword += 1;
                }
                k if k == Rule::Argument as u32 => {
                    if mpconfig::PY_ASSIGN_EXPR
                        && parse::parse_node_is_struct_kind(
                            parse::parse_node_struct_node(pns_arg, 1),
                            Rule::Argument3,
                        )
                    {
                        let pns3 =
                            parse::parse_node_struct_node(pns_arg, 1) as *mut ParseNodeStruct;
                        compile_namedexpr_helper(
                            comp,
                            parse::parse_node_struct_node(pns_arg, 0),
                            parse::parse_node_struct_node(pns3, 0),
                        );
                        n_positional += 1;
                    } else if !parse::parse_node_is_struct_kind(
                        parse::parse_node_struct_node(pns_arg, 1),
                        Rule::CompFor,
                    ) {
                        if !parse::parse_node_is_id(parse::parse_node_struct_node(pns_arg, 0)) {
                            compile_syntax_error(comp, arg, b"LHS of keyword arg must be an id");
                            return;
                        }
                        EMIT_ARG!(
                            comp,
                            load_const_str,
                            parse::parse_node_leaf_arg(parse::parse_node_struct_node(pns_arg, 0))
                        );
                        compile_node(comp, parse::parse_node_struct_node(pns_arg, 1));
                        n_keyword += 1;
                    } else {
                        compile_comprehension(comp, pns_arg, ScopeKind::GenExpr);
                        n_positional += 1;
                    }
                }
                _ => {
                    if star_flags & emit::EMIT_STAR_FLAG_DOUBLE != 0 {
                        compile_syntax_error(comp, arg, b"positional arg after **");
                        return;
                    }
                    if n_keyword > 0 {
                        compile_syntax_error(comp, arg, b"positional arg after keyword arg");
                        return;
                    }
                    compile_node(comp, arg);
                    n_positional += 1;
                }
            }
        } else {
            if star_flags & emit::EMIT_STAR_FLAG_DOUBLE != 0 {
                compile_syntax_error(comp, arg, b"positional arg after **");
                return;
            }
            if n_keyword > 0 {
                compile_syntax_error(comp, arg, b"positional arg after keyword arg");
                return;
            }
            compile_node(comp, arg);
            n_positional += 1;
        }
    }
    if star_flags != 0 {
        EMIT_ARG!(comp, load_const_small_int, star_args as i64);
    }
    if is_method_call {
        EMIT_ARG!(comp, call_method, n_positional, n_keyword, star_flags);
    } else {
        EMIT_ARG!(comp, call_function, n_positional, n_keyword, star_flags);
    }
}

fn compile_comprehension(comp: &mut Compiler, pns: *mut ParseNodeStruct, kind: ScopeKind) {
    debug_assert!(parse::parse_node_struct_num_nodes(pns) == 2);
    let pns_comp_for = parse::parse_node_struct_node(pns, 1) as *mut ParseNodeStruct;
    if comp.pass == PassKind::Scope {
        let emit_options = unsafe { (*comp.scope_cur.unwrap()).emit_options };
        let s = scope_new_and_link(comp, kind, pns as ParseNode, emit_options);
        parse::parse_node_struct_set_node(pns_comp_for, 3, s as ParseNode);
    }
    let cscope = unsafe { parse::parse_node_struct_node(pns_comp_for, 3) as *mut Scope };
    close_over_variables_etc(comp, cscope, 0, 0);
    compile_node(comp, parse::parse_node_struct_node(pns_comp_for, 1));
    if kind == ScopeKind::GenExpr {
        EMIT_ARG!(comp, get_iter, false);
    }
    EMIT_ARG!(comp, call_function, 1, 0, 0);
}

fn compile_atom_expr_normal(comp: &mut Compiler, pns: *mut ParseNodeStruct) {
    let node0 = parse::parse_node_struct_node(pns, 0);
    // Compile the subject of the expression. Note: for the special super()
    // case below this pushes a (dead, unused) lookup of the "super" global;
    // that matches py/compile.c exactly, which relies on this to reserve the
    // stack slot that LOAD_SUPER_METHOD later collapses onto, so that
    // anything already on the value stack below this expression (e.g. `1` in
    // `1 + super().f()`) is not clobbered.
    compile_node(comp, node0);
    let trail_root = parse::parse_node_struct_node(pns, 1);
    if parse::parse_node_is_null(trail_root) {
        return;
    }
    let is_trailers = parse::parse_node_is_struct_kind(trail_root, Rule::AtomExprTrailers);
    let (num_trail, trail_pns) = if is_trailers {
        let ps = trail_root as *mut ParseNodeStruct;
        (parse::parse_node_struct_num_nodes(ps), ps)
    } else {
        (1usize, trail_root as *mut ParseNodeStruct)
    };
    let get_trail = |idx: usize| -> *mut ParseNodeStruct {
        if is_trailers {
            parse::parse_node_struct_node(trail_pns, idx) as *mut ParseNodeStruct
        } else {
            trail_pns
        }
    };

    let mut i = 0usize;

    // handle special super() call (mirrors py/compile.c compile_atom_expr_normal)
    if unsafe { (*comp.scope_cur.unwrap()).kind } == ScopeKind::Function
        && parse::parse_node_is_id(node0)
        && parse::parse_node_leaf_arg(node0) == qstr::from_str("super")
        && parse::parse_node_struct_kind(get_trail(0)) == Rule::TrailerParen as u32
        && parse::parse_node_is_null(parse::parse_node_struct_node(get_trail(0), 0))
    {
        // at this point we have matched "super()" within a function

        // load the class for super to search for a parent
        compile_load_id(comp, qstr::from_str("__class__"));

        // look for first argument to function (assumes it's "self")
        let mut found = false;
        unsafe {
            for id in &(*comp.scope_cur.unwrap()).id_info {
                if id.flags & scope::ID_FLAG_IS_PARAM != 0 {
                    compile_load_id(comp, id.qst);
                    found = true;
                    break;
                }
            }
        }
        if !found {
            compile_syntax_error(
                comp,
                get_trail(0) as ParseNode,
                b"super() can't find self", // really a TypeError
            );
            return;
        }

        if num_trail >= 3
            && parse::parse_node_struct_kind(get_trail(1)) == Rule::TrailerPeriod as u32
            && parse::parse_node_struct_kind(get_trail(2)) == Rule::TrailerParen as u32
        {
            // optimisation for method calls super().f(...), to eliminate heap allocation
            let pns_period = get_trail(1);
            let pns_paren = get_trail(2);
            EMIT_ARG!(
                comp,
                load_method,
                parse::parse_node_leaf_arg(parse::parse_node_struct_node(pns_period, 0)),
                true
            );
            compile_trailer_paren_helper(
                comp,
                parse::parse_node_struct_node(pns_paren, 0),
                true,
                0,
            );
            i = 3;
        } else {
            // a super() call
            EMIT_ARG!(comp, call_function, 2, 0, 0);
            i = 1;
        }
    }

    while i < num_trail {
        let pns_t = get_trail(i);
        if i + 1 < num_trail
            && parse::parse_node_struct_kind(pns_t) == Rule::TrailerPeriod as u32
            && parse::parse_node_struct_kind(get_trail(i + 1)) == Rule::TrailerParen as u32
        {
            let pns_paren = get_trail(i + 1);
            EMIT_ARG!(
                comp,
                load_method,
                parse::parse_node_leaf_arg(parse::parse_node_struct_node(pns_t, 0)),
                false
            );
            compile_trailer_paren_helper(
                comp,
                parse::parse_node_struct_node(pns_paren, 0),
                true,
                0,
            );
            i += 2;
            continue;
        }
        match parse::parse_node_struct_kind(pns_t) {
            k if k == Rule::TrailerParen as u32 => {
                compile_trailer_paren_helper(
                    comp,
                    parse::parse_node_struct_node(pns_t, 0),
                    false,
                    0,
                );
            }
            k if k == Rule::TrailerBracket as u32 => {
                compile_node(comp, parse::parse_node_struct_node(pns_t, 0));
                EMIT_ARG!(comp, subscr, emit::EMIT_SUBSCR_LOAD);
            }
            k if k == Rule::TrailerPeriod as u32 => {
                EMIT_ARG!(
                    comp,
                    attr,
                    parse::parse_node_leaf_arg(parse::parse_node_struct_node(pns_t, 0)),
                    emit::EMIT_ATTR_LOAD
                );
            }
            _ => {}
        }
        i += 1;
    }
}

fn compile_atom_expr_await(comp: &mut Compiler, pns: *mut ParseNodeStruct) {
    if !mpconfig::PY_ASYNC_AWAIT {
        compile_atom_expr_normal(comp, pns);
        return;
    }
    let scope_kind = unsafe { (*comp.scope_cur.unwrap()).kind };
    if scope_kind != ScopeKind::Function && scope_kind != ScopeKind::Lambda {
        compile_syntax_error(comp, pns as ParseNode, b"'await' outside function");
        return;
    }
    compile_atom_expr_normal(comp, pns);
    compile_yield_from(comp);
}

fn compile_atom_paren(comp: &mut Compiler, pns: *mut ParseNodeStruct) {
    if parse::parse_node_is_null(parse::parse_node_struct_node(pns, 0)) {
        EMIT_ARG!(comp, build, 0, emit::EMIT_BUILD_TUPLE);
    } else {
        let inner = parse::parse_node_struct_node(pns, 0);
        // py/compile.c's compile_atom_paren asserts this is PN_testlist_comp
        // (not PN_testlist, a distinct rule): a parenthesized multi-item
        // expression list is parsed as `testlist_comp`. Checking the wrong
        // rule here meant this branch was never taken for e.g. `(a, b)`
        // (only constant-folded tuples like `(1, 2)` happened to work,
        // since those are folded away entirely at parse time before
        // reaching here) — the elements got compiled by the generic
        // fallback dispatch, which pushes each item but never emits
        // BUILD_TUPLE, leaving the value stack unbalanced.
        if parse::parse_node_is_struct_kind(inner, Rule::TestlistComp) {
            let pns2 = inner as *mut ParseNodeStruct;
            if parse_node_testlist_comp_has_comp_for(pns2) {
                compile_comprehension(comp, pns2, ScopeKind::GenExpr);
            } else {
                compile_generic_tuple(comp, pns2);
            }
        } else {
            compile_node(comp, inner);
        }
    }
}

fn compile_atom_bracket(comp: &mut Compiler, pns: *mut ParseNodeStruct) {
    if parse::parse_node_is_null(parse::parse_node_struct_node(pns, 0)) {
        EMIT_ARG!(comp, build, 0, emit::EMIT_BUILD_LIST);
    } else if parse::parse_node_is_struct_kind(
        parse::parse_node_struct_node(pns, 0),
        Rule::TestlistComp,
    ) {
        let pns2 = parse::parse_node_struct_node(pns, 0) as *mut ParseNodeStruct;
        if parse_node_testlist_comp_has_comp_for(pns2) {
            compile_comprehension(comp, pns2, ScopeKind::ListComp);
        } else {
            compile_generic_all_nodes(comp, pns2);
            EMIT_ARG!(
                comp,
                build,
                parse::parse_node_struct_num_nodes(pns2),
                emit::EMIT_BUILD_LIST
            );
        }
    } else {
        compile_node(comp, parse::parse_node_struct_node(pns, 0));
        EMIT_ARG!(comp, build, 1, emit::EMIT_BUILD_LIST);
    }
}

fn compile_atom_brace_helper(comp: &mut Compiler, pns: *mut ParseNodeStruct, create_map: bool) {
    let pn = parse::parse_node_struct_node(pns, 0);
    if parse::parse_node_is_null(pn) {
        if create_map {
            EMIT_ARG!(comp, build, 0, emit::EMIT_BUILD_MAP);
        }
    } else if parse::parse_node_is_struct(pn) {
        let pns_inner = pn as *mut ParseNodeStruct;
        if parse::parse_node_struct_kind(pns_inner) == Rule::DictorsetmakerItem as u32 {
            if create_map {
                EMIT_ARG!(comp, build, 1, emit::EMIT_BUILD_MAP);
            }
            compile_node(comp, pn);
            EMIT!(comp, store_map);
        } else if parse::parse_node_struct_kind(pns_inner) == Rule::Dictorsetmaker as u32 {
            let pns1 = parse::parse_node_struct_node(pns_inner, 1) as *mut ParseNodeStruct;
            if parse::parse_node_struct_kind(pns1) == Rule::DictorsetmakerList as u32 {
                let mut nodes_ptr: *mut ParseNode = core::ptr::null_mut();
                let n = parse::parse_node_extract_list(
                    &mut parse::parse_node_struct_node(pns1, 0),
                    Rule::DictorsetmakerList2,
                    &mut nodes_ptr,
                );
                let is_dict = !mpconfig::PY_BUILTINS_SET
                    || parse::parse_node_is_struct_kind(
                        parse::parse_node_struct_node(pns_inner, 0),
                        Rule::DictorsetmakerItem,
                    );
                if is_dict {
                    if create_map {
                        EMIT_ARG!(comp, build, 1 + n, emit::EMIT_BUILD_MAP);
                    }
                    compile_node(comp, parse::parse_node_struct_node(pns_inner, 0));
                    EMIT!(comp, store_map);
                } else {
                    compile_node(comp, parse::parse_node_struct_node(pns_inner, 0));
                }
                for i in 0..n {
                    let pn_i = unsafe { *nodes_ptr.add(i) };
                    let is_key_value =
                        parse::parse_node_is_struct_kind(pn_i, Rule::DictorsetmakerItem);
                    compile_node(comp, pn_i);
                    if is_dict {
                        if !is_key_value {
                            compile_syntax_error(
                                comp,
                                pns as ParseNode,
                                b"expecting key:value for dict",
                            );
                            return;
                        }
                        EMIT!(comp, store_map);
                    } else if is_key_value {
                        compile_syntax_error(
                            comp,
                            pns as ParseNode,
                            b"expecting just a value for set",
                        );
                        return;
                    }
                }
                if !is_dict && mpconfig::PY_BUILTINS_SET {
                    EMIT_ARG!(comp, build, 1 + n, emit::EMIT_BUILD_SET);
                }
            } else {
                debug_assert!(parse::parse_node_struct_kind(pns1) == Rule::CompFor as u32);
                if !mpconfig::PY_BUILTINS_SET
                    || parse::parse_node_is_struct_kind(
                        parse::parse_node_struct_node(pns_inner, 0),
                        Rule::DictorsetmakerItem,
                    )
                {
                    compile_comprehension(comp, pns_inner, ScopeKind::DictComp);
                } else {
                    compile_comprehension(comp, pns_inner, ScopeKind::SetComp);
                }
            }
        }
    } else if mpconfig::PY_BUILTINS_SET {
        compile_node(comp, pn);
        EMIT_ARG!(comp, build, 1, emit::EMIT_BUILD_SET);
    }
}

fn compile_atom_brace(comp: &mut Compiler, pns: *mut ParseNodeStruct) {
    compile_atom_brace_helper(comp, pns, true);
}

fn compile_trailer_paren(comp: &mut Compiler, pns: *mut ParseNodeStruct) {
    compile_trailer_paren_helper(comp, parse::parse_node_struct_node(pns, 0), false, 0);
}

fn compile_trailer_bracket(comp: &mut Compiler, pns: *mut ParseNodeStruct) {
    compile_node(comp, parse::parse_node_struct_node(pns, 0));
    EMIT_ARG!(comp, subscr, emit::EMIT_SUBSCR_LOAD);
}

fn compile_trailer_period(comp: &mut Compiler, pns: *mut ParseNodeStruct) {
    EMIT_ARG!(
        comp,
        attr,
        parse::parse_node_leaf_arg(parse::parse_node_struct_node(pns, 0)),
        emit::EMIT_ATTR_LOAD
    );
}

fn compile_subscript(comp: &mut Compiler, pns: *mut ParseNodeStruct) {
    if !mpconfig::PY_BUILTINS_SLICE {
        compile_node(comp, parse::parse_node_struct_node(pns, 0));
        return;
    }
    let mut pns = pns;
    if parse::parse_node_struct_kind(pns) == Rule::Subscript2 as u32 {
        compile_node(comp, parse::parse_node_struct_node(pns, 0));
        pns = parse::parse_node_struct_node(pns, 1) as *mut ParseNodeStruct;
    } else {
        EMIT_ARG!(comp, load_const_tok, TokenKind::KwNone);
    }
    debug_assert!(parse::parse_node_struct_kind(pns) == Rule::Subscript3 as u32);
    let pn = parse::parse_node_struct_node(pns, 0);
    if parse::parse_node_is_null(pn) {
        EMIT_ARG!(comp, load_const_tok, TokenKind::KwNone);
        EMIT_ARG!(comp, build, 2, emit::EMIT_BUILD_SLICE);
    } else if parse::parse_node_is_struct(pn) {
        let pns2 = pn as *mut ParseNodeStruct;
        match parse::parse_node_struct_kind(pns2) {
            k if k == Rule::Subscript3c as u32 => {
                EMIT_ARG!(comp, load_const_tok, TokenKind::KwNone);
                let pn2 = parse::parse_node_struct_node(pns2, 0);
                if parse::parse_node_is_null(pn2) {
                    EMIT_ARG!(comp, build, 2, emit::EMIT_BUILD_SLICE);
                } else {
                    compile_node(comp, pn2);
                    EMIT_ARG!(comp, build, 3, emit::EMIT_BUILD_SLICE);
                }
            }
            k if k == Rule::Subscript3d as u32 => {
                compile_node(comp, parse::parse_node_struct_node(pns2, 0));
                let pns3 = parse::parse_node_struct_node(pns2, 1) as *mut ParseNodeStruct;
                debug_assert!(parse::parse_node_struct_kind(pns3) == Rule::Sliceop as u32);
                if parse::parse_node_is_null(parse::parse_node_struct_node(pns3, 0)) {
                    EMIT_ARG!(comp, build, 2, emit::EMIT_BUILD_SLICE);
                } else {
                    compile_node(comp, parse::parse_node_struct_node(pns3, 0));
                    EMIT_ARG!(comp, build, 3, emit::EMIT_BUILD_SLICE);
                }
            }
            _ => {
                compile_node(comp, pn);
                EMIT_ARG!(comp, build, 2, emit::EMIT_BUILD_SLICE);
            }
        }
    } else {
        compile_node(comp, pn);
        EMIT_ARG!(comp, build, 2, emit::EMIT_BUILD_SLICE);
    }
}

fn compile_dictorsetmaker_item(comp: &mut Compiler, pns: *mut ParseNodeStruct) {
    compile_node(comp, parse::parse_node_struct_node(pns, 1));
    compile_node(comp, parse::parse_node_struct_node(pns, 0));
}

fn close_over_variables_etc(
    comp: &mut Compiler,
    this_scope: *mut Scope,
    n_pos_defaults: i32,
    n_kw_defaults: i32,
) {
    unsafe {
        if n_kw_defaults > 0 {
            (*this_scope).scope_flags |= bc0::SCOPE_FLAG_DEFKWARGS as u16;
        }
        (*this_scope).num_def_pos_args = n_pos_defaults as u16;
    }
    if mpconfig::EMIT_NATIVE {
        // When creating a function/closure it will take a reference to the current globals.
        unsafe {
            (*comp.scope_cur.unwrap()).scope_flags |=
                (bc0::SCOPE_FLAG_REFGLOBALS | bc0::SCOPE_FLAG_HASCONSTS) as u16;
        }
    }
    let mut nfree = 0usize;
    unsafe {
        if (*comp.scope_cur.unwrap()).kind != ScopeKind::Module {
            for id in &(*comp.scope_cur.unwrap()).id_info {
                if id.kind == IdInfoKind::Cell || id.kind == IdInfoKind::Free {
                    for id2 in &(*this_scope).id_info {
                        if id2.kind == IdInfoKind::Free && id.qst == id2.qst {
                            EMIT_LOAD_FAST!(comp, id.qst, id.local_num as usize);
                            nfree += 1;
                        }
                    }
                }
            }
        }
    }
    if nfree == 0 {
        EMIT_ARG!(
            comp,
            make_function,
            this_scope,
            n_pos_defaults as usize,
            n_kw_defaults as usize
        );
    } else {
        EMIT_ARG!(
            comp,
            make_closure,
            this_scope,
            nfree,
            n_pos_defaults as usize,
            n_kw_defaults as usize
        );
    }
}

fn compile_scope_func_lambda_param(
    comp: &mut Compiler,
    pn: ParseNode,
    pn_name: Rule,
    pn_star: Rule,
    pn_dbl_star: Rule,
) {
    if comp.pass != PassKind::Scope {
        return;
    }
    unsafe {
        if (*comp.scope_cur.unwrap()).scope_flags & bc0::SCOPE_FLAG_VARKEYWORDS as u16 != 0 {
            compile_syntax_error(comp, pn, b"invalid syntax");
            return;
        }
    }
    let mut param_name = qstr::QSTR_NULL;
    let mut param_flag = scope::ID_FLAG_IS_PARAM;
    let mut pns: Option<*mut ParseNodeStruct> = None;
    if parse::parse_node_is_id(pn) {
        param_name = parse::parse_node_leaf_arg(pn);
        unsafe {
            if comp.have_star {
                (*comp.scope_cur.unwrap()).num_kwonly_args += 1;
            } else {
                (*comp.scope_cur.unwrap()).num_pos_args += 1;
            }
        }
    } else {
        let pns_pn = pn as *mut ParseNodeStruct;
        pns = Some(pns_pn);
        let kind = parse::parse_node_struct_kind(pns_pn);
        if kind == pn_name as u32 {
            param_name = parse::parse_node_leaf_arg(parse::parse_node_struct_node(pns_pn, 0));
            unsafe {
                if comp.have_star {
                    (*comp.scope_cur.unwrap()).num_kwonly_args += 1;
                } else {
                    (*comp.scope_cur.unwrap()).num_pos_args += 1;
                }
            }
        } else if kind == pn_star as u32 {
            if comp.have_star {
                compile_syntax_error(comp, pn, b"invalid syntax");
                return;
            }
            comp.have_star = true;
            param_flag |= scope::ID_FLAG_IS_STAR_PARAM;
            if parse::parse_node_is_null(parse::parse_node_struct_node(pns_pn, 0)) {
                pns = None;
            } else if parse::parse_node_is_id(parse::parse_node_struct_node(pns_pn, 0)) {
                unsafe {
                    (*comp.scope_cur.unwrap()).scope_flags |= bc0::SCOPE_FLAG_VARARGS as u16;
                }
                param_name = parse::parse_node_leaf_arg(parse::parse_node_struct_node(pns_pn, 0));
                pns = None;
            } else {
                unsafe {
                    (*comp.scope_cur.unwrap()).scope_flags |= bc0::SCOPE_FLAG_VARARGS as u16;
                }
                let pns_tfp = parse::parse_node_struct_node(pns_pn, 0) as *mut ParseNodeStruct;
                param_name = parse::parse_node_leaf_arg(parse::parse_node_struct_node(pns_tfp, 0));
                pns = Some(pns_tfp);
            }
        } else {
            debug_assert!(kind == pn_dbl_star as u32);
            param_name = parse::parse_node_leaf_arg(parse::parse_node_struct_node(pns_pn, 0));
            param_flag |= scope::ID_FLAG_IS_DBL_STAR_PARAM;
            unsafe {
                (*comp.scope_cur.unwrap()).scope_flags |= bc0::SCOPE_FLAG_VARKEYWORDS as u16;
            }
        }
    }
    if param_name != qstr::QSTR_NULL {
        let id_info = scope::find_or_add_id(
            unsafe { &mut *comp.scope_cur.unwrap() },
            param_name,
            IdInfoKind::Undecided,
        );
        if id_info.kind != IdInfoKind::Undecided {
            compile_syntax_error(comp, pn, b"argument name reused");
            return;
        }
        id_info.kind = IdInfoKind::Local;
        id_info.flags = param_flag;
        if mpconfig::ENABLE_NATIVE_CODE {
            unsafe {
                if (*comp.scope_cur.unwrap()).emit_options == EMIT_OPT_VIPER
                    && pn_name == Rule::TypedargslistName
                {
                    if let Some(pns_t) = pns {
                        let native_type = compile_viper_type_annotation(
                            comp,
                            parse::parse_node_struct_node(pns_t, 1),
                        );
                        id_info.flags |= native_type << scope::ID_FLAG_VIPER_TYPE_POS;
                    }
                }
            }
        }
    }
}

fn compile_scope_func_param(comp: &mut Compiler, pn: ParseNode) {
    compile_scope_func_lambda_param(
        comp,
        pn,
        Rule::TypedargslistName,
        Rule::TypedargslistStar,
        Rule::TypedargslistDblStar,
    );
}

fn compile_scope_lambda_param(comp: &mut Compiler, pn: ParseNode) {
    compile_scope_func_lambda_param(
        comp,
        pn,
        Rule::VarargslistName,
        Rule::VarargslistStar,
        Rule::VarargslistDblStar,
    );
}

fn compile_funcdef_lambdef_param(comp: &mut Compiler, pn: ParseNode) {
    let pn_kind = if parse::parse_node_is_id(pn) {
        -1i32
    } else {
        parse::parse_node_struct_kind(pn as *mut ParseNodeStruct) as i32
    };
    if pn_kind == Rule::TypedargslistStar as i32 || pn_kind == Rule::VarargslistStar as i32 {
        comp.have_star = true;
    } else if pn_kind == Rule::TypedargslistDblStar as i32
        || pn_kind == Rule::VarargslistDblStar as i32
    {
        // named double star
    } else {
        let (pn_id, pn_equal) = if pn_kind == -1 {
            (pn, parse::PARSE_NODE_NULL)
        } else if pn_kind == Rule::TypedargslistName as i32 {
            let pns = pn as *mut ParseNodeStruct;
            (
                parse::parse_node_struct_node(pns, 0),
                parse::parse_node_struct_node(pns, 2),
            )
        } else {
            let pns = pn as *mut ParseNodeStruct;
            (
                parse::parse_node_struct_node(pns, 0),
                parse::parse_node_struct_node(pns, 1),
            )
        };
        if parse::parse_node_is_null(pn_equal) {
            if !comp.have_star && comp.num_default_params != 0 {
                compile_syntax_error(comp, pn, b"non-default argument follows default argument");
                return;
            }
        } else if comp.have_star {
            comp.num_dict_params += 1;
            if comp.num_dict_params == 1 {
                if comp.num_default_params > 0 {
                    EMIT_ARG!(
                        comp,
                        build,
                        comp.num_default_params as usize,
                        emit::EMIT_BUILD_TUPLE
                    );
                } else {
                    EMIT!(comp, load_null);
                }
                EMIT_ARG!(comp, build, 0, emit::EMIT_BUILD_MAP);
            }
            compile_node(comp, pn_equal);
            EMIT_ARG!(comp, load_const_str, parse::parse_node_leaf_arg(pn_id));
            EMIT!(comp, store_map);
        } else {
            comp.num_default_params += 1;
            compile_node(comp, pn_equal);
        }
    }
}

fn compile_funcdef_lambdef(
    comp: &mut Compiler,
    scope: *mut Scope,
    pn_params: ParseNode,
    list_rule: Rule,
) {
    let orig_have_star = comp.have_star;
    let orig_num_dict_params = comp.num_dict_params;
    let orig_num_default_params = comp.num_default_params;
    comp.have_star = false;
    comp.num_dict_params = 0;
    comp.num_default_params = 0;
    apply_to_single_or_list(comp, pn_params, list_rule, compile_funcdef_lambdef_param);
    if comp_has_error(comp) {
        return;
    }
    if comp.num_default_params > 0 && comp.num_dict_params == 0 {
        EMIT_ARG!(
            comp,
            build,
            comp.num_default_params as usize,
            emit::EMIT_BUILD_TUPLE
        );
        EMIT!(comp, load_null);
    }
    close_over_variables_etc(
        comp,
        scope,
        comp.num_default_params as i32,
        comp.num_dict_params as i32,
    );
    comp.have_star = orig_have_star;
    comp.num_dict_params = orig_num_dict_params;
    comp.num_default_params = orig_num_default_params;
}

fn compile_funcdef_helper(
    comp: &mut Compiler,
    pns: *mut ParseNodeStruct,
    emit_options: u16,
) -> Qstr {
    if comp.pass == PassKind::Scope {
        let s = scope_new_and_link(comp, ScopeKind::Function, pns as ParseNode, emit_options);
        parse::parse_node_struct_set_node(pns, 4, s as ParseNode);
    }
    let fscope = unsafe { parse::parse_node_struct_node(pns, 4) as *mut Scope };
    compile_funcdef_lambdef(
        comp,
        fscope,
        parse::parse_node_struct_node(pns, 1),
        Rule::Typedargslist,
    );
    unsafe { (*fscope).simple_name }
}

fn compile_funcdef(comp: &mut Compiler, pns: *mut ParseNodeStruct) {
    let emit_options = unsafe { (*comp.scope_cur.unwrap()).emit_options };
    let fname = compile_funcdef_helper(comp, pns, emit_options);
    compile_store_id(comp, fname);
}

fn compile_viper_type_annotation(comp: &mut Compiler, pn_annotation: ParseNode) -> u8 {
    if parse::parse_node_is_null(pn_annotation) {
        return nativeglue::NATIVE_TYPE_OBJ as u8;
    }
    if parse::parse_node_is_id(pn_annotation) {
        let type_name = parse::parse_node_leaf_arg(pn_annotation);
        let native_type = nativeglue::native_type_from_qstr(type_name);
        if native_type < 0 {
            comp.compile_error = objexcept::new_exception_args(
                objexcept::type_viper_type_error(),
                1,
                &[objstr::new_str(b"unknown viper type")],
            );
            compile_error_set_line(comp, pn_annotation);
            return 0;
        }
        return native_type as u8;
    }
    compile_syntax_error(comp, pn_annotation, b"annotation must be an identifier");
    0
}

fn compile_classdef_helper(
    comp: &mut Compiler,
    pns: *mut ParseNodeStruct,
    emit_options: u16,
) -> Qstr {
    if comp.pass == PassKind::Scope {
        let s = scope_new_and_link(comp, ScopeKind::Class, pns as ParseNode, emit_options);
        parse::parse_node_struct_set_node(pns, 3, s as ParseNode);
    }
    EMIT!(comp, load_build_class);
    let cscope = unsafe { parse::parse_node_struct_node(pns, 3) as *mut Scope };
    close_over_variables_etc(comp, cscope, 0, 0);
    EMIT_ARG!(comp, load_const_str, unsafe { (*cscope).simple_name });
    let mut parents = parse::parse_node_struct_node(pns, 1);
    if parse::parse_node_is_struct_kind(parents, Rule::Classdef2) {
        parents = parse::PARSE_NODE_NULL;
    }
    compile_trailer_paren_helper(comp, parents, false, 2);
    unsafe { (*cscope).simple_name }
}

fn compile_classdef(comp: &mut Compiler, pns: *mut ParseNodeStruct) {
    let cname = compile_classdef_helper(comp, pns, unsafe {
        (*comp.scope_cur.unwrap()).emit_options
    });
    compile_store_id(comp, cname);
}

fn compile_built_in_decorator(
    comp: &mut Compiler,
    name_len: usize,
    name_nodes: &[ParseNode],
    emit_options: &mut u16,
) -> bool {
    if parse::parse_node_leaf_arg(name_nodes[0]) != qstr::from_str("micropython") {
        return false;
    }
    if name_len != 2 {
        compile_syntax_error(comp, name_nodes[0], b"invalid micropython decorator");
        return true;
    }
    if parse::parse_node_leaf_arg(name_nodes[1]) == qstr::from_str("bytecode") {
        *emit_options = EMIT_OPT_BYTECODE;
    } else if mpconfig::ENABLE_NATIVE_CODE
        && parse::parse_node_leaf_arg(name_nodes[1]) == qstr::from_str("native")
    {
        *emit_options = EMIT_OPT_NATIVE_PYTHON;
    } else if mpconfig::ENABLE_NATIVE_CODE
        && parse::parse_node_leaf_arg(name_nodes[1]) == qstr::from_str("viper")
    {
        *emit_options = EMIT_OPT_VIPER;
    } else {
        compile_syntax_error(comp, name_nodes[1], b"invalid micropython decorator");
    }
    true
}

fn compile_decorated(comp: &mut Compiler, pns: *mut ParseNodeStruct) {
    let mut nodes_ptr: *mut ParseNode = core::ptr::null_mut();
    let n = parse::parse_node_extract_list(
        &mut parse::parse_node_struct_node(pns, 0),
        Rule::Decorators,
        &mut nodes_ptr,
    );
    let mut emit_options = unsafe { (*comp.scope_cur.unwrap()).emit_options };
    let mut num_built_in_decorators = 0usize;
    for i in 0..n {
        let pns_decorator = unsafe { *nodes_ptr.add(i) } as *mut ParseNodeStruct;
        let mut name_nodes_ptr: *mut ParseNode = core::ptr::null_mut();
        let name_len = parse::parse_node_extract_list(
            &mut parse::parse_node_struct_node(pns_decorator, 0),
            Rule::DottedName,
            &mut name_nodes_ptr,
        );
        let name_nodes = unsafe { std::slice::from_raw_parts(name_nodes_ptr, name_len) };
        if compile_built_in_decorator(comp, name_len, name_nodes, &mut emit_options) {
            num_built_in_decorators += 1;
        } else {
            compile_node(comp, name_nodes[0]);
            for j in 1..name_len {
                EMIT_ARG!(
                    comp,
                    attr,
                    parse::parse_node_leaf_arg(name_nodes[j]),
                    emit::EMIT_ATTR_LOAD
                );
            }
            if !parse::parse_node_is_null(parse::parse_node_struct_node(pns_decorator, 1)) {
                compile_node(comp, parse::parse_node_struct_node(pns_decorator, 1));
            }
        }
    }
    let pns_body = parse::parse_node_struct_node(pns, 1) as *mut ParseNodeStruct;
    let body_name = if parse::parse_node_struct_kind(pns_body) == Rule::Funcdef as u32 {
        compile_funcdef_helper(comp, pns_body, emit_options)
    } else if mpconfig::PY_ASYNC_AWAIT
        && parse::parse_node_struct_kind(pns_body) == Rule::AsyncFuncdef as u32
    {
        let pns0 = parse::parse_node_struct_node(pns_body, 0) as *mut ParseNodeStruct;
        let name = compile_funcdef_helper(comp, pns0, emit_options);
        unsafe {
            let fscope = parse::parse_node_struct_node(pns0, 4) as *mut Scope;
            (*fscope).scope_flags |= bc0::SCOPE_FLAG_GENERATOR as u16;
        }
        name
    } else {
        compile_classdef_helper(comp, pns_body, emit_options)
    };
    for _ in 0..n - num_built_in_decorators {
        EMIT_ARG!(comp, call_function, 1, 0, 0);
    }
    compile_store_id(comp, body_name);
}

fn c_del_stmt(comp: &mut Compiler, pn: ParseNode) {
    if parse::parse_node_is_id(pn) {
        compile_delete_id(comp, parse::parse_node_leaf_arg(pn));
    } else if parse::parse_node_is_struct_kind(pn, Rule::AtomExprNormal) {
        let pns = pn as *mut ParseNodeStruct;
        compile_node(comp, parse::parse_node_struct_node(pns, 0));
        let pns1 = if parse::parse_node_is_struct(parse::parse_node_struct_node(pns, 1)) {
            let p = parse::parse_node_struct_node(pns, 1) as *mut ParseNodeStruct;
            if parse::parse_node_struct_kind(p) == Rule::AtomExprTrailers as u32 {
                let n = parse::parse_node_struct_num_nodes(p);
                for i in 0..n - 1 {
                    compile_node(comp, parse::parse_node_struct_node(p, i));
                }
                parse::parse_node_struct_node(p, n - 1) as *mut ParseNodeStruct
            } else {
                p
            }
        } else {
            compile_syntax_error(comp, pn, b"can't delete expression");
            return;
        };
        if parse::parse_node_struct_kind(pns1) == Rule::TrailerBracket as u32 {
            compile_node(comp, parse::parse_node_struct_node(pns1, 0));
            EMIT_ARG!(comp, subscr, emit::EMIT_SUBSCR_DELETE);
        } else if parse::parse_node_struct_kind(pns1) == Rule::TrailerPeriod as u32 {
            EMIT_ARG!(
                comp,
                attr,
                parse::parse_node_leaf_arg(parse::parse_node_struct_node(pns1, 0)),
                emit::EMIT_ATTR_DELETE
            );
        } else {
            compile_syntax_error(comp, pn, b"can't delete expression");
        }
    } else if parse::parse_node_is_struct_kind(pn, Rule::AtomParen) {
        let inner = parse::parse_node_struct_node(pn as *mut ParseNodeStruct, 0);
        if parse::parse_node_is_null(inner) {
            compile_syntax_error(comp, pn, b"can't delete expression");
        } else if parse::parse_node_is_struct_kind(inner, Rule::Testlist) {
            let pns = inner as *mut ParseNodeStruct;
            if parse_node_testlist_comp_has_comp_for(pns) {
                compile_syntax_error(comp, pn, b"can't delete expression");
            } else {
                let n = parse::parse_node_struct_num_nodes(pns);
                for i in 0..n {
                    c_del_stmt(comp, parse::parse_node_struct_node(pns, i));
                }
            }
        } else {
            compile_syntax_error(comp, pn, b"can't delete expression");
        }
    } else {
        compile_syntax_error(comp, pn, b"can't delete expression");
    }
}

fn compile_del_stmt(comp: &mut Compiler, pns: *mut ParseNodeStruct) {
    apply_to_single_or_list(
        comp,
        parse::parse_node_struct_node(pns, 0),
        Rule::Exprlist,
        c_del_stmt,
    );
}

fn compile_break_cont_stmt(comp: &mut Compiler, pns: *mut ParseNodeStruct) {
    let label = if parse::parse_node_struct_kind(pns) == Rule::BreakStmt as u32 {
        comp.break_label
    } else {
        comp.continue_label
    };
    if label == INVALID_LABEL {
        compile_syntax_error(comp, pns as ParseNode, b"'break'/'continue' outside loop");
        return;
    }
    EMIT_ARG!(
        comp,
        unwind_jump,
        label as usize,
        (comp.cur_except_level - comp.break_continue_except_level) as usize
    );
}

fn compile_return_stmt(comp: &mut Compiler, pns: *mut ParseNodeStruct) {
    if mpconfig::CPYTHON_COMPAT && unsafe { (*comp.scope_cur.unwrap()).kind } != ScopeKind::Function
    {
        compile_syntax_error(comp, pns as ParseNode, b"'return' outside function");
        return;
    }
    if parse::parse_node_is_null(parse::parse_node_struct_node(pns, 0)) {
        EMIT_ARG!(comp, load_const_tok, TokenKind::KwNone);
    } else if mpconfig::COMP_RETURN_IF_EXPR
        && parse::parse_node_is_struct_kind(parse::parse_node_struct_node(pns, 0), Rule::TestIfExpr)
    {
        let pns_test_if_expr = parse::parse_node_struct_node(pns, 0) as *mut ParseNodeStruct;
        let pns_test_if_else =
            parse::parse_node_struct_node(pns_test_if_expr, 1) as *mut ParseNodeStruct;
        let l_fail = comp_next_label(comp);
        c_if_cond(
            comp,
            parse::parse_node_struct_node(pns_test_if_else, 0),
            false,
            l_fail,
        );
        compile_node(comp, parse::parse_node_struct_node(pns_test_if_expr, 0));
        EMIT!(comp, return_value);
        EMIT_ARG!(comp, label_assign, l_fail);
        compile_node(comp, parse::parse_node_struct_node(pns_test_if_else, 1));
    } else {
        compile_node(comp, parse::parse_node_struct_node(pns, 0));
    }
    EMIT!(comp, return_value);
}

fn compile_yield_expr(comp: &mut Compiler, pns: *mut ParseNodeStruct) {
    let scope_kind = unsafe { (*comp.scope_cur.unwrap()).kind };
    if scope_kind != ScopeKind::Function && scope_kind != ScopeKind::Lambda {
        compile_syntax_error(comp, pns as ParseNode, b"'yield' outside function");
        return;
    }
    if parse::parse_node_is_null(parse::parse_node_struct_node(pns, 0)) {
        EMIT_ARG!(comp, load_const_tok, TokenKind::KwNone);
        EMIT_ARG!(comp, yield_, emit::EMIT_YIELD_VALUE);
        reserve_labels_for_native(comp, 2);
    } else if parse::parse_node_is_struct_kind(
        parse::parse_node_struct_node(pns, 0),
        Rule::YieldArgFrom,
    ) {
        let pns_from = parse::parse_node_struct_node(pns, 0) as *mut ParseNodeStruct;
        compile_node(comp, parse::parse_node_struct_node(pns_from, 0));
        compile_yield_from(comp);
    } else {
        compile_node(comp, parse::parse_node_struct_node(pns, 0));
        EMIT_ARG!(comp, yield_, emit::EMIT_YIELD_VALUE);
        reserve_labels_for_native(comp, 2);
    }
}

fn compile_raise_stmt(comp: &mut Compiler, pns: *mut ParseNodeStruct) {
    if parse::parse_node_is_null(parse::parse_node_struct_node(pns, 0)) {
        EMIT_ARG!(comp, raise_varargs, 0);
    } else if parse::parse_node_is_struct_kind(
        parse::parse_node_struct_node(pns, 0),
        Rule::RaiseStmtArg,
    ) {
        let pns_arg = parse::parse_node_struct_node(pns, 0) as *mut ParseNodeStruct;
        compile_node(comp, parse::parse_node_struct_node(pns_arg, 0));
        compile_node(comp, parse::parse_node_struct_node(pns_arg, 1));
        EMIT_ARG!(comp, raise_varargs, 2);
    } else {
        compile_node(comp, parse::parse_node_struct_node(pns, 0));
        EMIT_ARG!(comp, raise_varargs, 1);
    }
}

fn do_import_name(comp: &mut Compiler, pn: ParseNode, q_base: &mut Qstr) {
    let mut pn = pn;
    let mut is_as = false;
    if parse::parse_node_is_struct_kind(pn, Rule::DottedAsName) {
        let pns = pn as *mut ParseNodeStruct;
        *q_base = parse::parse_node_leaf_arg(parse::parse_node_struct_node(pns, 1));
        pn = parse::parse_node_struct_node(pns, 0);
        is_as = true;
    }
    if parse::parse_node_is_null(pn) {
        *q_base = qstr::from_str("");
        EMIT_ARG!(comp, import, qstr::from_str(""), emit::EMIT_IMPORT_NAME);
    } else if parse::parse_node_is_id(pn) {
        let q_full = parse::parse_node_leaf_arg(pn);
        if !is_as {
            *q_base = q_full;
        }
        EMIT_ARG!(comp, import, q_full, emit::EMIT_IMPORT_NAME);
    } else {
        let pns = pn as *mut ParseNodeStruct;
        if !is_as {
            *q_base = parse::parse_node_leaf_arg(parse::parse_node_struct_node(pns, 0));
        }
        let n = parse::parse_node_struct_num_nodes(pns);
        let mut buf = Vec::new();
        for i in 0..n {
            if i > 0 {
                buf.push(b'.');
            }
            if let Some((data, _)) = qstr::qstr_data(parse::parse_node_leaf_arg(
                parse::parse_node_struct_node(pns, i),
            )) {
                buf.extend_from_slice(&data);
            }
        }
        let q_full = qstr::from_strn(&buf);
        EMIT_ARG!(comp, import, q_full, emit::EMIT_IMPORT_NAME);
        if is_as {
            for i in 1..n {
                EMIT_ARG!(
                    comp,
                    attr,
                    parse::parse_node_leaf_arg(parse::parse_node_struct_node(pns, i)),
                    emit::EMIT_ATTR_LOAD
                );
            }
        }
    }
}

fn compile_dotted_as_name(comp: &mut Compiler, pn: ParseNode) {
    EMIT_ARG!(comp, load_const_small_int, 0);
    EMIT_ARG!(comp, load_const_tok, TokenKind::KwNone);
    let mut q_base = qstr::QSTR_NULL;
    do_import_name(comp, pn, &mut q_base);
    compile_store_id(comp, q_base);
}

fn compile_import_name(comp: &mut Compiler, pns: *mut ParseNodeStruct) {
    apply_to_single_or_list(
        comp,
        parse::parse_node_struct_node(pns, 0),
        Rule::DottedAsNames,
        compile_dotted_as_name,
    );
}

fn compile_import_from(comp: &mut Compiler, pns: *mut ParseNodeStruct) {
    let mut pn_import_source = parse::parse_node_struct_node(pns, 0);
    let mut import_level = 0u32;
    loop {
        let (mut pn_rel, done) = if parse::parse_node_is_token(pn_import_source)
            || parse::parse_node_is_struct_kind(pn_import_source, Rule::OneOrMorePeriodOrEllipsis)
        {
            (pn_import_source, true)
        } else if parse::parse_node_is_struct_kind(pn_import_source, Rule::ImportFrom2b) {
            let pns_2b = pn_import_source as *mut ParseNodeStruct;
            pn_import_source = parse::parse_node_struct_node(pns_2b, 1);
            (parse::parse_node_struct_node(pns_2b, 0), false)
        } else {
            break;
        };
        let mut nodes_ptr: *mut ParseNode = core::ptr::null_mut();
        let n = parse::parse_node_extract_list(
            &mut pn_rel,
            Rule::OneOrMorePeriodOrEllipsis,
            &mut nodes_ptr,
        );
        for i in 0..n {
            let node = unsafe { *nodes_ptr.add(i) };
            if parse::parse_node_is_token_kind(node, TokenKind::DelPeriod) {
                import_level += 1;
            } else {
                import_level += 3;
            }
        }
        if done {
            pn_import_source = parse::PARSE_NODE_NULL;
            break;
        }
    }
    if parse::parse_node_is_token_kind(parse::parse_node_struct_node(pns, 1), TokenKind::OpStar) {
        EMIT_ARG!(comp, load_const_small_int, import_level as i64);
        EMIT_ARG!(comp, load_const_str, qstr::from_str("*"));
        EMIT_ARG!(comp, build, 1, emit::EMIT_BUILD_TUPLE);
        let mut dummy_q = qstr::QSTR_NULL;
        do_import_name(comp, pn_import_source, &mut dummy_q);
        EMIT_ARG!(comp, import, qstr::QSTR_NULL, emit::EMIT_IMPORT_STAR);
    } else {
        EMIT_ARG!(comp, load_const_small_int, import_level as i64);
        let mut pn_nodes: *mut ParseNode = core::ptr::null_mut();
        let n = parse::parse_node_extract_list(
            &mut parse::parse_node_struct_node(pns, 1),
            Rule::ImportAsNames,
            &mut pn_nodes,
        );
        for i in 0..n {
            let pns3 = unsafe { *pn_nodes.add(i) } as *mut ParseNodeStruct;
            let id2 = parse::parse_node_leaf_arg(parse::parse_node_struct_node(pns3, 0));
            EMIT_ARG!(comp, load_const_str, id2);
        }
        EMIT_ARG!(comp, build, n, emit::EMIT_BUILD_TUPLE);
        let mut dummy_q = qstr::QSTR_NULL;
        do_import_name(comp, pn_import_source, &mut dummy_q);
        for i in 0..n {
            let pns3 = unsafe { *pn_nodes.add(i) } as *mut ParseNodeStruct;
            let id2 = parse::parse_node_leaf_arg(parse::parse_node_struct_node(pns3, 0));
            EMIT_ARG!(comp, import, id2, emit::EMIT_IMPORT_FROM);
            if parse::parse_node_is_null(parse::parse_node_struct_node(pns3, 1)) {
                compile_store_id(comp, id2);
            } else {
                compile_store_id(
                    comp,
                    parse::parse_node_leaf_arg(parse::parse_node_struct_node(pns3, 1)),
                );
            }
        }
        EMIT!(comp, pop_top);
    }
}

fn compile_declare_global(comp: &mut Compiler, pn: ParseNode, id_info: &mut IdInfo) {
    if id_info.kind != IdInfoKind::Undecided && id_info.kind != IdInfoKind::GlobalExplicit {
        compile_syntax_error(comp, pn, b"identifier redefined as global");
        return;
    }
    id_info.kind = IdInfoKind::GlobalExplicit;
    unsafe {
        let mut s = comp.scope_cur.unwrap();
        while (*s).parent.is_some() {
            s = (*s).parent.unwrap();
        }
        let id = scope::find_or_add_id(unsafe { &mut *s }, id_info.qst, IdInfoKind::Undecided);
        id.kind = IdInfoKind::GlobalExplicit;
    }
}

fn compile_declare_nonlocal(comp: &mut Compiler, pn: ParseNode, id_info: &mut IdInfo) {
    if id_info.kind == IdInfoKind::Undecided {
        id_info.kind = IdInfoKind::GlobalImplicit;
        scope::check_to_close_over(unsafe { &mut *comp.scope_cur.unwrap() }, id_info);
        if id_info.kind == IdInfoKind::GlobalImplicit {
            compile_syntax_error(comp, pn, b"no binding for nonlocal found");
        }
    } else if id_info.kind != IdInfoKind::Free {
        compile_syntax_error(comp, pn, b"identifier redefined as nonlocal");
    }
}

fn compile_global_nonlocal_stmt(comp: &mut Compiler, pns: *mut ParseNodeStruct) {
    if comp.pass == PassKind::Scope {
        let is_global = parse::parse_node_struct_kind(pns) == Rule::GlobalStmt as u32;
        if !is_global && unsafe { (*comp.scope_cur.unwrap()).kind } == ScopeKind::Module {
            compile_syntax_error(
                comp,
                pns as ParseNode,
                b"can't declare nonlocal in outer code",
            );
            return;
        }
        let mut nodes_ptr: *mut ParseNode = core::ptr::null_mut();
        let n = parse::parse_node_extract_list(
            &mut parse::parse_node_struct_node(pns, 0),
            Rule::NameList,
            &mut nodes_ptr,
        );
        for i in 0..n {
            let pn = unsafe { *nodes_ptr.add(i) };
            let qst = parse::parse_node_leaf_arg(pn);
            let id_info = scope::find_or_add_id(
                unsafe { &mut *comp.scope_cur.unwrap() },
                qst,
                IdInfoKind::Undecided,
            );
            if is_global {
                compile_declare_global(comp, pn, id_info);
            } else {
                compile_declare_nonlocal(comp, pn, id_info);
            }
        }
    }
}

fn compile_assert_stmt(comp: &mut Compiler, pns: *mut ParseNodeStruct) {
    if mpstate::with_vm(|vm| vm.mp_optimise_value) != 0 {
        return;
    }
    let l_end = comp_next_label(comp);
    c_if_cond(comp, parse::parse_node_struct_node(pns, 0), true, l_end);
    EMIT_LOAD_GLOBAL!(comp, qstr::from_str("AssertionError"));
    if !parse::parse_node_is_null(parse::parse_node_struct_node(pns, 1)) {
        compile_node(comp, parse::parse_node_struct_node(pns, 1));
        EMIT_ARG!(comp, call_function, 1, 0, 0);
    }
    EMIT_ARG!(comp, raise_varargs, 1);
    EMIT_ARG!(comp, label_assign, l_end);
}

fn compile_if_stmt(comp: &mut Compiler, pns: *mut ParseNodeStruct) {
    let l_end = comp_next_label(comp);
    if !parse::parse_node_is_const_false(parse::parse_node_struct_node(pns, 0)) {
        let l_fail = comp_next_label(comp);
        c_if_cond(comp, parse::parse_node_struct_node(pns, 0), false, l_fail);
        compile_node(comp, parse::parse_node_struct_node(pns, 1));
        if !parse::parse_node_is_const_true(parse::parse_node_struct_node(pns, 0)) {
            if !(parse::parse_node_is_null(parse::parse_node_struct_node(pns, 2))
                && parse::parse_node_is_null(parse::parse_node_struct_node(pns, 3)))
            {
                EMIT_ARG!(comp, jump, l_end);
            }
            EMIT_ARG!(comp, label_assign, l_fail);
        } else {
            EMIT_ARG!(comp, label_assign, l_end);
            return;
        }
    }
    let mut pn_elif: *mut ParseNode = core::ptr::null_mut();
    let n_elif = parse::parse_node_extract_list(
        &mut parse::parse_node_struct_node(pns, 2),
        Rule::IfStmtElifList,
        &mut pn_elif,
    );
    for i in 0..n_elif {
        let pns_elif = unsafe { *pn_elif.add(i) } as *mut ParseNodeStruct;
        if !parse::parse_node_is_const_false(parse::parse_node_struct_node(pns_elif, 0)) {
            let l_fail = comp_next_label(comp);
            c_if_cond(
                comp,
                parse::parse_node_struct_node(pns_elif, 0),
                false,
                l_fail,
            );
            compile_node(comp, parse::parse_node_struct_node(pns_elif, 1));
            if parse::parse_node_is_const_true(parse::parse_node_struct_node(pns_elif, 0)) {
                EMIT_ARG!(comp, label_assign, l_end);
                return;
            }
            EMIT_ARG!(comp, jump, l_end);
            EMIT_ARG!(comp, label_assign, l_fail);
        }
    }
    compile_node(comp, parse::parse_node_struct_node(pns, 3));
    EMIT_ARG!(comp, label_assign, l_end);
}

fn compile_while_stmt(comp: &mut Compiler, pns: *mut ParseNodeStruct) {
    let old_break = comp.break_label;
    let old_continue = comp.continue_label;
    let old_bc_level = comp.break_continue_except_level;
    let break_label = comp_next_label(comp) as u16;
    let continue_label = comp_next_label(comp) as u16;
    comp.break_label = break_label;
    comp.continue_label = continue_label;
    comp.break_continue_except_level = comp.cur_except_level;
    if !parse::parse_node_is_const_false(parse::parse_node_struct_node(pns, 0)) {
        let top_label = comp_next_label(comp);
        if !parse::parse_node_is_const_true(parse::parse_node_struct_node(pns, 0)) {
            EMIT_ARG!(comp, jump, continue_label as usize);
        }
        EMIT_ARG!(comp, label_assign, top_label);
        compile_node(comp, parse::parse_node_struct_node(pns, 1));
        EMIT_ARG!(comp, label_assign, continue_label as usize);
        c_if_cond(comp, parse::parse_node_struct_node(pns, 0), true, top_label);
    }
    comp.break_label = old_break;
    comp.continue_label = old_continue;
    comp.break_continue_except_level = old_bc_level;
    compile_node(comp, parse::parse_node_struct_node(pns, 2));
    EMIT_ARG!(comp, label_assign, break_label as usize);
}

fn compile_for_stmt_optimised_range(
    comp: &mut Compiler,
    pn_var: ParseNode,
    pn_start: ParseNode,
    pn_end: ParseNode,
    pn_step: ParseNode,
    pn_body: ParseNode,
    pn_else: ParseNode,
) {
    let old_break = comp.break_label;
    let old_continue = comp.continue_label;
    let old_bc_level = comp.break_continue_except_level;
    let break_label = comp_next_label(comp) as u16;
    let continue_label = comp_next_label(comp) as u16;
    comp.break_label = break_label;
    comp.continue_label = continue_label;
    comp.break_continue_except_level = comp.cur_except_level;
    let top_label = comp_next_label(comp);
    let entry_label = comp_next_label(comp);
    let end_on_stack = !parse::parse_node_is_small_int(pn_end);
    if end_on_stack {
        compile_node(comp, pn_end);
    }
    compile_node(comp, pn_start);
    EMIT_ARG!(comp, jump, entry_label);
    EMIT_ARG!(comp, label_assign, top_label);
    EMIT!(comp, dup_top);
    c_assign(comp, pn_var, AssignKind::Store);
    compile_node(comp, pn_body);
    EMIT_ARG!(comp, label_assign, continue_label as usize);
    compile_node(comp, pn_step);
    EMIT_ARG!(comp, binary_op, BinaryOp::InplaceAdd);
    EMIT_ARG!(comp, label_assign, entry_label);
    if end_on_stack {
        EMIT!(comp, dup_top_two);
        EMIT!(comp, rot_two);
    } else {
        EMIT!(comp, dup_top);
        compile_node(comp, pn_end);
    }
    if parse::parse_node_leaf_small_int(pn_step) >= 0 {
        EMIT_ARG!(comp, binary_op, BinaryOp::Less);
    } else {
        EMIT_ARG!(comp, binary_op, BinaryOp::More);
    }
    EMIT_ARG!(comp, pop_jump_if, true, top_label);
    comp.break_label = old_break;
    comp.continue_label = old_continue;
    comp.break_continue_except_level = old_bc_level;
    let mut end_label = 0usize;
    if !parse::parse_node_is_null(pn_else) {
        EMIT!(comp, pop_top);
        if end_on_stack {
            EMIT!(comp, pop_top);
        }
        compile_node(comp, pn_else);
        end_label = comp_next_label(comp);
        EMIT_ARG!(comp, jump, end_label);
        EMIT_ARG!(comp, adjust_stack_size, if end_on_stack { 2 } else { 1 });
    }
    EMIT_ARG!(comp, label_assign, break_label as usize);
    EMIT!(comp, pop_top);
    if end_on_stack {
        EMIT!(comp, pop_top);
    }
    if !parse::parse_node_is_null(pn_else) {
        EMIT_ARG!(comp, label_assign, end_label);
    }
}

fn compile_for_stmt(comp: &mut Compiler, pns: *mut ParseNodeStruct) {
    if parse::parse_node_is_id(parse::parse_node_struct_node(pns, 0))
        && parse::parse_node_is_struct_kind(
            parse::parse_node_struct_node(pns, 1),
            Rule::AtomExprNormal,
        )
    {
        let pns_it = parse::parse_node_struct_node(pns, 1) as *mut ParseNodeStruct;
        if parse::parse_node_is_id(parse::parse_node_struct_node(pns_it, 0))
            && parse::parse_node_leaf_arg(parse::parse_node_struct_node(pns_it, 0))
                == qstr::from_str("range")
            && parse::parse_node_is_struct_kind(
                parse::parse_node_struct_node(pns_it, 1),
                Rule::TrailerParen,
            )
        {
            let mut pn_range_args = parse::parse_node_struct_node(
                parse::parse_node_struct_node(pns_it, 1) as *mut ParseNodeStruct,
                0,
            );
            let mut args_ptr: *mut ParseNode = core::ptr::null_mut();
            let n_args =
                parse::parse_node_extract_list(&mut pn_range_args, Rule::Arglist, &mut args_ptr);
            let (pn_range_start, pn_range_end, pn_range_step, optimize) =
                if n_args >= 1 && n_args <= 3 {
                    let (start, end, step) = if n_args == 1 {
                        (
                            parse::parse_node_new_small_int(0),
                            unsafe { *args_ptr },
                            parse::parse_node_new_small_int(1),
                        )
                    } else if n_args == 2 {
                        (
                            unsafe { *args_ptr },
                            unsafe { *args_ptr.add(1) },
                            parse::parse_node_new_small_int(1),
                        )
                    } else {
                        (unsafe { *args_ptr }, unsafe { *args_ptr.add(1) }, unsafe {
                            *args_ptr.add(2)
                        })
                    };
                    let mut ok = true;
                    if !parse::parse_node_is_small_int(step)
                        || parse::parse_node_leaf_small_int(step) == 0
                    {
                        ok = false;
                    }
                    for pn in [start, end] {
                        if parse::parse_node_is_struct(pn) {
                            let k = parse::parse_node_struct_kind(pn as *mut ParseNodeStruct);
                            if k == Rule::ArglistStar as u32
                                || k == Rule::ArglistDblStar as u32
                                || k == Rule::Argument as u32
                            {
                                ok = false;
                            }
                        }
                    }
                    (start, end, step, ok)
                } else {
                    (
                        parse::PARSE_NODE_NULL,
                        parse::PARSE_NODE_NULL,
                        parse::PARSE_NODE_NULL,
                        false,
                    )
                };
            if optimize {
                compile_for_stmt_optimised_range(
                    comp,
                    parse::parse_node_struct_node(pns, 0),
                    pn_range_start,
                    pn_range_end,
                    pn_range_step,
                    parse::parse_node_struct_node(pns, 2),
                    parse::parse_node_struct_node(pns, 3),
                );
                return;
            }
        }
    }
    let old_break = comp.break_label;
    let old_continue = comp.continue_label;
    let old_bc_level = comp.break_continue_except_level;
    let break_label = comp_next_label(comp) as u16;
    let continue_label = comp_next_label(comp) as u16;
    comp.break_label = break_label;
    comp.continue_label = continue_label;
    comp.break_continue_except_level = comp.cur_except_level;
    comp.break_label |= emit::EMIT_BREAK_FROM_FOR;
    let pop_label = comp_next_label(comp);
    compile_node(comp, parse::parse_node_struct_node(pns, 1));
    EMIT_ARG!(comp, get_iter, true);
    EMIT_ARG!(comp, label_assign, continue_label as usize);
    EMIT_ARG!(comp, for_iter, pop_label);
    c_assign(
        comp,
        parse::parse_node_struct_node(pns, 0),
        AssignKind::Store,
    );
    compile_node(comp, parse::parse_node_struct_node(pns, 2));
    EMIT_ARG!(comp, jump, continue_label as usize);
    EMIT_ARG!(comp, label_assign, pop_label);
    EMIT!(comp, for_iter_end);
    comp.break_label = old_break;
    comp.continue_label = old_continue;
    comp.break_continue_except_level = old_bc_level;
    compile_node(comp, parse::parse_node_struct_node(pns, 3));
    EMIT_ARG!(comp, label_assign, break_label as usize);
}

fn compile_try_except(
    comp: &mut Compiler,
    pn_body: ParseNode,
    n_except: usize,
    pn_excepts: *mut ParseNode,
    pn_else: ParseNode,
) {
    let l1 = comp_next_label(comp);
    let success_label = comp_next_label(comp);
    compile_increase_except_level(comp, l1, emit::EMIT_SETUP_BLOCK_EXCEPT);
    compile_node(comp, pn_body);
    EMIT_ARG!(comp, pop_except_jump, success_label, false);
    EMIT_ARG!(comp, label_assign, l1);
    EMIT!(comp, start_except_handler);
    let l2 = comp_next_label(comp);
    for i in 0..n_except {
        let pns_except = unsafe { *pn_excepts.add(i) } as *mut ParseNodeStruct;
        let mut qstr_exception_local = qstr::QSTR_NULL;
        let end_finally_label = comp_next_label(comp);
        if parse::parse_node_is_null(parse::parse_node_struct_node(pns_except, 0)) {
            if i + 1 != n_except {
                compile_syntax_error(
                    comp,
                    unsafe { *pn_excepts.add(i) },
                    b"default 'except' must be last",
                );
                compile_decrease_except_level(comp);
                return;
            }
        } else {
            let mut pns_exception_expr = parse::parse_node_struct_node(pns_except, 0);
            if parse::parse_node_is_struct(pns_exception_expr) {
                let pns3 = pns_exception_expr as *mut ParseNodeStruct;
                if parse::parse_node_struct_kind(pns3) == Rule::TryStmtAsName as u32 {
                    pns_exception_expr = parse::parse_node_struct_node(pns3, 0);
                    qstr_exception_local =
                        parse::parse_node_leaf_arg(parse::parse_node_struct_node(pns3, 1));
                }
            }
            EMIT!(comp, dup_top);
            compile_node(comp, pns_exception_expr);
            EMIT_ARG!(comp, binary_op, BinaryOp::ExceptionMatch);
            EMIT_ARG!(comp, pop_jump_if, false, end_finally_label);
        }
        if qstr_exception_local == qstr::QSTR_NULL {
            EMIT!(comp, pop_top);
        } else {
            compile_store_id(comp, qstr_exception_local);
        }
        let mut l3 = 0usize;
        if qstr_exception_local != qstr::QSTR_NULL {
            l3 = comp_next_label(comp);
            compile_increase_except_level(comp, l3, emit::EMIT_SETUP_BLOCK_FINALLY);
        }
        compile_node(comp, parse::parse_node_struct_node(pns_except, 1));
        if qstr_exception_local != qstr::QSTR_NULL {
            EMIT_ARG!(comp, load_const_tok, TokenKind::KwNone);
            EMIT_ARG!(comp, label_assign, l3);
            EMIT_ARG!(comp, adjust_stack_size, 1);
            EMIT_ARG!(comp, load_const_tok, TokenKind::KwNone);
            compile_store_id(comp, qstr_exception_local);
            compile_delete_id(comp, qstr_exception_local);
            EMIT_ARG!(comp, adjust_stack_size, -1);
            compile_decrease_except_level(comp);
        }
        EMIT_ARG!(comp, pop_except_jump, l2, true);
        EMIT_ARG!(comp, label_assign, end_finally_label);
        EMIT_ARG!(comp, adjust_stack_size, 1);
    }
    compile_decrease_except_level(comp);
    EMIT!(comp, end_except_handler);
    EMIT_ARG!(comp, label_assign, success_label);
    compile_node(comp, pn_else);
    EMIT_ARG!(comp, label_assign, l2);
}

fn compile_try_finally(
    comp: &mut Compiler,
    pn_body: ParseNode,
    n_except: usize,
    pn_except: *mut ParseNode,
    pn_else: ParseNode,
    pn_finally: ParseNode,
) {
    let l_finally_block = comp_next_label(comp);
    compile_increase_except_level(comp, l_finally_block, emit::EMIT_SETUP_BLOCK_FINALLY);
    if n_except == 0 {
        EMIT_ARG!(comp, adjust_stack_size, 3);
        compile_node(comp, pn_body);
        EMIT_ARG!(comp, adjust_stack_size, -3);
    } else {
        compile_try_except(comp, pn_body, n_except, pn_except, pn_else);
    }
    EMIT_ARG!(comp, load_const_tok, TokenKind::KwNone);
    EMIT_ARG!(comp, label_assign, l_finally_block);
    EMIT_ARG!(comp, adjust_stack_size, 1);
    compile_node(comp, pn_finally);
    EMIT_ARG!(comp, adjust_stack_size, -1);
    compile_decrease_except_level(comp);
}

fn compile_try_stmt(comp: &mut Compiler, pns: *mut ParseNodeStruct) {
    let pns2 = parse::parse_node_struct_node(pns, 1) as *mut ParseNodeStruct;
    if parse::parse_node_struct_kind(pns2) == Rule::TryStmtFinally as u32 {
        compile_try_finally(
            comp,
            parse::parse_node_struct_node(pns, 0),
            0,
            core::ptr::null_mut(),
            parse::PARSE_NODE_NULL,
            parse::parse_node_struct_node(pns2, 0),
        );
    } else if parse::parse_node_struct_kind(pns2) == Rule::TryStmtExceptAndMore as u32 {
        let mut pn_excepts: *mut ParseNode = core::ptr::null_mut();
        let n_except = parse::parse_node_extract_list(
            &mut parse::parse_node_struct_node(pns2, 0),
            Rule::TryStmtExceptList,
            &mut pn_excepts,
        );
        if parse::parse_node_is_null(parse::parse_node_struct_node(pns2, 2)) {
            compile_try_except(
                comp,
                parse::parse_node_struct_node(pns, 0),
                n_except,
                pn_excepts,
                parse::parse_node_struct_node(pns2, 1),
            );
        } else {
            let pns_fin = parse::parse_node_struct_node(pns2, 2) as *mut ParseNodeStruct;
            compile_try_finally(
                comp,
                parse::parse_node_struct_node(pns, 0),
                n_except,
                pn_excepts,
                parse::parse_node_struct_node(pns2, 1),
                parse::parse_node_struct_node(pns_fin, 0),
            );
        }
    } else {
        let mut pn_excepts: *mut ParseNode = core::ptr::null_mut();
        let n_except = parse::parse_node_extract_list(
            &mut parse::parse_node_struct_node(pns, 1),
            Rule::TryStmtExceptList,
            &mut pn_excepts,
        );
        compile_try_except(
            comp,
            parse::parse_node_struct_node(pns, 0),
            n_except,
            pn_excepts,
            parse::PARSE_NODE_NULL,
        );
    }
}

fn compile_with_stmt_helper(comp: &mut Compiler, n: usize, nodes: *mut ParseNode, body: ParseNode) {
    if n == 0 {
        compile_node(comp, body);
    } else {
        let l_end = comp_next_label(comp);
        let node0 = unsafe { *nodes };
        if parse::parse_node_is_struct_kind(node0, Rule::WithItem) {
            let pns = node0 as *mut ParseNodeStruct;
            compile_node(comp, parse::parse_node_struct_node(pns, 0));
            compile_increase_except_level(comp, l_end, emit::EMIT_SETUP_BLOCK_WITH);
            c_assign(
                comp,
                parse::parse_node_struct_node(pns, 1),
                AssignKind::Store,
            );
        } else {
            compile_node(comp, node0);
            compile_increase_except_level(comp, l_end, emit::EMIT_SETUP_BLOCK_WITH);
            EMIT!(comp, pop_top);
        }
        compile_with_stmt_helper(comp, n - 1, unsafe { nodes.add(1) }, body);
        EMIT_ARG!(comp, with_cleanup, l_end);
        reserve_labels_for_native(comp, 3);
        compile_decrease_except_level(comp);
    }
}

fn compile_with_stmt(comp: &mut Compiler, pns: *mut ParseNodeStruct) {
    let mut nodes_ptr: *mut ParseNode = core::ptr::null_mut();
    let n = parse::parse_node_extract_list(
        &mut parse::parse_node_struct_node(pns, 0),
        Rule::WithStmtList,
        &mut nodes_ptr,
    );
    debug_assert!(n > 0);
    compile_with_stmt_helper(comp, n, nodes_ptr, parse::parse_node_struct_node(pns, 1));
}

fn compile_await_object_method(comp: &mut Compiler, method: Qstr) {
    EMIT_ARG!(comp, load_method, method, false);
    EMIT_ARG!(comp, call_method, 0, 0, 0);
    compile_yield_from(comp);
}

fn compile_async_for_stmt(comp: &mut Compiler, pns: *mut ParseNodeStruct) {
    let while_else_label = comp_next_label(comp);
    let try_exception_label = comp_next_label(comp);
    let try_else_label = comp_next_label(comp);
    let try_finally_label = comp_next_label(comp);

    compile_node(comp, parse::parse_node_struct_node(pns, 1));
    EMIT_ARG!(comp, load_method, qstr::from_str("__aiter__"), false);
    EMIT_ARG!(comp, call_method, 0, 0, 0);

    let old_break = comp.break_label;
    let old_continue = comp.continue_label;
    let old_bc_level = comp.break_continue_except_level;
    let break_label = comp_next_label(comp) as u16;
    let continue_label = comp_next_label(comp) as u16;
    comp.break_label = break_label;
    comp.continue_label = continue_label;
    comp.break_continue_except_level = comp.cur_except_level;

    EMIT_ARG!(comp, label_assign, continue_label as usize);
    compile_increase_except_level(comp, try_exception_label, emit::EMIT_SETUP_BLOCK_EXCEPT);
    EMIT!(comp, dup_top);
    compile_await_object_method(comp, qstr::from_str("__anext__"));
    c_assign(
        comp,
        parse::parse_node_struct_node(pns, 0),
        AssignKind::Store,
    );
    EMIT_ARG!(comp, pop_except_jump, try_else_label, false);

    EMIT_ARG!(comp, label_assign, try_exception_label);
    EMIT!(comp, start_except_handler);
    EMIT!(comp, dup_top);
    EMIT_LOAD_GLOBAL!(comp, qstr::from_str("StopAsyncIteration"));
    EMIT_ARG!(comp, binary_op, BinaryOp::ExceptionMatch);
    EMIT_ARG!(comp, pop_jump_if, false, try_finally_label);
    EMIT!(comp, pop_top);
    EMIT_ARG!(comp, pop_except_jump, while_else_label, true);

    EMIT_ARG!(comp, label_assign, try_finally_label);
    EMIT_ARG!(comp, adjust_stack_size, 1);
    compile_decrease_except_level(comp);
    EMIT!(comp, end_except_handler);

    EMIT_ARG!(comp, label_assign, try_else_label);
    compile_node(comp, parse::parse_node_struct_node(pns, 2));
    EMIT_ARG!(comp, jump, continue_label as usize);

    comp.break_label = old_break;
    comp.continue_label = old_continue;
    comp.break_continue_except_level = old_bc_level;

    EMIT_ARG!(comp, label_assign, while_else_label);
    compile_node(comp, parse::parse_node_struct_node(pns, 3));
    EMIT_ARG!(comp, label_assign, break_label as usize);
    EMIT!(comp, pop_top);
}

fn compile_async_with_stmt_helper(
    comp: &mut Compiler,
    n: usize,
    nodes: *mut ParseNode,
    body: ParseNode,
) {
    if n == 0 {
        compile_node(comp, body);
    } else {
        let l_finally_block = comp_next_label(comp);
        let l_aexit_no_exc = comp_next_label(comp);
        let l_ret_unwind_jump = comp_next_label(comp);
        let l_end = comp_next_label(comp);

        let node0 = unsafe { *nodes };
        if parse::parse_node_is_struct_kind(node0, Rule::WithItem) {
            let pns = node0 as *mut ParseNodeStruct;
            compile_node(comp, parse::parse_node_struct_node(pns, 0));
            EMIT!(comp, dup_top);
            compile_await_object_method(comp, qstr::from_str("__aenter__"));
            c_assign(
                comp,
                parse::parse_node_struct_node(pns, 1),
                AssignKind::Store,
            );
        } else {
            compile_node(comp, node0);
            EMIT!(comp, dup_top);
            compile_await_object_method(comp, qstr::from_str("__aenter__"));
            EMIT!(comp, pop_top);
        }

        compile_increase_except_level(comp, l_finally_block, emit::EMIT_SETUP_BLOCK_FINALLY);
        EMIT_ARG!(comp, adjust_stack_size, 3);
        compile_async_with_stmt_helper(comp, n - 1, unsafe { nodes.add(1) }, body);
        EMIT_ARG!(comp, adjust_stack_size, -3);

        EMIT_ARG!(
            comp,
            async_with_setup_finally,
            l_aexit_no_exc,
            l_finally_block,
            l_ret_unwind_jump,
        );

        EMIT!(comp, dup_top);
        EMIT!(comp, rot_three);
        EMIT!(comp, rot_two);
        EMIT_ARG!(comp, load_method, qstr::from_str("__aexit__"), false);
        EMIT!(comp, rot_three);
        EMIT!(comp, rot_three);
        EMIT!(comp, dup_top);
        if mpconfig::CPYTHON_COMPAT {
            EMIT_ARG!(
                comp,
                attr,
                qstr::from_str("__class__"),
                emit::EMIT_ATTR_LOAD
            );
        } else {
            compile_load_id(comp, qstr::from_str("type"));
            EMIT!(comp, rot_two);
            EMIT_ARG!(comp, call_function, 1, 0, 0);
        }
        EMIT!(comp, rot_two);
        EMIT_ARG!(comp, load_const_tok, TokenKind::KwNone);
        EMIT_ARG!(comp, call_method, 3, 0, 0);
        compile_yield_from(comp);
        EMIT_ARG!(comp, pop_jump_if, false, l_end);
        EMIT!(comp, pop_top);
        EMIT_ARG!(comp, load_const_tok, TokenKind::KwNone);
        EMIT_ARG!(comp, jump, l_end);
        EMIT_ARG!(comp, adjust_stack_size, 2);

        EMIT_ARG!(comp, label_assign, l_ret_unwind_jump);
        if comp.use_native_emit {
            emitnx64::emit_native_x64_async_with_ret_unwind_enter(comp.emit);
        }
        EMIT!(comp, rot_three);
        EMIT!(comp, rot_three);
        EMIT_ARG!(comp, label_assign, l_aexit_no_exc);
        EMIT_ARG!(comp, load_method, qstr::from_str("__aexit__"), false);
        EMIT_ARG!(comp, load_const_tok, TokenKind::KwNone);
        EMIT!(comp, dup_top);
        EMIT!(comp, dup_top);
        EMIT_ARG!(comp, call_method, 3, 0, 0);
        compile_yield_from(comp);
        EMIT!(comp, pop_top);
        EMIT_ARG!(comp, adjust_stack_size, -1);

        EMIT_ARG!(comp, label_assign, l_end);
        compile_decrease_except_level(comp);
    }
}

fn compile_async_with_stmt(comp: &mut Compiler, pns: *mut ParseNodeStruct) {
    let mut nodes_ptr: *mut ParseNode = core::ptr::null_mut();
    let n = parse::parse_node_extract_list(
        &mut parse::parse_node_struct_node(pns, 0),
        Rule::WithStmtList,
        &mut nodes_ptr,
    );
    debug_assert!(n > 0);
    compile_async_with_stmt_helper(comp, n, nodes_ptr, parse::parse_node_struct_node(pns, 1));
}

fn compile_async_stmt(comp: &mut Compiler, pns: *mut ParseNodeStruct) {
    if !mpconfig::PY_ASYNC_AWAIT {
        compile_generic_all_nodes(comp, pns);
        return;
    }
    let pns0 = parse::parse_node_struct_node(pns, 0) as *mut ParseNodeStruct;
    if parse::parse_node_struct_kind(pns0) == Rule::Funcdef as u32 {
        compile_funcdef(comp, pns0);
        unsafe {
            let fscope = parse::parse_node_struct_node(pns0, 4) as *mut Scope;
            (*fscope).scope_flags |= bc0::SCOPE_FLAG_GENERATOR as u16;
        }
    } else {
        let scope_flags = unsafe { (*comp.scope_cur.unwrap()).scope_flags };
        if scope_flags & bc0::SCOPE_FLAG_GENERATOR as u16 == 0 {
            compile_syntax_error(
                comp,
                pns as ParseNode,
                b"async for/with outside async function",
            );
            return;
        }
        if parse::parse_node_struct_kind(pns0) == Rule::ForStmt as u32 {
            compile_async_for_stmt(comp, pns0);
        } else {
            debug_assert!(parse::parse_node_struct_kind(pns0) == Rule::WithStmt as u32);
            compile_async_with_stmt(comp, pns0);
        }
    }
}

fn compile_expr_stmt(comp: &mut Compiler, pns: *mut ParseNodeStruct) {
    let pn_rhs = parse::parse_node_struct_node(pns, 1);
    if parse::parse_node_is_null(pn_rhs) {
        if comp.is_repl && unsafe { (*comp.scope_cur.unwrap()).kind } == ScopeKind::Module {
            compile_load_id(comp, qstr::from_str("__repl_print__"));
            compile_node(comp, parse::parse_node_struct_node(pns, 0));
            EMIT_ARG!(comp, call_function, 1, 0, 0);
            EMIT!(comp, pop_top);
        } else {
            let pn0 = parse::parse_node_struct_node(pns, 0);
            if (parse::parse_node_is_leaf(pn0) && !parse::parse_node_is_id(pn0))
                || parse::parse_node_is_struct_kind(pn0, Rule::ConstObject)
            {
                // lonely constant
            } else {
                compile_node(comp, pn0);
                EMIT!(comp, pop_top);
            }
        }
    } else if parse::parse_node_is_struct(pn_rhs) {
        let pns1 = pn_rhs as *mut ParseNodeStruct;
        let kind = parse::parse_node_struct_kind(pns1);
        if kind == Rule::Annassign as u32 {
            if parse::parse_node_is_null(parse::parse_node_struct_node(pns1, 1)) {
                if unsafe { (*comp.scope_cur.unwrap()).kind } == ScopeKind::Function {
                    if parse::parse_node_is_id(parse::parse_node_struct_node(pns, 0)) {
                        let lhs = parse::parse_node_leaf_arg(parse::parse_node_struct_node(pns, 0));
                        scope::find_or_add_id(
                            unsafe { &mut *comp.scope_cur.unwrap() },
                            lhs,
                            IdInfoKind::Local,
                        );
                    }
                }
            } else {
                compile_node(comp, parse::parse_node_struct_node(pns1, 1));
                c_assign(
                    comp,
                    parse::parse_node_struct_node(pns, 0),
                    AssignKind::Store,
                );
            }
        } else if kind == Rule::ExprStmtAugassign as u32 {
            c_assign(
                comp,
                parse::parse_node_struct_node(pns, 0),
                AssignKind::AugLoad,
            );
            compile_node(comp, parse::parse_node_struct_node(pns1, 1));
            let tok = parse::parse_node_leaf_arg(parse::parse_node_struct_node(pns1, 0));
            let op = unsafe {
                core::mem::transmute::<u8, BinaryOp>(
                    (tok - TokenKind::DelPipeEqual as usize) as u8 + BinaryOp::InplaceOr as u8,
                )
            };
            EMIT_ARG!(comp, binary_op, op);
            c_assign(
                comp,
                parse::parse_node_struct_node(pns, 0),
                AssignKind::AugStore,
            );
        } else if kind == Rule::ExprStmtAssignList as u32 {
            let rhs = parse::parse_node_struct_num_nodes(pns1) - 1;
            compile_node(comp, parse::parse_node_struct_node(pns1, rhs));
            if rhs > 0 {
                EMIT!(comp, dup_top);
            }
            c_assign(
                comp,
                parse::parse_node_struct_node(pns, 0),
                AssignKind::Store,
            );
            for i in 0..rhs {
                if i + 1 < rhs {
                    EMIT!(comp, dup_top);
                }
                c_assign(
                    comp,
                    parse::parse_node_struct_node(pns1, i),
                    AssignKind::Store,
                );
            }
        } else {
            compile_node(comp, pn_rhs);
            c_assign(
                comp,
                parse::parse_node_struct_node(pns, 0),
                AssignKind::Store,
            );
        }
    } else {
        compile_node(comp, pn_rhs);
        c_assign(
            comp,
            parse::parse_node_struct_node(pns, 0),
            AssignKind::Store,
        );
    }
}

fn compile_test_if_expr(comp: &mut Compiler, pns: *mut ParseNodeStruct) {
    let pns_test_if_else = parse::parse_node_struct_node(pns, 1) as *mut ParseNodeStruct;
    let l_fail = comp_next_label(comp);
    let l_end = comp_next_label(comp);
    c_if_cond(
        comp,
        parse::parse_node_struct_node(pns_test_if_else, 0),
        false,
        l_fail,
    );
    compile_node(comp, parse::parse_node_struct_node(pns, 0));
    EMIT_ARG!(comp, jump, l_end);
    EMIT_ARG!(comp, label_assign, l_fail);
    EMIT_ARG!(comp, adjust_stack_size, -1);
    compile_node(comp, parse::parse_node_struct_node(pns_test_if_else, 1));
    EMIT_ARG!(comp, label_assign, l_end);
}

fn compile_lambdef(comp: &mut Compiler, pns: *mut ParseNodeStruct) {
    if comp.pass == PassKind::Scope {
        let s = scope_new_and_link(comp, ScopeKind::Lambda, pns as ParseNode, unsafe {
            (*comp.scope_cur.unwrap()).emit_options
        });
        parse::parse_node_struct_set_node(pns, 2, s as ParseNode);
    }
    let this_scope = unsafe { parse::parse_node_struct_node(pns, 2) as *mut Scope };
    compile_funcdef_lambdef(
        comp,
        this_scope,
        parse::parse_node_struct_node(pns, 0),
        Rule::Varargslist,
    );
}

fn compile_namedexpr_helper(comp: &mut Compiler, pn_name: ParseNode, pn_expr: ParseNode) {
    if !parse::parse_node_is_id(pn_name) {
        compile_syntax_error(comp, pn_name, b"can't use named expr with non-name");
        return;
    }
    let target = parse::parse_node_leaf_arg(pn_name);
    compile_node(comp, pn_expr);
    EMIT!(comp, dup_top);
    compile_store_id(comp, target);
}

fn compile_namedexpr(comp: &mut Compiler, pns: *mut ParseNodeStruct) {
    compile_namedexpr_helper(
        comp,
        parse::parse_node_struct_node(pns, 0),
        parse::parse_node_struct_node(pns, 1),
    );
}

fn compile_yield_stmt(comp: &mut Compiler, pns: *mut ParseNodeStruct) {
    compile_node(comp, parse::parse_node_struct_node(pns, 0));
    EMIT!(comp, pop_top);
}

pub fn compile_to_raw_code(
    parse_tree: &mut parse::ParseTree,
    source_file: Qstr,
    is_repl: bool,
    cm: &mut CompiledModule,
) {
    let mut comp = Compiler {
        is_repl,
        pass: PassKind::Scope,
        have_star: false,
        compile_error: obj::OBJ_NULL,
        compile_error_line: 0,
        next_label: 0,
        num_dict_params: 0,
        num_default_params: 0,
        break_label: INVALID_LABEL,
        continue_label: INVALID_LABEL,
        cur_except_level: 0,
        break_continue_except_level: 0,
        scope_head: None,
        scope_cur: None,
        emit: core::ptr::null_mut(),
        emit_native: core::ptr::null_mut(),
        use_native_emit: false,
        emit_common: EmitCommon {
            pass: PassKind::Scope,
            ct_cur_child: 0,
            children: core::ptr::null_mut(),
            qstr_map: Map::default(),
            const_obj_list: Vec::new(),
        },
    };
    emit_common_init(&mut comp.emit_common, source_file);
    let module_emit_opt = mpstate::with_vm(|vm| vm.default_emit_opt as u16);
    let module_scope = scope_new_and_link(
        &mut comp,
        ScopeKind::Module,
        parse_tree.root,
        module_emit_opt,
    );
    let emit_bc = emitbc::new(&mut comp.emit_common as *mut _);
    comp.emit = emit_bc;
    let mut max_num_labels = 0usize;
    let mut s = comp.scope_head;
    while let Some(scope) = s {
        if !comp_has_error(&comp) {
            compile_scope(&mut comp, scope, PassKind::Scope);
            unsafe {
                for id in &mut (*scope).id_info {
                    if id.kind == IdInfoKind::GlobalImplicit {
                        scope::check_to_close_over(&mut *scope, id);
                    }
                }
            }
        }
        if comp.next_label > max_num_labels {
            max_num_labels = comp.next_label;
        }
        s = unsafe { (*scope).next };
    }
    s = comp.scope_head;
    while let Some(scope) = s {
        if !comp_has_error(&comp) {
            scope_compute_things(scope);
        }
        s = unsafe { (*scope).next };
    }
    emitbc::set_max_num_labels(emit_bc, max_num_labels);
    s = comp.scope_head;
    while let Some(scope) = s {
        if !comp_has_error(&comp) {
            let emit_options = unsafe { (*scope).emit_options };
            comp.use_native_emit = mpconfig::ENABLE_NATIVE_CODE
                && asmbase::machine_code_dispatch_supported()
                && (emit_options == EMIT_OPT_NATIVE_PYTHON || emit_options == EMIT_OPT_VIPER);
            if comp.use_native_emit {
                if comp.emit_native.is_null() {
                    comp.emit_native = emitnx64::emit_native_x64_new(
                        &mut comp.emit_common as *mut _,
                        &mut comp.compile_error,
                        &mut comp.next_label,
                        max_num_labels,
                    );
                    if comp.emit_native.is_null() {
                        comp.compile_error = objexcept::new_exception_args(
                            objexcept::type_not_implemented_error(),
                            1,
                            &[objstr::new_str(
                                b"cannot emit native code for this architecture",
                            )],
                        );
                    }
                }
                comp.emit = comp.emit_native;
            } else {
                comp.emit = emit_bc;
            }
            if comp.compile_error == obj::OBJ_NULL {
                compile_scope(&mut comp, scope, PassKind::StackSize);
            }
            if !comp_has_error(&comp) {
                compile_scope(&mut comp, scope, PassKind::CodeSize);
            }
            if !comp_has_error(&comp) {
                while !compile_scope(&mut comp, scope, PassKind::Emit) {}
            }
        }
        s = unsafe { (*scope).next };
    }
    if !comp.emit_native.is_null() {
        cm.has_native = true;
    }
    if comp.compile_error != obj::OBJ_NULL {
        let (err_pn, err_name) = unsafe {
            let sc = comp.scope_cur.unwrap();
            ((*sc).pn, (*sc).simple_name)
        };
        compile_error_set_line(&mut comp, err_pn);
        objexcept::exception_add_traceback(
            comp.compile_error,
            source_file,
            comp.compile_error_line,
            err_name,
        );
    }
    cm.rc = unsafe { (*module_scope).raw_code };
    if comp.compile_error == obj::OBJ_NULL {
        cm.n_qstr = comp.emit_common.qstr_map.used;
        cm.n_obj = comp.emit_common.const_obj_list.len();
        emit_common_populate_module_context(&mut comp.emit_common, source_file, cm.context);
    }
    emitbc::free(emit_bc);
    if !comp.emit_native.is_null() {
        emitnx64::emit_native_x64_free(comp.emit_native);
    }
    parse::parse_tree_clear(parse_tree);
    let mut s = Some(module_scope);
    while let Some(scope) = s {
        s = unsafe { (*scope).next };
        scope::free(scope);
    }
    if comp.compile_error != obj::OBJ_NULL {
        raise::raise_obj(comp.compile_error);
    }
}

pub fn compile(parse_tree: &mut parse::ParseTree, source_file: Qstr, is_repl: bool) -> Obj {
    let ctx = malloc::new_obj::<ModuleContext>().expect("module context");
    unsafe {
        (*ctx).module.globals = objdict::dict_ptr(mpstate::globals_get());
    }
    let mut cm = CompiledModule {
        context: ctx,
        rc: core::ptr::null(),
        has_native: false,
        n_qstr: 0,
        n_obj: 0,
        arch_flags: 0,
    };
    compile_to_raw_code(parse_tree, source_file, is_repl, &mut cm);
    emitglue::make_function_from_proto_fun(cm.rc as *const _, ctx, None)
}

/// `mp_parse_compile_execute`
pub fn parse_compile_execute(
    lex: crate::lexer::Lexer,
    parse_input_kind: parse::ParseInputKind,
    globals: Option<Obj>,
    locals: Option<Obj>,
) -> Obj {
    let old_globals = mpstate::globals_get();
    let old_locals = mpstate::locals_get();
    let globals = globals.unwrap_or(old_globals);
    let locals = locals.unwrap_or(old_locals);
    mpstate::globals_set(globals);
    mpstate::locals_set(locals);

    nlr::push_jump_callback(move || {
        crate::runtime::globals_locals_set_from_nlr_jump_callback(old_globals, old_locals);
    });

    let source_name = lex.source_name;
    let is_repl = parse_input_kind == parse::ParseInputKind::SingleInput;
    let mut parse_tree = parse::parse(lex, parse_input_kind);
    let module_fun = compile(&mut parse_tree, source_name, is_repl);

    let ret = if mpconfig::PY_BUILTINS_COMPILE
        && mpconfig::PY_BUILTINS_CODE == mpconfig::PY_BUILTINS_CODE_MINIMUM
        && globals == obj::OBJ_NULL
    {
        module_fun
    } else if mpstate::skip_compiled_execution() {
        module_fun
    } else {
        crate::runtime::call_function_0(module_fun)
    };

    nlr::pop_jump_callback(true);
    ret
}

#[cfg(test)]
mod compile_tests {
    use super::*;
    use crate::asmbase;
    use crate::lexer;
    use crate::mpstate;
    use crate::obj;
    use crate::objdict;
    use crate::objexcept;
    use crate::objfun;
    use crate::objstr;
    use crate::objtuple;
    use crate::parse::{self, ParseInputKind};
    use crate::qstr;
    use crate::reader::READER_IS_ROM;
    use crate::runtime;

    fn setup() {
        runtime::init();
        let _ = crate::modbuiltins::init_builtins_module();
    }

    #[test]
    fn compile_native_decorator_return_const_int() {
        setup();
        if !asmbase::machine_code_dispatch_supported() {
            return;
        }
        let src = "@micropython.native\ndef f():\n    return 42\n";
        let lex = lexer::Lexer::new_from_str_len(
            qstr::from_str("<stdin>"),
            src.as_bytes(),
            READER_IS_ROM,
        );
        let mut tree = parse::parse(lex, ParseInputKind::FileInput);
        let module_fun = compile(&mut tree, qstr::from_str("<stdin>"), false);
        runtime::call_function_n_kw(module_fun, 0, 0, &[]);
        let f_qstr = qstr::from_str("f");
        let f_obj = objdict::dict_get(mpstate::globals_get(), obj::new_qstr(f_qstr));
        assert_ne!(f_obj, obj::OBJ_NULL);
        assert!(obj::is_exact_type(f_obj, objfun::type_fun_native()));
        let result = runtime::call_function_n_kw(f_obj, 0, 0, &[]);
        assert_eq!(obj::small_int_value(result), 42);
    }

    #[test]
    fn compile_viper_decorator_return_const_int() {
        setup();
        if !asmbase::machine_code_dispatch_supported() {
            return;
        }
        let src = "@micropython.viper\ndef f() -> int:\n    return 42\n";
        let lex = lexer::Lexer::new_from_str_len(
            qstr::from_str("<stdin>"),
            src.as_bytes(),
            READER_IS_ROM,
        );
        let mut tree = parse::parse(lex, ParseInputKind::FileInput);
        let module_fun = compile(&mut tree, qstr::from_str("<stdin>"), false);
        runtime::call_function_n_kw(module_fun, 0, 0, &[]);
        let f_qstr = qstr::from_str("f");
        let f_obj = objdict::dict_get(mpstate::globals_get(), obj::new_qstr(f_qstr));
        assert_ne!(f_obj, obj::OBJ_NULL);
        assert!(obj::is_exact_type(f_obj, objfun::type_fun_viper()));
        let result = runtime::call_function_n_kw(f_obj, 0, 0, &[]);
        assert_eq!(obj::small_int_value(result), 42);
    }

    #[test]
    fn compile_viper_decorator_int_add() {
        setup();
        if !asmbase::machine_code_dispatch_supported() {
            return;
        }
        let src = "@micropython.viper\ndef f() -> int:\n    return 1 + 2\n";
        let lex = lexer::Lexer::new_from_str_len(
            qstr::from_str("<stdin>"),
            src.as_bytes(),
            READER_IS_ROM,
        );
        let mut tree = parse::parse(lex, ParseInputKind::FileInput);
        let module_fun = compile(&mut tree, qstr::from_str("<stdin>"), false);
        runtime::call_function_n_kw(module_fun, 0, 0, &[]);
        let f_obj = objdict::dict_get(mpstate::globals_get(), obj::new_qstr(qstr::from_str("f")));
        assert!(obj::is_exact_type(f_obj, objfun::type_fun_viper()));
        let result = runtime::call_function_n_kw(f_obj, 0, 0, &[]);
        assert_eq!(obj::small_int_value(result), 3);
    }

    // Regression test for a segfault where the JIT prologue's incoming-args
    // pointer clobbered `REG_QSTR_TABLE` (both were aliased to the same
    // physical register under `PERSISTENT_CODE_SAVE`). Any native/viper
    // function that takes an argument reproduces it; earlier tests above only
    // exercise zero-arg functions.
    #[test]
    fn compile_native_decorator_arg_plus_one() {
        setup();
        if !asmbase::machine_code_dispatch_supported() {
            return;
        }
        let src = "@micropython.native\ndef f(x):\n    return x + 1\n";
        let f_obj = compile_and_call_native(src, "f");
        assert!(obj::is_exact_type(f_obj, objfun::type_fun_native()));
        let result = runtime::call_function_n_kw(f_obj, 1, 0, &[obj::new_small_int(41)]);
        assert_eq!(obj::small_int_value(result), 42);
    }

    #[test]
    fn compile_viper_decorator_arg_plus_one() {
        setup();
        if !asmbase::machine_code_dispatch_supported() {
            return;
        }
        let src = "@micropython.viper\ndef f(x: int) -> int:\n    return x + 1\n";
        let f_obj = compile_and_call_native(src, "f");
        assert!(obj::is_exact_type(f_obj, objfun::type_fun_viper()));
        let result = runtime::call_function_n_kw(f_obj, 1, 0, &[obj::new_small_int(41)]);
        assert_eq!(obj::small_int_value(result), 42);
    }

    // Regression test mirroring `-X emit=native`: the module's default emit
    // option (not a per-function decorator) drives `SETUP_CODE_STATE` /
    // `mp_code_state_native_t` setup for a plain top-level `def`.
    #[test]
    fn compile_module_default_emit_native_function_call_with_arg() {
        setup();
        if !asmbase::machine_code_dispatch_supported() {
            return;
        }
        let prev_emit_opt = mpstate::with_vm(|vm| vm.default_emit_opt);
        mpstate::with_vm(|vm| vm.default_emit_opt = EMIT_OPT_NATIVE_PYTHON as u8);
        let src = "def f(x):\n    return x + 1\n";
        let f_obj = compile_and_call_native(src, "f");
        mpstate::with_vm(|vm| vm.default_emit_opt = prev_emit_opt);
        assert!(obj::is_exact_type(f_obj, objfun::type_fun_native()));
        let result = runtime::call_function_n_kw(f_obj, 1, 0, &[obj::new_small_int(41)]);
        assert_eq!(obj::small_int_value(result), 42);
    }

    #[test]
    fn compile_eval_one_plus_two_runs_in_vm() {
        setup();
        let src = "1+2";
        let lex = lexer::Lexer::new_from_str_len(
            qstr::from_str("<stdin>"),
            src.as_bytes(),
            READER_IS_ROM,
        );
        let mut tree = parse::parse(lex, ParseInputKind::EvalInput);
        let fun = compile(&mut tree, qstr::from_str("<stdin>"), false);
        let result = runtime::call_function_n_kw(fun, 0, 0, &[]);
        assert_eq!(result, obj::new_small_int(3));
    }

    fn compile_and_call_native(src: &str, name: &str) -> Obj {
        let lex = lexer::Lexer::new_from_str_len(
            qstr::from_str("<stdin>"),
            src.as_bytes(),
            READER_IS_ROM,
        );
        let mut tree = parse::parse(lex, ParseInputKind::FileInput);
        let module_fun = compile(&mut tree, qstr::from_str("<stdin>"), false);
        runtime::call_function_n_kw(module_fun, 0, 0, &[]);
        let f_qstr = qstr::from_str(name);
        objdict::dict_get(mpstate::globals_get(), obj::new_qstr(f_qstr))
    }

    #[test]
    fn compile_native_try_finally_runs_cleanup() {
        setup();
        if !asmbase::machine_code_dispatch_supported() {
            return;
        }
        let src = "@micropython.native\ndef f():\n    r = 0\n    try:\n        r = 1\n    finally:\n        r = 10\n    return r\n";
        let f_obj = compile_and_call_native(src, "f");
        assert!(obj::is_exact_type(f_obj, objfun::type_fun_native()));
        let result = runtime::call_function_n_kw(f_obj, 0, 0, &[]);
        assert_eq!(obj::small_int_value(result), 10);
    }

    #[test]
    fn compile_native_async_try_finally_return_runs() {
        setup();
        if !asmbase::machine_code_dispatch_supported() {
            return;
        }
        let src = "\
@micropython.native
async def f():
    try:
        return 99
    finally:
        pass
";
        let lex = lexer::Lexer::new_from_str_len(
            qstr::from_str("<stdin>"),
            src.as_bytes(),
            READER_IS_ROM,
        );
        let mut tree = parse::parse(lex, ParseInputKind::FileInput);
        let module_fun = compile(&mut tree, qstr::from_str("<stdin>"), false);
        runtime::call_function_n_kw(module_fun, 0, 0, &[]);
        let f_obj = objdict::dict_get(mpstate::globals_get(), obj::new_qstr(qstr::from_str("f")));
        let gen = runtime::call_function_n_kw(f_obj, 0, 0, &[]);
        let mut ret = obj::OBJ_NULL;
        let kind = crate::objgenerator::gen_resume(gen, obj::CONST_NONE, obj::OBJ_NULL, &mut ret);
        assert_eq!(kind, crate::runtime::VmReturnKind::Normal);
        assert_eq!(obj::small_int_value(ret), 99);
    }

    #[test]
    fn compile_native_generator_yield_once() {
        setup();
        if !asmbase::machine_code_dispatch_supported() {
            return;
        }
        let src = "@micropython.native\ndef g():\n    yield 7\n";
        let g_obj = compile_and_call_native(src, "g");
        assert!(obj::is_exact_type(
            g_obj,
            crate::objgenerator::type_native_gen_wrap()
        ));
        let gen = runtime::call_function_n_kw(g_obj, 0, 0, &[]);
        assert!(obj::is_exact_type(
            gen,
            crate::objgenerator::type_gen_instance()
        ));
        let mut ret = obj::OBJ_NULL;
        let kind = crate::objgenerator::gen_resume(gen, obj::CONST_NONE, obj::OBJ_NULL, &mut ret);
        assert_eq!(kind, crate::runtime::VmReturnKind::Yield);
        assert_eq!(obj::small_int_value(ret), 7);
        let kind2 = crate::objgenerator::gen_resume(gen, obj::CONST_NONE, obj::OBJ_NULL, &mut ret);
        assert_eq!(kind2, crate::runtime::VmReturnKind::Normal);
    }

    #[test]
    fn compile_native_generator_yield_from() {
        setup();
        if !asmbase::machine_code_dispatch_supported() {
            return;
        }
        let src = "@micropython.native\ndef inner():\n    yield 10\n    yield 20\n\n@micropython.native\ndef outer():\n    yield from inner()\n";
        let outer_obj = compile_and_call_native(src, "outer");
        assert!(obj::is_exact_type(
            outer_obj,
            crate::objgenerator::type_native_gen_wrap()
        ));
        let gen = runtime::call_function_n_kw(outer_obj, 0, 0, &[]);
        assert!(obj::is_exact_type(
            gen,
            crate::objgenerator::type_gen_instance()
        ));
        let mut ret = obj::OBJ_NULL;
        let kind = crate::objgenerator::gen_resume(gen, obj::CONST_NONE, obj::OBJ_NULL, &mut ret);
        assert_eq!(kind, crate::runtime::VmReturnKind::Yield);
        assert_eq!(obj::small_int_value(ret), 10);
        let kind2 = crate::objgenerator::gen_resume(gen, obj::CONST_NONE, obj::OBJ_NULL, &mut ret);
        assert_eq!(kind2, crate::runtime::VmReturnKind::Yield);
        assert_eq!(obj::small_int_value(ret), 20);
        let kind3 = crate::objgenerator::gen_resume(gen, obj::CONST_NONE, obj::OBJ_NULL, &mut ret);
        assert_eq!(kind3, crate::runtime::VmReturnKind::Normal);
    }

    #[test]
    fn compile_native_generator_yield_from_bytecode_delegate() {
        setup();
        if !asmbase::machine_code_dispatch_supported() {
            return;
        }
        let src = "\
def inner():
    yield 10
    yield 20

@micropython.native
def outer():
    yield from inner()
";
        let outer_obj = compile_and_call_native(src, "outer");
        let gen = runtime::call_function_n_kw(outer_obj, 0, 0, &[]);
        let mut ret = obj::OBJ_NULL;
        let kind = crate::objgenerator::gen_resume(gen, obj::CONST_NONE, obj::OBJ_NULL, &mut ret);
        assert_eq!(kind, crate::runtime::VmReturnKind::Yield);
        assert_eq!(obj::small_int_value(ret), 10);
        let kind2 = crate::objgenerator::gen_resume(gen, obj::CONST_NONE, obj::OBJ_NULL, &mut ret);
        assert_eq!(kind2, crate::runtime::VmReturnKind::Yield);
        assert_eq!(obj::small_int_value(ret), 20);
        let kind3 = crate::objgenerator::gen_resume(gen, obj::CONST_NONE, obj::OBJ_NULL, &mut ret);
        assert_eq!(kind3, crate::runtime::VmReturnKind::Normal);
    }

    fn gen_throw_expects_generator_exit(gen: Obj) {
        let mut ret = obj::OBJ_NULL;
        let kind = crate::objgenerator::gen_resume(
            gen,
            obj::CONST_NONE,
            crate::objgenerator::const_generator_exit(),
            &mut ret,
        );
        assert_eq!(
            kind,
            crate::runtime::VmReturnKind::Exception,
            "throw GeneratorExit into native generator must propagate",
        );
    }

    #[test]
    fn compile_native_generator_throw_at_yield() {
        setup();
        if !asmbase::machine_code_dispatch_supported() {
            return;
        }
        let src = "@micropython.native\ndef f():\n    yield 1\n";
        let f_obj = compile_and_call_native(src, "f");
        let gen = runtime::call_function_n_kw(f_obj, 0, 0, &[]);
        let mut ret = obj::OBJ_NULL;
        let kind = crate::objgenerator::gen_resume(gen, obj::CONST_NONE, obj::OBJ_NULL, &mut ret);
        assert_eq!(kind, crate::runtime::VmReturnKind::Yield);
        gen_throw_expects_generator_exit(gen);
    }

    #[test]
    fn compile_native_generator_yield_from_throw_native_delegate() {
        setup();
        if !asmbase::machine_code_dispatch_supported() {
            return;
        }
        let src = "\
@micropython.native
def inner():
    yield 10

@micropython.native
def outer():
    yield from inner()
";
        let outer_obj = compile_and_call_native(src, "outer");
        let gen = runtime::call_function_n_kw(outer_obj, 0, 0, &[]);
        let mut ret = obj::OBJ_NULL;
        let kind = crate::objgenerator::gen_resume(gen, obj::CONST_NONE, obj::OBJ_NULL, &mut ret);
        assert_eq!(kind, crate::runtime::VmReturnKind::Yield);
        assert_eq!(obj::small_int_value(ret), 10);
        gen_throw_expects_generator_exit(gen);
    }

    #[test]
    fn compile_native_generator_yield_from_throw_bytecode_delegate() {
        setup();
        if !asmbase::machine_code_dispatch_supported() {
            return;
        }
        let src = "\
def inner():
    yield 10

@micropython.native
def outer():
    yield from inner()
";
        let outer_obj = compile_and_call_native(src, "outer");
        let gen = runtime::call_function_n_kw(outer_obj, 0, 0, &[]);
        let mut ret = obj::OBJ_NULL;
        let kind = crate::objgenerator::gen_resume(gen, obj::CONST_NONE, obj::OBJ_NULL, &mut ret);
        assert_eq!(kind, crate::runtime::VmReturnKind::Yield);
        assert_eq!(obj::small_int_value(ret), 10);
        gen_throw_expects_generator_exit(gen);
    }

    #[test]
    fn compile_class_instantiation_works() {
        setup();
        let src = "\
class Ctx:
    pass

x = Ctx()
";
        let lex = lexer::Lexer::new_from_str_len(
            qstr::from_str("<stdin>"),
            src.as_bytes(),
            READER_IS_ROM,
        );
        let mut tree = parse::parse(lex, ParseInputKind::FileInput);
        let module_fun = compile(&mut tree, qstr::from_str("<stdin>"), false);
        runtime::call_function_n_kw(module_fun, 0, 0, &[]);
        let x = objdict::dict_get(mpstate::globals_get(), obj::new_qstr(qstr::from_str("x")));
        assert_ne!(x, obj::OBJ_NULL);
    }

    fn run_async_with_e2e(src: &str, fun_name: &str) {
        let lex = lexer::Lexer::new_from_str_len(
            qstr::from_str("<stdin>"),
            src.as_bytes(),
            READER_IS_ROM,
        );
        let mut tree = parse::parse(lex, ParseInputKind::FileInput);
        let module_fun = compile(&mut tree, qstr::from_str("<stdin>"), false);
        runtime::call_function_n_kw(module_fun, 0, 0, &[]);
        let f_obj = objdict::dict_get(
            mpstate::globals_get(),
            obj::new_qstr(qstr::from_str(fun_name)),
        );
        let gen = runtime::call_function_n_kw(f_obj, 0, 0, &[]);
        let mut ret = obj::OBJ_NULL;
        let kind = crate::objgenerator::gen_resume(gen, obj::CONST_NONE, obj::OBJ_NULL, &mut ret);
        assert_eq!(kind, crate::runtime::VmReturnKind::Normal);
        assert_eq!(obj::small_int_value(ret), 99);
    }

    #[test]
    fn compile_async_with_bytecode_runs() {
        setup();
        run_async_with_e2e(
            "\
class Ctx:
    async def __aenter__(self):
        return 99
    async def __aexit__(self, exc_type, exc, tb):
        return False

async def f():
    async with Ctx() as x:
        return x
",
            "f",
        );
    }

    #[test]
    fn compile_native_async_with_runs() {
        setup();
        if !asmbase::machine_code_dispatch_supported() {
            return;
        }
        run_async_with_e2e(
            "\
class Ctx:
    async def __aenter__(self):
        return 99
    async def __aexit__(self, exc_type, exc, tb):
        return False

@micropython.native
async def f():
    async with Ctx() as x:
        return x
",
            "f",
        );
    }

    fn compile_expect_error(src: &str) -> Obj {
        let lex = lexer::Lexer::new_from_str_len(
            qstr::from_str("<stdin>"),
            src.as_bytes(),
            READER_IS_ROM,
        );
        let mut tree = parse::parse(lex, ParseInputKind::FileInput);
        let mut nlr_buf = crate::nlr::NlrBuf::default();
        let err = crate::nlr::protect(&mut nlr_buf, || {
            compile(&mut tree, qstr::from_str("<stdin>"), false)
        });
        assert!(err.is_err(), "expected compile error for {src:?}");
        Obj(err.unwrap_err())
    }

    #[test]
    fn compile_native_async_with_compiles() {
        setup();
        if !asmbase::machine_code_dispatch_supported() {
            return;
        }
        // Ctx is resolved at runtime; compile-only check that native emitter accepts async-with.
        let native_src = "\
@micropython.native
async def f():
    async with Ctx() as x:
        return x
";
        let lex = lexer::Lexer::new_from_str_len(
            qstr::from_str("<stdin>"),
            native_src.as_bytes(),
            READER_IS_ROM,
        );
        let mut tree = parse::parse(lex, ParseInputKind::FileInput);
        let module_fun = compile(&mut tree, qstr::from_str("<stdin>"), false);
        runtime::call_function_n_kw(module_fun, 0, 0, &[]);
        let f_obj = objdict::dict_get(mpstate::globals_get(), obj::new_qstr(qstr::from_str("f")));
        assert_ne!(f_obj, obj::OBJ_NULL);
        assert!(obj::is_exact_type(
            f_obj,
            crate::objgenerator::type_native_gen_wrap()
        ));
    }

    #[test]
    fn compile_viper_yield_rejects_like_c() {
        setup();
        if !asmbase::machine_code_dispatch_supported() {
            return;
        }
        let exc = compile_expect_error("@micropython.viper\ndef g():\n    yield 1\n");
        assert!(objexcept::exception_match(
            exc,
            obj::from_ptr(
                objexcept::type_not_implemented_error() as *const obj::ObjType as *const ()
            ),
        ));
        let msg = objstr::str_get_str(objexcept::exception_get_value(exc));
        assert!(msg.contains("native yield"), "got {msg:?}");
    }

    #[test]
    fn compile_native_positional_default_args() {
        setup();
        if !asmbase::machine_code_dispatch_supported() {
            return;
        }
        let src = "@micropython.native\ndef f(a=1, b=2):\n    return a + b\n";
        let f_obj = compile_and_call_native(src, "f");
        assert!(obj::is_exact_type(f_obj, objfun::type_fun_native()));
        let result0 = runtime::call_function_n_kw(f_obj, 0, 0, &[]);
        assert_eq!(obj::small_int_value(result0), 3);
        let result1 = runtime::call_function_n_kw(f_obj, 1, 0, &[obj::new_small_int(10)]);
        assert_eq!(obj::small_int_value(result1), 12);
    }

    #[test]
    fn compile_viper_mixed_int_uint_compare_type_error() {
        setup();
        if !asmbase::machine_code_dispatch_supported() {
            return;
        }
        let exc = compile_expect_error(
            "@micropython.viper\ndef f(a: int, b: uint) -> bool:\n    return a == b\n",
        );
        assert!(objexcept::exception_match(
            exc,
            obj::from_ptr(objexcept::type_viper_type_error() as *const obj::ObjType as *const ()),
        ));
        let msg = objstr::str_get_str(objexcept::exception_get_value(exc));
        assert!(msg.contains("comparison of int and uint"), "got {msg:?}");
    }

    #[test]
    fn compile_list_comprehension_runs() {
        setup();
        let src = "[i * i for i in range(4)]";
        let lex = lexer::Lexer::new_from_str_len(
            qstr::from_str("<stdin>"),
            src.as_bytes(),
            READER_IS_ROM,
        );
        let mut tree = parse::parse(lex, ParseInputKind::EvalInput);
        let fun = compile(&mut tree, qstr::from_str("<stdin>"), false);
        let result = runtime::call_function_n_kw(fun, 0, 0, &[]);
        let (len, items) = crate::objlist::list_get(result);
        assert_eq!(len, 4);
        assert_eq!(obj::small_int_value(items[0]), 0);
        assert_eq!(obj::small_int_value(items[1]), 1);
        assert_eq!(obj::small_int_value(items[2]), 4);
        assert_eq!(obj::small_int_value(items[3]), 9);
    }

    #[test]
    fn compile_with_statement_runs() {
        setup();
        let src = "\
class T:
    def __enter__(self):
        return self
    def __exit__(self, *a):
        pass

with T() as x:
    y = 1
";
        let lex = lexer::Lexer::new_from_str_len(
            qstr::from_str("<stdin>"),
            src.as_bytes(),
            READER_IS_ROM,
        );
        let mut tree = parse::parse(lex, ParseInputKind::FileInput);
        let module_fun = compile(&mut tree, qstr::from_str("<stdin>"), false);
        runtime::call_function_n_kw(module_fun, 0, 0, &[]);
        let y = objdict::dict_get(mpstate::globals_get(), obj::new_qstr(qstr::from_str("y")));
        assert_eq!(obj::small_int_value(y), 1);
    }

    #[test]
    fn compile_dict_comprehension_runs() {
        setup();
        let src = "{i: i for i in range(3)}";
        let lex = lexer::Lexer::new_from_str_len(
            qstr::from_str("<stdin>"),
            src.as_bytes(),
            READER_IS_ROM,
        );
        let mut tree = parse::parse(lex, ParseInputKind::EvalInput);
        let fun = compile(&mut tree, qstr::from_str("<stdin>"), false);
        let result = runtime::call_function_n_kw(fun, 0, 0, &[]);
        assert_eq!(
            obj::small_int_value(objdict::dict_get(result, obj::new_small_int(0))),
            0
        );
        assert_eq!(
            obj::small_int_value(objdict::dict_get(result, obj::new_small_int(2))),
            2
        );
    }

    /// Run `src` as a module body (populating `mpstate::globals_get()`), then
    /// call the named zero-arg global function and return its result.
    fn run_and_call(src: &str, fun_name: &str) -> Obj {
        let lex = lexer::Lexer::new_from_str_len(
            qstr::from_str("<stdin>"),
            src.as_bytes(),
            READER_IS_ROM,
        );
        let mut tree = parse::parse(lex, ParseInputKind::FileInput);
        let module_fun = compile(&mut tree, qstr::from_str("<stdin>"), false);
        runtime::call_function_n_kw(module_fun, 0, 0, &[]);
        let f_obj = objdict::dict_get(
            mpstate::globals_get(),
            obj::new_qstr(qstr::from_str(fun_name)),
        );
        assert_ne!(f_obj, obj::OBJ_NULL, "global {fun_name:?} not defined");
        runtime::call_function_n_kw(f_obj, 0, 0, &[])
    }

    // --- Bug #1: try/except must catch exceptions raised by opcodes -----------

    #[test]
    fn try_except_catches_zero_division_error() {
        setup();
        let result = run_and_call(
            "\
def test():
    try:
        1 / 0
    except ZeroDivisionError as e:
        return type(e) is ZeroDivisionError
    return False
",
            "test",
        );
        assert_eq!(result, obj::CONST_TRUE, "ZeroDivisionError was not caught");
    }

    #[test]
    fn try_except_matches_correct_type_and_skips_others() {
        setup();
        // A `try` with multiple `except` clauses must dispatch to the first
        // matching clause and let non-matching ones fall through.
        let result = run_and_call(
            "\
def test():
    try:
        [1, 2][10]
    except ZeroDivisionError:
        return 'wrong'
    except IndexError:
        return 'right'
    return 'none'
",
            "test",
        );
        assert_eq!(objstr::str_get_str(result), "right");
    }

    #[test]
    fn raise_from_still_raises_new_exception() {
        setup();
        // C micropython warns that exception chaining isn't supported but
        // still raises the new (TypeError) exception; the original
        // (ValueError) must not leak out instead.
        let result = run_and_call(
            "\
def test():
    try:
        try:
            raise ValueError(1)
        except ValueError as e:
            raise TypeError(2) from e
    except TypeError as e2:
        return e2.args[0]
    return -1
",
            "test",
        );
        assert_eq!(obj::small_int_value(result), 2);
    }

    // --- Bug #2: keyword-only args and positional defaults ---------------------

    #[test]
    fn positional_default_and_kwonly_args() {
        setup();
        let result = run_and_call(
            "\
def f(a, b=2, *, c=3):
    return a + b + c

def test():
    return f(1, c=4)
",
            "test",
        );
        assert_eq!(obj::small_int_value(result), 7);
    }

    #[test]
    fn positional_default_arg_no_call_args() {
        setup();
        let result = run_and_call(
            "\
def f(a=1):
    return a

def test():
    return f()
",
            "test",
        );
        assert_eq!(obj::small_int_value(result), 1);
    }

    #[test]
    fn kwonly_default_arg_with_and_without_override() {
        setup();
        let result = run_and_call(
            "\
def g(*, x=1):
    return x

def test():
    return (g(), g(x=2))
",
            "test",
        );
        let (len, items) = objtuple::tuple_get(result);
        assert_eq!(len, 2);
        assert_eq!(obj::small_int_value(items[0]), 1);
        assert_eq!(obj::small_int_value(items[1]), 2);
    }

    #[test]
    fn kwonly_missing_required_arg_raises_type_error() {
        setup();
        let src = "\
def g(*, x):
    return x

def test():
    return g()
";
        let lex = lexer::Lexer::new_from_str_len(
            qstr::from_str("<stdin>"),
            src.as_bytes(),
            READER_IS_ROM,
        );
        let mut tree = parse::parse(lex, ParseInputKind::FileInput);
        let module_fun = compile(&mut tree, qstr::from_str("<stdin>"), false);
        runtime::call_function_n_kw(module_fun, 0, 0, &[]);
        let f_obj = objdict::dict_get(
            mpstate::globals_get(),
            obj::new_qstr(qstr::from_str("test")),
        );
        let mut nlr_buf = crate::nlr::NlrBuf::default();
        let err = crate::nlr::protect(&mut nlr_buf, || {
            runtime::call_function_n_kw(f_obj, 0, 0, &[])
        });
        assert!(err.is_err(), "missing keyword-only arg should raise");
        assert!(objexcept::exception_match(
            Obj(err.unwrap_err()),
            obj::from_ptr(objexcept::type_type_error() as *const obj::ObjType as *const ()),
        ));
    }

    // --- Bug #3: super() ---------------------------------------------------

    #[test]
    fn basic_subclass_instantiation_and_isinstance() {
        setup();
        // Regression test for `class_lookup` incorrectly resolving a parent
        // type's *metaclass* instead of the parent type itself, which broke
        // even trivial single-inheritance subclass instantiation.
        let result = run_and_call(
            "\
class A:
    pass

class B(A):
    pass

def test():
    return isinstance(B(), A)
",
            "test",
        );
        assert_eq!(result, obj::CONST_TRUE);
    }

    #[test]
    fn super_zero_arg_call_dispatches_to_parent_method() {
        setup();
        let result = run_and_call(
            "\
class A:
    def f(self):
        return 1

class B(A):
    def f(self):
        return super().f() + 1

def test():
    return B().f()
",
            "test",
        );
        assert_eq!(obj::small_int_value(result), 2);
    }

    #[test]
    fn super_init_and_attribute_expression_variants() {
        setup();
        // Covers super().__init__(...), a bare `super()` bound to a local,
        // and super() used inside a larger expression (stack accounting).
        let result = run_and_call(
            "\
class A:
    def __init__(self, x):
        self.x = x
    def f(self):
        return 1

class B(A):
    def __init__(self, x):
        super().__init__(x * 2)
    def f(self):
        s = super()
        return 1 + s.f()

def test():
    b = B(3)
    return (b.x, b.f())
",
            "test",
        );
        let (len, items) = objtuple::tuple_get(result);
        assert_eq!(len, 2);
        assert_eq!(obj::small_int_value(items[0]), 6);
        assert_eq!(obj::small_int_value(items[1]), 2);
    }

    // --- Bug #4: @property must act as a data descriptor -----------------------

    #[test]
    fn property_getter_returns_computed_value() {
        setup();
        let result = run_and_call(
            "\
class A:
    @property
    def x(self):
        return 7

def test():
    return A().x
",
            "test",
        );
        assert_eq!(obj::small_int_value(result), 7);
    }

    #[test]
    fn property_setter_via_decorator_syntax() {
        setup();
        // Exercises `@x.setter`, which requires property.setter to be
        // reachable via attribute lookup on the property object, and
        // requires plain (non-call) attribute lookup of a bound method to
        // yield a bound-method value rather than immediately invoking it.
        let result = run_and_call(
            "\
class A:
    def __init__(self):
        self._x = 0
    @property
    def x(self):
        return self._x
    @x.setter
    def x(self, v):
        self._x = v * 2

def test():
    a = A()
    a.x = 5
    return a.x
",
            "test",
        );
        assert_eq!(obj::small_int_value(result), 10);
    }

    // --- Regressions found while adding the above tests --------------------

    #[test]
    fn parenthesized_multi_element_tuple_of_identifiers() {
        setup();
        // Regression test for `compile_atom_paren` checking `Rule::Testlist`
        // instead of `Rule::TestlistComp`: a parenthesized tuple whose items
        // aren't constant-foldable (so it can't be short-circuited into a
        // single `LOAD_CONST_OBJ`) fell through to a path that pushed each
        // element but never emitted `BUILD_TUPLE`, unbalancing the value
        // stack (`assertion failed: emit.stack_size == 0`).
        let result = run_and_call(
            "\
def test():
    a = 1
    b = 2
    return (a, b)
",
            "test",
        );
        let (len, items) = objtuple::tuple_get(result);
        assert_eq!(len, 2);
        assert_eq!(obj::small_int_value(items[0]), 1);
        assert_eq!(obj::small_int_value(items[1]), 2);
    }

    #[test]
    fn negative_small_int_literal_near_multi_encoding_boundary() {
        setup();
        // Regression test for `mp_emit_bc_load_const_small_int`'s multi-byte
        // encoding: the C reference does `MP_BC_LOAD_CONST_SMALL_INT_MULTI +
        // MP_BC_LOAD_CONST_SMALL_INT_MULTI_EXCESS + arg` in wide signed
        // arithmetic and only truncates to a `byte` at the very end. Casting
        // a negative `arg` to `u8` *before* adding (as an early version of
        // the Rust port did) wraps e.g. -1 into 255 and then overflows the
        // subsequent `u8 + u8` addition, panicking in debug builds.
        //
        // Note: a *tuple* literal of constants gets folded by the parser
        // into a single `LOAD_CONST_OBJ`, which would bypass
        // `mp_emit_bc_load_const_small_int` entirely and not exercise the
        // bug. A list literal isn't constant-folded, so each element still
        // goes through the per-element small-int emission path.
        let result = run_and_call(
            "\
def test():
    return [-1, -16, -17]
",
            "test",
        );
        let (len, items) = crate::objlist::list_get(result);
        assert_eq!(len, 3);
        assert_eq!(obj::small_int_value(items[0]), -1);
        assert_eq!(obj::small_int_value(items[1]), -16);
        assert_eq!(obj::small_int_value(items[2]), -17);
    }

    // --- Star unpack in assignment (`a, b, *c = ...`) --------------------------

    #[test]
    fn star_unpack_trailing_captures_remainder_as_list() {
        setup();
        let result = run_and_call(
            "\
def test():
    a, b, *c = [1, 2, 3, 4]
    return (a, b, c)
",
            "test",
        );
        let (len, items) = objtuple::tuple_get(result);
        assert_eq!(len, 3);
        assert_eq!(obj::small_int_value(items[0]), 1);
        assert_eq!(obj::small_int_value(items[1]), 2);
        let (clen, citems) = crate::objlist::list_get(items[2]);
        assert_eq!(clen, 2);
        assert_eq!(obj::small_int_value(citems[0]), 3);
        assert_eq!(obj::small_int_value(citems[1]), 4);
    }

    #[test]
    fn star_unpack_leading_captures_prefix_as_list() {
        setup();
        let result = run_and_call(
            "\
def test():
    *a, b = [1, 2, 3]
    return (a, b)
",
            "test",
        );
        let (len, items) = objtuple::tuple_get(result);
        assert_eq!(len, 2);
        let (alen, aitems) = crate::objlist::list_get(items[0]);
        assert_eq!(alen, 2);
        assert_eq!(obj::small_int_value(aitems[0]), 1);
        assert_eq!(obj::small_int_value(aitems[1]), 2);
        assert_eq!(obj::small_int_value(items[1]), 3);
    }

    // --- Star args in calls (`f(*(1,2), 3)`) ------------------------------------

    #[test]
    fn star_args_unpacked_in_call_middle_and_end() {
        setup();
        let result = run_and_call(
            "\
def f(a, b, c):
    return a + b + c

def test():
    return (f(*(1, 2), 3), f(*[1, 2, 3]), f(1, *(2, 3)))
",
            "test",
        );
        let (len, items) = objtuple::tuple_get(result);
        assert_eq!(len, 3);
        assert_eq!(obj::small_int_value(items[0]), 6);
        assert_eq!(obj::small_int_value(items[1]), 6);
        assert_eq!(obj::small_int_value(items[2]), 6);
    }

    // --- List slice assign + del item / del dict key ----------------------------

    #[test]
    fn list_slice_assign_replaces_range() {
        setup();
        let result = run_and_call(
            "\
def test():
    a = [1, 2, 3, 4]
    a[1:3] = [9, 8]
    return a
",
            "test",
        );
        let (len, items) = crate::objlist::list_get(result);
        assert_eq!(len, 4);
        assert_eq!(obj::small_int_value(items[0]), 1);
        assert_eq!(obj::small_int_value(items[1]), 9);
        assert_eq!(obj::small_int_value(items[2]), 8);
        assert_eq!(obj::small_int_value(items[3]), 4);
    }

    #[test]
    fn list_del_item_removes_element() {
        setup();
        let result = run_and_call(
            "\
def test():
    a = [1, 2, 3]
    del a[1]
    return a
",
            "test",
        );
        let (len, items) = crate::objlist::list_get(result);
        assert_eq!(len, 2);
        assert_eq!(obj::small_int_value(items[0]), 1);
        assert_eq!(obj::small_int_value(items[1]), 3);
    }

    #[test]
    fn dict_del_item_removes_key() {
        setup();
        let result = run_and_call(
            "\
def test():
    d = {1: 2}
    del d[1]
    return d
",
            "test",
        );
        assert_eq!(objdict::dict_len(result), 0);
    }

    // --- isinstance / issubclass -------------------------------------------------

    #[test]
    fn isinstance_of_small_int_against_int_type() {
        setup();
        let result = run_and_call("def test():\n    return isinstance(1, int)\n", "test");
        assert_eq!(result, obj::CONST_TRUE);
    }

    #[test]
    fn issubclass_bool_is_not_subclass_of_int() {
        setup();
        // MicroPython, unlike CPython, treats `bool` and `int` as unrelated
        // builtin types: `issubclass(bool, int)` is `False`.
        let result = run_and_call("def test():\n    return issubclass(bool, int)\n", "test");
        assert_eq!(result, obj::CONST_FALSE);
    }

    #[test]
    fn isinstance_true_against_bool_type() {
        setup();
        let result = run_and_call("def test():\n    return isinstance(True, bool)\n", "test");
        assert_eq!(result, obj::CONST_TRUE);
    }

    // --- min/max multi-arg form ---------------------------------------------------

    #[test]
    fn min_max_accept_multiple_positional_args() {
        setup();
        let result = run_and_call(
            "\
def test():
    return (min(3, 1, 2), max(3, 1, 2), min([3, 1, 2]))
",
            "test",
        );
        let (len, items) = objtuple::tuple_get(result);
        assert_eq!(len, 3);
        assert_eq!(obj::small_int_value(items[0]), 1);
        assert_eq!(obj::small_int_value(items[1]), 3);
        assert_eq!(obj::small_int_value(items[2]), 1);
    }

    // --- bytes literals must produce `bytes`, not `str` -------------------------

    #[test]
    fn bytes_literal_is_actual_bytes_type() {
        setup();
        let result = run_and_call(
            "\
def test():
    return isinstance(b'hi', bytes)
",
            "test",
        );
        assert_eq!(result, obj::CONST_TRUE);
    }

    #[test]
    fn bytes_repr_is_prefixed_with_b() {
        setup();
        let result = run_and_call("def test():\n    return repr(b'hi')\n", "test");
        assert_eq!(objstr::str_get_str(result), "b'hi'");
    }

    #[test]
    fn bytearray_from_bytes_literal_roundtrips() {
        setup();
        let result = run_and_call(
            "\
def test():
    x = bytearray(b'hi')
    return (repr(x), x[0], x[1])
",
            "test",
        );
        let (len, items) = objtuple::tuple_get(result);
        assert_eq!(len, 3);
        assert_eq!(objstr::str_get_str(items[0]), "bytearray(b'hi')");
        assert_eq!(obj::small_int_value(items[1]), b'h' as obj::Int);
        assert_eq!(obj::small_int_value(items[2]), b'i' as obj::Int);
    }

    #[test]
    fn memoryview_of_bytes_literal_indexes_correctly() {
        setup();
        let result = run_and_call(
            "\
def test():
    m = memoryview(b'ab')
    return (m[0], m[1])
",
            "test",
        );
        let (len, items) = objtuple::tuple_get(result);
        assert_eq!(len, 2);
        assert_eq!(obj::small_int_value(items[0]), b'a' as obj::Int);
        assert_eq!(obj::small_int_value(items[1]), b'b' as obj::Int);
    }
}
