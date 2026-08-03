//! rewrite of py/scope.c + py/scope.h
// symmetry: done

use crate::emitglue;
use crate::malloc;
use crate::mpconfig;
use crate::parse::{self, ParseNode, ParseNodeStruct};
use crate::qstr::{self, Qstr};

/// Identifier binding kind (`id_info_kind_t`).
#[repr(u8)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum IdInfoKind {
    Undecided = 0,
    GlobalImplicit,
    GlobalImplicitAssigned,
    GlobalExplicit,
    Local,
    Cell,
    Free,
}

pub const ID_FLAG_IS_PARAM: u8 = 0x01;
pub const ID_FLAG_IS_STAR_PARAM: u8 = 0x02;
pub const ID_FLAG_IS_DBL_STAR_PARAM: u8 = 0x04;
pub const ID_FLAG_VIPER_TYPE_POS: u8 = 4;

/// Per-name scope information (`id_info_t`).
#[derive(Copy, Clone, Debug)]
pub struct IdInfo {
    pub kind: IdInfoKind,
    pub flags: u8,
    pub local_num: u16,
    pub qst: Qstr,
}

impl IdInfo {
    pub fn new(kind: IdInfoKind, qst: Qstr) -> Self {
        Self {
            kind,
            flags: 0,
            local_num: 0,
            qst,
        }
    }
}

/// Scope kind (`scope_kind_t`).
#[repr(u8)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ScopeKind {
    Module = 0,
    Class,
    Lambda,
    ListComp,
    DictComp,
    SetComp,
    GenExpr,
    Function,
}

pub fn scope_is_func_like(kind: ScopeKind) -> bool {
    kind as u8 >= ScopeKind::Lambda as u8
}

pub fn scope_is_comp_like(kind: ScopeKind) -> bool {
    matches!(
        kind,
        ScopeKind::ListComp | ScopeKind::DictComp | ScopeKind::SetComp | ScopeKind::GenExpr
    )
}

/// Lexical scope / block (`scope_t`).
pub struct Scope {
    pub kind: ScopeKind,
    pub parent: Option<*mut Scope>,
    pub next: Option<*mut Scope>,
    pub pn: ParseNode,
    pub raw_code: *mut emitglue::RawCode,
    #[cfg(debug_assertions)]
    pub raw_code_data_len: usize,
    pub simple_name: Qstr,
    pub scope_flags: u16,
    pub emit_options: u16,
    pub num_pos_args: u16,
    pub num_kwonly_args: u16,
    pub num_def_pos_args: u16,
    pub num_locals: u16,
    pub stack_size: u16,
    pub exc_stack_size: u16,
    pub id_info: Vec<IdInfo>,
}

fn scope_simple_name_table(kind: ScopeKind) -> Qstr {
    match kind {
        ScopeKind::Module => qstr::from_str("<module>"),
        ScopeKind::Lambda => qstr::from_str("<lambda>"),
        ScopeKind::ListComp => qstr::from_str("<listcomp>"),
        ScopeKind::DictComp => qstr::from_str("<dictcomp>"),
        ScopeKind::SetComp => qstr::from_str("<setcomp>"),
        ScopeKind::GenExpr => qstr::from_str("<genexpr>"),
        _ => qstr::QSTR_NULL,
    }
}

/// `scope_new`
pub fn new(kind: ScopeKind, pn: ParseNode, emit_options: u16) -> *mut Scope {
    let scope = malloc::new_obj::<Scope>().expect("scope alloc");
    unsafe {
        (*scope).kind = kind;
        (*scope).pn = pn;
        if matches!(kind, ScopeKind::Function | ScopeKind::Class) {
            debug_assert!(parse::parse_node_is_struct(pn));
            let pns = pn as *const ParseNodeStruct;
            (*scope).simple_name = parse::parse_node_leaf_arg(parse::parse_node_struct_node(pns, 0));
        } else {
            (*scope).simple_name = scope_simple_name_table(kind);
        }
        (*scope).raw_code = emitglue::new_raw_code();
        (*scope).emit_options = emit_options;
        (*scope).scope_flags = 0;
        (*scope).num_pos_args = 0;
        (*scope).num_kwonly_args = 0;
        (*scope).num_def_pos_args = 0;
        (*scope).num_locals = 0;
        (*scope).stack_size = 0;
        (*scope).exc_stack_size = 0;
        (*scope).id_info = Vec::with_capacity(mpconfig::ALLOC_SCOPE_ID_INIT as usize);
        (*scope).parent = None;
        (*scope).next = None;
        #[cfg(debug_assertions)]
        {
            (*scope).raw_code_data_len = 0;
        }
    }
    scope
}

/// `scope_free`
pub fn free(scope: *mut Scope) {
    if scope.is_null() {
        return;
    }
    unsafe {
        (*scope).id_info.clear();
        malloc::del_obj(scope);
    }
}

/// `scope_find_or_add_id`
pub fn find_or_add_id(scope: &mut Scope, qst: Qstr, kind: IdInfoKind) -> &mut IdInfo {
    if let Some(id) = find(scope, qst) {
        let idx = scope.id_info.iter().position(|i| i.qst == qst).unwrap();
        return &mut scope.id_info[idx];
    }

    if scope.id_info.len() >= scope.id_info.capacity() {
        scope
            .id_info
            .reserve(mpconfig::ALLOC_SCOPE_ID_INC as usize);
    }

    scope.id_info.push(IdInfo::new(kind, qst));
    let len = scope.id_info.len();
    &mut scope.id_info[len - 1]
}

/// `scope_find`
pub fn find(scope: &Scope, qst: Qstr) -> Option<&IdInfo> {
    scope.id_info.iter().find(|id| id.qst == qst)
}

/// `scope_find_global`
pub fn find_global(mut scope: &Scope, qst: Qstr) -> Option<&IdInfo> {
    while let Some(parent) = scope.parent {
        scope = unsafe { &*parent };
    }
    find(scope, qst)
}

fn scope_close_over_in_parents(scope: &mut Scope, qst: Qstr) {
    debug_assert!(scope.parent.is_some());
    let mut s = scope.parent.unwrap();
    loop {
        let s_ref = unsafe { &mut *s };
        debug_assert!(s_ref.parent.is_some());
        let id = find_or_add_id(s_ref, qst, IdInfoKind::Undecided);
        if id.kind == IdInfoKind::Undecided {
            id.kind = IdInfoKind::Free;
        } else {
            if id.kind == IdInfoKind::Local {
                id.kind = IdInfoKind::Cell;
            } else {
                debug_assert!(id.kind == IdInfoKind::Free || id.kind == IdInfoKind::Cell);
            }
            return;
        }
        s = s_ref.parent.unwrap();
    }
}

/// `scope_check_to_close_over`
pub fn check_to_close_over(scope: &mut Scope, id: &mut IdInfo) {
    if scope.parent.is_none() {
        return;
    }
    let mut s = scope.parent;
    while let Some(sp) = s {
        let s_ref = unsafe { &*sp };
        if s_ref.parent.is_none() {
            break;
        }
        if let Some(id2) = find(s_ref, id.qst) {
            if matches!(
                id2.kind,
                IdInfoKind::Local | IdInfoKind::Cell | IdInfoKind::Free
            ) {
                id.kind = IdInfoKind::Free;
                scope_close_over_in_parents(scope, id.qst);
            }
            break;
        }
        s = s_ref.parent;
    }
}
