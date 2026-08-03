//! rewrite of py/parse.h + py/parse.c
// symmetry: done

use core::ptr;

use crate::grammar::{self, Rule};
use crate::lexer::{Lexer, TokenKind};
use crate::malloc;
use crate::map::{self, LookupKind, Map};
use crate::mpconfig;
use crate::nlr::{self, NlrBuf};
use crate::obj::{self, Int, Obj};
use crate::objfloat;
use crate::objtuple;
use crate::parsenum;
use crate::qstr::{self, Qstr};
use crate::raise::{self, MpRaise};
use crate::reader::READER_IS_ROM;
use crate::runtime::{self, RuntimeError};
use crate::runtime0::{BinaryOp, UnaryOp};

// --- parse node encoding (parse.h) ---

pub type ParseNode = usize;

pub const PARSE_NODE_NULL: ParseNode = 0;
pub const PARSE_NODE_SMALL_INT: ParseNode = 0x1;
pub const PARSE_NODE_ID: ParseNode = 0x02;
pub const PARSE_NODE_STRING: ParseNode = 0x06;
pub const PARSE_NODE_TOKEN: ParseNode = 0x0a;

#[repr(C)]
pub struct ParseNodeStruct {
    pub source_line: u32,
    pub kind_num_nodes: u32,
}

#[inline]
pub fn parse_node_is_null(pn: ParseNode) -> bool {
    pn == PARSE_NODE_NULL
}

#[inline]
pub fn parse_node_is_leaf(pn: ParseNode) -> bool {
    (pn & 3) != 0
}

#[inline]
pub fn parse_node_is_struct(pn: ParseNode) -> bool {
    pn != PARSE_NODE_NULL && (pn & 3) == 0
}

#[inline]
pub fn parse_node_is_struct_kind(pn: ParseNode, kind: Rule) -> bool {
    pn != PARSE_NODE_NULL
        && (pn & 3) == 0
        && parse_node_struct_kind(pn as *const ParseNodeStruct) == kind as u32
}

#[inline]
pub fn parse_node_is_small_int(pn: ParseNode) -> bool {
    (pn & 0x1) == PARSE_NODE_SMALL_INT
}

#[inline]
pub fn parse_node_is_id(pn: ParseNode) -> bool {
    (pn & 0x0f) == PARSE_NODE_ID
}

#[inline]
pub fn parse_node_is_token(pn: ParseNode) -> bool {
    (pn & 0x0f) == PARSE_NODE_TOKEN
}

#[inline]
pub fn parse_node_is_token_kind(pn: ParseNode, kind: TokenKind) -> bool {
    pn == (PARSE_NODE_TOKEN | ((kind as usize) << 4))
}

#[inline]
pub fn parse_node_leaf_kind(pn: ParseNode) -> usize {
    pn & 0x0f
}

#[inline]
pub fn parse_node_leaf_arg(pn: ParseNode) -> usize {
    pn >> 4
}

#[inline]
pub fn parse_node_leaf_small_int(pn: ParseNode) -> Int {
    (pn as Int) >> 1
}

#[inline]
pub fn parse_node_struct_kind(pns: *const ParseNodeStruct) -> u32 {
    unsafe { (*pns).kind_num_nodes & 0xff }
}

#[inline]
pub fn parse_node_struct_num_nodes(pns: *const ParseNodeStruct) -> usize {
    unsafe { ((*pns).kind_num_nodes >> 8) as usize }
}

#[inline]
pub fn parse_node_new_small_int(val: Int) -> ParseNode {
    PARSE_NODE_SMALL_INT | ((val as usize) << 1)
}

#[inline]
pub fn parse_node_new_leaf(kind: usize, arg: usize) -> ParseNode {
    kind | (arg << 4)
}

#[inline]
unsafe fn parse_node_struct_nodes(pns: *const ParseNodeStruct) -> *const ParseNode {
    (pns as *const u8).add(core::mem::size_of::<ParseNodeStruct>()) as *const ParseNode
}

#[inline]
unsafe fn parse_node_struct_nodes_mut(pns: *mut ParseNodeStruct) -> *mut ParseNode {
    (pns as *mut u8).add(core::mem::size_of::<ParseNodeStruct>()) as *mut ParseNode
}

pub fn parse_node_extract_const_object(pns: *const ParseNodeStruct) -> Obj {
    unsafe { Obj(*parse_node_struct_nodes(pns)) }
}

#[inline]
pub fn parse_node_struct_node(pns: *const ParseNodeStruct, index: usize) -> ParseNode {
    unsafe { *parse_node_struct_nodes(pns).add(index) }
}

#[inline]
pub fn parse_node_struct_node_mut(pns: *mut ParseNodeStruct, index: usize) -> ParseNode {
    unsafe { *parse_node_struct_nodes_mut(pns).add(index) }
}

#[inline]
pub fn parse_node_struct_set_node(pns: *mut ParseNodeStruct, index: usize, node: ParseNode) {
    unsafe {
        *parse_node_struct_nodes_mut(pns).add(index) = node;
    }
}

// --- parse tree ---

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseInputKind {
    SingleInput,
    FileInput,
    EvalInput,
}

struct ParseChunk {
    alloc: usize,
    used: usize,
    next: Option<Box<ParseChunk>>,
    data: Vec<u8>,
}

pub struct ParseTree {
    pub root: ParseNode,
    chunk: Option<Box<ParseChunk>>,
}

pub fn parse_tree_clear(tree: &mut ParseTree) {
    tree.chunk = None;
    tree.root = PARSE_NODE_NULL;
}

// --- errors ---

#[derive(Debug)]
pub enum ParseError {
    Syntax(&'static str),
    Runtime(RuntimeError),
}

impl From<RuntimeError> for ParseError {
    fn from(e: RuntimeError) -> Self {
        ParseError::Runtime(e)
    }
}

// --- parser internals ---

struct RuleStackEntry {
    src_line: usize,
    rule_id: u8,
    arg_i: usize,
}

struct Parser {
    rule_stack: Vec<RuleStackEntry>,
    result_stack: Vec<ParseNode>,
    lexer: Lexer,
    tree: ParseTree,
    cur_chunk: Option<ParseChunk>,
    consts: Map,
}

impl Parser {
    fn parser_alloc(&mut self, num_bytes: usize) -> *mut u8 {
        let chunk = &mut self.cur_chunk;
        if let Some(c) = chunk {
            if c.used + num_bytes > c.alloc {
                let old_total = core::mem::size_of::<ParseChunk>() + c.alloc;
                let new_total = old_total + num_bytes;
                if malloc::renew_maybe(c.data.as_mut_ptr(), old_total, new_total, false).is_none() {
                    c.alloc = c.used;
                    let finished = self.cur_chunk.take().unwrap();
                    let mut boxed = Box::new(ParseChunk {
                        alloc: finished.alloc,
                        used: finished.used,
                        next: None,
                        data: finished.data,
                    });
                    boxed.next = self.tree.chunk.take();
                    self.tree.chunk = Some(boxed);
                    self.cur_chunk = None;
                } else {
                    c.alloc += num_bytes;
                }
            }
        }

        if self.cur_chunk.is_none() {
            let mut alloc = mpconfig::ALLOC_PARSE_CHUNK_INIT as usize;
            if alloc < num_bytes {
                alloc = num_bytes;
            }
            self.cur_chunk = Some(ParseChunk {
                alloc,
                used: 0,
                next: None,
                data: vec![0u8; alloc],
            });
        }

        let c = self.cur_chunk.as_mut().unwrap();
        let ret = unsafe { c.data.as_mut_ptr().add(c.used) };
        c.used += num_bytes;
        ret
    }

    fn push_rule(&mut self, src_line: usize, rule_id: u8, arg_i: usize) {
        self.rule_stack.push(RuleStackEntry {
            src_line,
            rule_id,
            arg_i,
        });
    }

    fn push_rule_from_arg(&mut self, arg: u16) {
        debug_assert!(
            (arg & grammar::RULE_ARG_KIND_MASK) == grammar::RULE_ARG_RULE
                || (arg & grammar::RULE_ARG_KIND_MASK) == grammar::RULE_ARG_OPT_RULE
        );
        let rule_id = (arg & grammar::RULE_ARG_ARG_MASK) as u8;
        self.push_rule(self.lexer.tok_line, rule_id, 0);
    }

    fn pop_rule(&mut self) -> (u8, usize, usize) {
        let rs = self.rule_stack.pop().expect("rule stack underflow");
        (rs.rule_id, rs.arg_i, rs.src_line)
    }

    fn peek_rule(&self, n: usize) -> u8 {
        assert!(self.rule_stack.len() > n);
        self.rule_stack[self.rule_stack.len() - 1 - n].rule_id
    }

    fn pop_result(&mut self) -> ParseNode {
        self.result_stack.pop().expect("result stack underflow")
    }

    fn peek_result(&self, pos: usize) -> ParseNode {
        assert!(self.result_stack.len() > pos);
        self.result_stack[self.result_stack.len() - 1 - pos]
    }

    fn push_result_node(&mut self, pn: ParseNode) {
        self.result_stack.push(pn);
    }

    fn make_node_const_object(&mut self, src_line: u32, obj: Obj) -> ParseNode {
        let pn = self.parser_alloc(
            core::mem::size_of::<ParseNodeStruct>() + core::mem::size_of::<ParseNode>(),
        ) as *mut ParseNodeStruct;
        unsafe {
            (*pn).source_line = src_line;
            (*pn).kind_num_nodes = Rule::ConstObject as u32 | (1 << 8);
            *parse_node_struct_nodes_mut(pn) = obj.0;
        }
        pn as ParseNode
    }

    fn make_node_const_object_optimised(&mut self, src_line: u32, obj: Obj) -> ParseNode {
        if obj::is_small_int(obj) {
            return parse_node_new_small_int(obj::small_int_value(obj));
        }
        self.make_node_const_object(src_line, obj)
    }

    fn push_result_token(&mut self, rule_id: u8) {
        let lex = &self.lexer;
        let pn = match lex.tok_kind {
            TokenKind::Name => {
                let text = lex.token_text();
                let id = qstr::from_strn(text.as_bytes());
                let const_val = if mpconfig::COMP_CONST && rule_id == Rule::Atom as u8 {
                    map::lookup(
                        &mut self.consts,
                        obj::new_qstr(id as usize),
                        LookupKind::Lookup,
                    )
                    .map(|e| e.value)
                } else {
                    None
                };
                if let Some(val) = const_val {
                    let pn = self.make_node_const_object_optimised(lex.tok_line as u32, val);
                    self.push_result_node(pn);
                    return;
                }
                parse_node_new_leaf(PARSE_NODE_ID, id as usize)
            }
            TokenKind::Integer => {
                let text = lex.token_text();
                let o = parsenum::parse_num_integer(text.as_bytes(), 0, None);
                self.make_node_const_object_optimised(lex.tok_line as u32, o)
            }
            TokenKind::FloatOrImag => {
                let text = lex.token_text();
                let o = parsenum::parse_num_float(text.as_bytes(), true, None);
                self.make_node_const_object(lex.tok_line as u32, o)
            }
            TokenKind::String => {
                let text = lex.token_text();
                let bytes = text.as_bytes();
                let qst = if bytes.len() <= mpconfig::ALLOC_PARSE_INTERN_STRING_LEN as usize {
                    qstr::from_strn(bytes)
                } else {
                    let found = qstr::find_strn(bytes);
                    if found != 0 {
                        found
                    } else {
                        qstr::from_strn(bytes)
                    }
                };
                if qst != 0 {
                    parse_node_new_leaf(PARSE_NODE_STRING, qst as usize)
                } else {
                    self.make_node_const_object(
                        lex.tok_line as u32,
                        obj::new_qstr(qstr::from_strn(bytes) as usize),
                    )
                }
            }
            TokenKind::Bytes => {
                let bytes = lex.token_bytes();
                self.make_node_const_object(lex.tok_line as u32, crate::objstr::new_bytes(bytes))
            }
            _ => parse_node_new_leaf(PARSE_NODE_TOKEN, lex.tok_kind as usize),
        };
        self.push_result_node(pn);
    }

    fn push_result_rule(&mut self, src_line: usize, rule_id: u8, mut num_args: usize) {
        if rule_id == Rule::AtomParen as u8 {
            let pn = self.peek_result(0);
            if parse_node_is_null(pn) {
            } else if parse_node_is_struct_kind(pn, Rule::TestlistComp) {
            } else {
                return;
            }
        } else if rule_id == Rule::TestlistComp as u8 {
            debug_assert_eq!(num_args, 2);
            let pn = self.peek_result(0);
            if parse_node_is_struct(pn) {
                let pns = pn as *mut ParseNodeStruct;
                let kind = parse_node_struct_kind(pns);
                if kind == Rule::TestlistComp3b as u32 {
                    self.pop_result();
                    num_args -= 1;
                } else if kind == Rule::TestlistComp3c as u32 {
                    self.pop_result();
                    unsafe {
                        (*pns).kind_num_nodes =
                            (Rule::TestlistComp as u32) | ((*pns).kind_num_nodes & !0xff);
                    }
                    return;
                }
            }
        } else if rule_id == Rule::TestlistComp3c as u8 {
            num_args += 1;
        }

        if mpconfig::COMP_CONST_FOLDING {
            if self.fold_logical_constants(rule_id, &mut num_args) {
                return;
            }
            if self.fold_constants(rule_id, num_args) {
                return;
            }
        }

        if mpconfig::COMP_CONST_TUPLE {
            if self.build_tuple(src_line, rule_id, num_args) {
                return;
            }
        }

        let pn = self.parser_alloc(
            core::mem::size_of::<ParseNodeStruct>() + core::mem::size_of::<ParseNode>() * num_args,
        ) as *mut ParseNodeStruct;
        unsafe {
            (*pn).source_line = src_line as u32;
            (*pn).kind_num_nodes = (rule_id as u32) | ((num_args as u32) << 8);
            let nodes = parse_node_struct_nodes_mut(pn);
            for i in (0..num_args).rev() {
                *nodes.add(i) = self.pop_result();
            }
        }
        if rule_id == Rule::TestlistComp3c as u8 {
            self.push_result_node(pn as ParseNode);
        }
        self.push_result_node(pn as ParseNode);
    }

    fn syntax_error(&self, msg: &'static str) -> ! {
        raise::raise(MpRaise::SyntaxError(msg));
    }

    fn parse_loop(&mut self, input_kind: ParseInputKind) {
        let mut backtrack = false;

        'next_rule: loop {
            if self.rule_stack.is_empty() {
                break;
            }

            let (rule_id, mut i, rule_src_line) = self.pop_rule();
            let rule_act = grammar::RULE_ACT_TABLE[rule_id as usize];
            let rule_arg = grammar::rule_arg(rule_id);
            let n = (rule_act & grammar::RULE_ACT_ARG_MASK) as usize;

            match rule_act & grammar::RULE_ACT_KIND_MASK {
                grammar::RULE_ACT_OR => {
                    if i > 0 && !backtrack {
                        continue 'next_rule;
                    }
                    backtrack = false;
                    let mut matched = false;
                    for idx in i..n {
                        let kind = rule_arg[idx] & grammar::RULE_ARG_KIND_MASK;
                        if kind == grammar::RULE_ARG_TOK {
                            if self.lexer.tok_kind
                                == TokenKind::from_u8(
                                    (rule_arg[idx] & grammar::RULE_ARG_ARG_MASK) as u8,
                                )
                            {
                                self.push_result_token(rule_id);
                                self.lexer.to_next();
                                matched = true;
                                break;
                            }
                        } else {
                            debug_assert_eq!(kind, grammar::RULE_ARG_RULE);
                            if idx + 1 < n {
                                self.push_rule(rule_src_line, rule_id, idx + 1);
                            }
                            self.push_rule_from_arg(rule_arg[idx]);
                            continue 'next_rule;
                        }
                    }
                    if matched {
                        continue 'next_rule;
                    }
                    backtrack = true;
                }

                grammar::RULE_ACT_AND => {
                    if backtrack {
                        debug_assert!(i > 0);
                        if (rule_arg[i - 1] & grammar::RULE_ARG_KIND_MASK)
                            == grammar::RULE_ARG_OPT_RULE
                        {
                            self.push_result_node(PARSE_NODE_NULL);
                            backtrack = false;
                        } else if i > 1 {
                            self.syntax_error("invalid syntax");
                        } else {
                            continue 'next_rule;
                        }
                    }

                    let progressed = false;
                    for idx in i..n {
                        if (rule_arg[idx] & grammar::RULE_ARG_KIND_MASK) == grammar::RULE_ARG_TOK {
                            let tok_kind = TokenKind::from_u8(
                                (rule_arg[idx] & grammar::RULE_ARG_ARG_MASK) as u8,
                            );
                            if self.lexer.tok_kind == tok_kind {
                                if tok_kind == TokenKind::Name {
                                    self.push_result_token(rule_id);
                                }
                                self.lexer.to_next();
                            } else if idx > 0 {
                                self.syntax_error("invalid syntax");
                            } else {
                                backtrack = true;
                                continue 'next_rule;
                            }
                        } else {
                            self.push_rule(rule_src_line, rule_id, idx + 1);
                            self.push_rule_from_arg(rule_arg[idx]);
                            continue 'next_rule;
                        }
                    }
                    let _ = progressed;
                    debug_assert_eq!(i.max(n), n);

                    if !mpconfig::ENABLE_DOC_STRING
                        && input_kind != ParseInputKind::SingleInput
                        && rule_id == Rule::ExprStmt as u8
                        && self.peek_result(0) == PARSE_NODE_NULL
                    {
                        let p = self.peek_result(1);
                        if (parse_node_is_leaf(p) && !parse_node_is_id(p))
                            || parse_node_is_struct_kind(p, Rule::ConstObject)
                        {
                            self.pop_result();
                            self.pop_result();
                            self.push_result_rule(rule_src_line, Rule::PassStmt as u8, 0);
                            continue 'next_rule;
                        }
                    }

                    let mut stack_args = 0usize;
                    let mut num_not_nil = 0usize;
                    for x in (0..n).rev() {
                        if (rule_arg[x] & grammar::RULE_ARG_KIND_MASK) == grammar::RULE_ARG_TOK {
                            if TokenKind::from_u8((rule_arg[x] & grammar::RULE_ARG_ARG_MASK) as u8)
                                == TokenKind::Name
                            {
                                stack_args += 1;
                                num_not_nil += 1;
                            }
                        } else {
                            if self.peek_result(stack_args) != PARSE_NODE_NULL {
                                num_not_nil += 1;
                            }
                            stack_args += 1;
                        }
                    }

                    if num_not_nil == 1 && (rule_act & grammar::RULE_ACT_ALLOW_IDENT) != 0 {
                        let mut pn = PARSE_NODE_NULL;
                        for _ in 0..stack_args {
                            let pn2 = self.pop_result();
                            if pn2 != PARSE_NODE_NULL {
                                pn = pn2;
                            }
                        }
                        self.push_result_node(pn);
                    } else {
                        if (rule_act & grammar::RULE_ACT_ADD_BLANK) != 0 {
                            self.push_result_node(PARSE_NODE_NULL);
                            stack_args += 1;
                        }
                        self.push_result_rule(rule_src_line, rule_id, stack_args);
                    }
                }

                _ => {
                    debug_assert_eq!(
                        rule_act & grammar::RULE_ACT_KIND_MASK,
                        grammar::RULE_ACT_LIST
                    );
                    let mut had_trailing_sep = false;
                    'list_rule: loop {
                        if backtrack {
                            had_trailing_sep = false;
                            if n == 2 {
                                if i == 1 {
                                    continue 'next_rule;
                                }
                                backtrack = false;
                            } else if i == 1 {
                                continue 'next_rule;
                            } else if (i & 1) == 1 {
                                if n == 3 {
                                    had_trailing_sep = true;
                                    backtrack = false;
                                } else {
                                    self.syntax_error("invalid syntax");
                                }
                            } else {
                                backtrack = false;
                            }
                            break 'list_rule;
                        }

                        let arg = rule_arg[i & 1 & n];
                        if (arg & grammar::RULE_ARG_KIND_MASK) == grammar::RULE_ARG_TOK {
                            if self.lexer.tok_kind
                                == TokenKind::from_u8((arg & grammar::RULE_ARG_ARG_MASK) as u8)
                            {
                                if (i & 1 & n) == 0 {
                                    self.push_result_token(rule_id);
                                }
                                self.lexer.to_next();
                                i += 1;
                            } else {
                                i += 1;
                                backtrack = true;
                                continue 'list_rule;
                            }
                        } else {
                            debug_assert_eq!(
                                arg & grammar::RULE_ARG_KIND_MASK,
                                grammar::RULE_ARG_RULE
                            );
                            self.push_rule(rule_src_line, rule_id, i + 1);
                            self.push_rule_from_arg(arg);
                            continue 'next_rule;
                        }
                    }

                    debug_assert!(i >= 1);
                    i -= 1;
                    if (n & 1) != 0
                        && (rule_arg[1] & grammar::RULE_ARG_KIND_MASK) == grammar::RULE_ARG_TOK
                    {
                        i = (i + 1) / 2;
                    }

                    if i == 1 {
                        if had_trailing_sep {
                            self.push_result_rule(rule_src_line, rule_id, i);
                        }
                    } else {
                        self.push_result_rule(rule_src_line, rule_id, i);
                    }
                }
            }
        }
    }
}

// --- const folding (parse.c) ---

impl Parser {
    fn parse_node_get_number_maybe(pn: ParseNode) -> Option<Obj> {
        if parse_node_is_small_int(pn) {
            Some(obj::new_small_int(parse_node_leaf_small_int(pn)))
        } else if parse_node_is_struct_kind(pn, Rule::ConstObject) {
            let o = parse_node_extract_const_object(pn as *const ParseNodeStruct);
            if obj::is_int(o) || (mpconfig::COMP_CONST_FLOAT && objfloat::is_float(o)) {
                Some(o)
            } else {
                None
            }
        } else {
            None
        }
    }

    fn parse_node_is_const(pn: ParseNode) -> bool {
        if parse_node_is_small_int(pn) {
            return true;
        }
        if parse_node_is_leaf(pn) {
            let kind = parse_node_leaf_kind(pn);
            if kind == PARSE_NODE_STRING {
                return true;
            }
            if kind == PARSE_NODE_TOKEN {
                let arg = parse_node_leaf_arg(pn);
                return matches!(
                    arg,
                    x if x == TokenKind::KwNone as usize
                        || x == TokenKind::KwFalse as usize
                        || x == TokenKind::KwTrue as usize
                        || x == TokenKind::Ellipsis as usize
                );
            }
        } else if parse_node_is_struct_kind(pn, Rule::ConstObject) {
            return true;
        } else if parse_node_is_struct_kind(pn, Rule::AtomParen) {
            let pns = pn as *const ParseNodeStruct;
            unsafe {
                return parse_node_is_null(*parse_node_struct_nodes(pns));
            }
        }
        false
    }

    fn parse_node_convert_to_obj(pn: ParseNode) -> Obj {
        debug_assert!(Self::parse_node_is_const(pn));
        if parse_node_is_small_int(pn) {
            return obj::new_small_int(parse_node_leaf_small_int(pn));
        }
        if parse_node_is_leaf(pn) {
            let kind = parse_node_leaf_kind(pn);
            let arg = parse_node_leaf_arg(pn);
            if kind == PARSE_NODE_STRING {
                return obj::new_qstr(arg);
            }
            debug_assert_eq!(kind, PARSE_NODE_TOKEN);
            return match TokenKind::from_u8(arg as u8) {
                TokenKind::KwNone => obj::CONST_NONE,
                TokenKind::KwFalse => obj::CONST_FALSE,
                TokenKind::KwTrue => obj::CONST_TRUE,
                _ => obj::CONST_NONE,
            };
        }
        if parse_node_is_struct_kind(pn, Rule::ConstObject) {
            return parse_node_extract_const_object(pn as *const ParseNodeStruct);
        }
        debug_assert!(parse_node_is_struct_kind(pn, Rule::AtomParen));
        objtuple::new_tuple(0, None)
    }

    fn fold_logical_constants(&mut self, rule_id: u8, num_args: &mut usize) -> bool {
        if rule_id == Rule::OrTest as u8 || rule_id == Rule::AndTest as u8 {
            let mut copy_to = *num_args;
            let mut i = copy_to;
            while i > 0 {
                i -= 1;
                let pn = self.peek_result(i);
                let top = self.result_stack.len();
                self.result_stack[top - copy_to] = pn;
                if i == 0 {
                    break;
                }
                if rule_id == Rule::OrTest as u8 {
                    if parse_node_is_const_true(pn) {
                        break;
                    } else if !parse_node_is_const_false(pn) {
                        copy_to -= 1;
                    }
                } else if parse_node_is_const_false(pn) {
                    break;
                } else if !parse_node_is_const_true(pn) {
                    copy_to -= 1;
                }
            }
            copy_to -= 1;
            for _ in 0..copy_to {
                self.pop_result();
            }
            *num_args -= copy_to;
            return *num_args == 1;
        }
        if rule_id == Rule::NotTest2 as u8 {
            let pn = self.peek_result(0);
            let folded = if parse_node_is_const_false(pn) {
                parse_node_new_leaf(PARSE_NODE_TOKEN, TokenKind::KwTrue as usize)
            } else if parse_node_is_const_true(pn) {
                parse_node_new_leaf(PARSE_NODE_TOKEN, TokenKind::KwFalse as usize)
            } else {
                return false;
            };
            self.pop_result();
            self.push_result_node(folded);
            return true;
        }
        false
    }

    fn binary_op_maybe(op: BinaryOp, lhs: Obj, rhs: Obj) -> Option<Obj> {
        let mut buf = NlrBuf::default();
        match nlr::protect(&mut buf, || runtime::binary_op_obj(op, lhs, rhs)) {
            Ok(v) => Some(v),
            Err(_) => None,
        }
    }

    fn fold_constants(&mut self, rule_id: u8, num_args: usize) -> bool {
        if num_args == 0 {
            return false;
        }
        let mut arg0 = match Self::parse_node_get_number_maybe(self.peek_result(num_args - 1)) {
            Some(v) => v,
            None if matches!(
                rule_id,
                x if x == Rule::Expr as u8
                    || x == Rule::XorExpr as u8
                    || x == Rule::AndExpr as u8
                    || x == Rule::Power as u8
                    || x == Rule::ShiftExpr as u8
                    || x == Rule::ArithExpr as u8
                    || x == Rule::Term as u8
            ) =>
            {
                return false
            }
            None if rule_id == Rule::Factor2 as u8 => {
                let Some(v) = Self::parse_node_get_number_maybe(self.peek_result(0)) else {
                    return false;
                };
                let tok = parse_node_leaf_arg(self.peek_result(1));
                let op = match TokenKind::from_u8(tok as u8) {
                    TokenKind::OpTilde => {
                        if !obj::is_int(v) {
                            return false;
                        }
                        UnaryOp::Invert
                    }
                    TokenKind::OpPlus => UnaryOp::Positive,
                    TokenKind::OpMinus => UnaryOp::Negative,
                    _ => return false,
                };
                let folded = runtime::unary_op_obj(op, v);
                for _ in 0..num_args {
                    self.pop_result();
                }
                let pn = self.make_node_const_object_optimised(0, folded);
                self.push_result_node(pn);
                return true;
            }
            None => return false,
        };

        if matches!(
            rule_id,
            x if x == Rule::Expr as u8
                || x == Rule::XorExpr as u8
                || x == Rule::AndExpr as u8
                || x == Rule::Power as u8
        ) {
            let op = match rule_id {
                x if x == Rule::Expr as u8 => BinaryOp::Or,
                x if x == Rule::XorExpr as u8 => BinaryOp::Xor,
                x if x == Rule::AndExpr as u8 => BinaryOp::And,
                _ => BinaryOp::Power,
            };
            let mut idx = num_args as isize - 2;
            while idx >= 0 {
                let pn = self.peek_result(idx as usize);
                let Some(arg1) = Self::parse_node_get_number_maybe(pn) else {
                    return false;
                };
                let Some(next) = Self::binary_op_maybe(op, arg0, arg1) else {
                    return false;
                };
                arg0 = next;
                idx -= 1;
            }
        } else if matches!(
            rule_id,
            x if x == Rule::ShiftExpr as u8 || x == Rule::ArithExpr as u8 || x == Rule::Term as u8
        ) {
            let mut idx = num_args as isize - 2;
            while idx >= 1 {
                let pn = self.peek_result((idx - 1) as usize);
                let Some(arg1) = Self::parse_node_get_number_maybe(pn) else {
                    return false;
                };
                let tok = parse_node_leaf_arg(self.peek_result(idx as usize));
                let op = match TokenKind::from_u8(tok as u8) {
                    TokenKind::OpDblLess => BinaryOp::Lshift,
                    TokenKind::OpDblMore => BinaryOp::Rshift,
                    TokenKind::OpPlus => BinaryOp::Add,
                    TokenKind::OpMinus => BinaryOp::Subtract,
                    TokenKind::OpStar => BinaryOp::Multiply,
                    TokenKind::OpAt => BinaryOp::MatMult,
                    TokenKind::OpSlash => BinaryOp::TrueDivide,
                    TokenKind::OpPercent => BinaryOp::Modulo,
                    TokenKind::OpDblSlash => BinaryOp::FloorDivide,
                    _ => return false,
                };
                let Some(next) = Self::binary_op_maybe(op, arg0, arg1) else {
                    return false;
                };
                arg0 = next;
                idx -= 2;
            }
        } else {
            return false;
        }

        for _ in 0..num_args {
            self.pop_result();
        }
        let pn = self.make_node_const_object_optimised(0, arg0);
        self.push_result_node(pn);
        true
    }

    fn build_tuple_from_stack(&mut self, src_line: usize, num_args: usize) -> bool {
        for i in (0..num_args).rev() {
            if !Self::parse_node_is_const(self.peek_result(i)) {
                return false;
            }
        }
        let mut items = Vec::with_capacity(num_args);
        for _ in 0..num_args {
            items.push(Self::parse_node_convert_to_obj(self.pop_result()));
        }
        items.reverse();
        let tuple = objtuple::new_tuple(num_args, Some(&items));
        let pn = self.make_node_const_object(src_line as u32, tuple);
        self.push_result_node(pn);
        true
    }

    fn build_tuple(&mut self, src_line: usize, rule_id: u8, num_args: usize) -> bool {
        if rule_id == Rule::TestlistComp as u8 && self.peek_rule(0) == Rule::AtomParen as u8 {
            return self.build_tuple_from_stack(src_line, num_args);
        }
        if rule_id == Rule::TestlistComp3c as u8 {
            debug_assert_eq!(self.peek_rule(0), Rule::TestlistComp3b as u8);
            debug_assert_eq!(self.peek_rule(1), Rule::TestlistComp as u8);
            if self.peek_rule(2) == Rule::AtomParen as u8
                && self.build_tuple_from_stack(src_line, num_args)
            {
                self.rule_stack.truncate(self.rule_stack.len() - 2);
                return true;
            }
        }
        if matches!(
            rule_id,
            x if x == Rule::TestlistStarExpr as u8
                || x == Rule::Testlist as u8
                || x == Rule::Subscriptlist as u8
        ) {
            return self.build_tuple_from_stack(src_line, num_args);
        }
        false
    }
}

fn parse_node_is_const_bool(pn: ParseNode, value: bool) -> bool {
    if mpconfig::COMP_CONST_TUPLE || mpconfig::COMP_CONST {
        Parser::parse_node_is_const(pn)
            && obj::is_true(Parser::parse_node_convert_to_obj(pn)) == value
    } else {
        parse_node_is_token_kind(
            pn,
            if value {
                TokenKind::KwTrue
            } else {
                TokenKind::KwFalse
            },
        ) || (parse_node_is_small_int(pn) && (parse_node_leaf_small_int(pn) != 0) == value)
    }
}

pub fn parse_node_is_const_false(pn: ParseNode) -> bool {
    parse_node_is_const_bool(pn, false)
}

pub fn parse_node_is_const_true(pn: ParseNode) -> bool {
    parse_node_is_const_bool(pn, true)
}

pub fn parse_node_get_int_maybe(pn: ParseNode, out: &mut Obj) -> bool {
    if let Some(o) = Parser::parse_node_get_number_maybe(pn) {
        if mpconfig::COMP_CONST_FLOAT && !obj::is_int(o) {
            return false;
        }
        *out = o;
        true
    } else {
        false
    }
}

pub fn parse_node_extract_list(
    pn: &mut ParseNode,
    pn_kind: Rule,
    nodes: &mut *mut ParseNode,
) -> usize {
    if parse_node_is_null(*pn) {
        *nodes = ptr::null_mut();
        0
    } else if parse_node_is_leaf(*pn) {
        *nodes = pn;
        1
    } else {
        let pns = *pn as *const ParseNodeStruct;
        if parse_node_struct_kind(pns) != pn_kind as u32 {
            *nodes = pn;
            1
        } else {
            unsafe {
                *nodes = parse_node_struct_nodes(pns) as *mut ParseNode;
                parse_node_struct_num_nodes(pns)
            }
        }
    }
}

/// Parse source using MicroPython's grammar (`mp_parse`).
pub fn parse(lex: Lexer, input_kind: ParseInputKind) -> ParseTree {
    let mut parser = Parser {
        rule_stack: Vec::with_capacity(mpconfig::ALLOC_PARSE_RULE_INIT as usize),
        result_stack: Vec::with_capacity(mpconfig::ALLOC_PARSE_RESULT_INIT as usize),
        lexer: lex,
        tree: ParseTree {
            root: PARSE_NODE_NULL,
            chunk: None,
        },
        cur_chunk: None,
        consts: Map::default(),
    };

    if mpconfig::COMP_CONST {
        map::init(&mut parser.consts, 0);
    }

    let top_level_rule = match input_kind {
        ParseInputKind::SingleInput => Rule::SingleInput as u8,
        ParseInputKind::EvalInput => Rule::EvalInput as u8,
        ParseInputKind::FileInput => Rule::FileInput as u8,
    };
    parser.push_rule(parser.lexer.tok_line, top_level_rule, 0);
    parser.parse_loop(input_kind);

    if mpconfig::COMP_CONST {
        map::deinit(&mut parser.consts);
    }

    if let Some(chunk) = parser.cur_chunk.take() {
        let mut boxed = Box::new(ParseChunk {
            alloc: chunk.used,
            used: chunk.used,
            next: None,
            data: chunk.data,
        });
        boxed.next = parser.tree.chunk.take();
        parser.tree.chunk = Some(boxed);
    }

    let lex = &parser.lexer;
    if lex.tok_kind != TokenKind::End || parser.result_stack.is_empty() {
        if lex.tok_kind == TokenKind::Indent {
            raise::raise(MpRaise::SyntaxError("unexpected indent"));
        } else if lex.tok_kind == TokenKind::DedentMismatch {
            raise::raise(MpRaise::SyntaxError(
                "unindent doesn't match any outer indent level",
            ));
        } else if lex.tok_kind == TokenKind::MalformedFstring {
            raise::raise(MpRaise::SyntaxError("malformed f-string"));
        } else {
            raise::raise(MpRaise::SyntaxError("invalid syntax"));
        }
    }

    assert_eq!(parser.result_stack.len(), 1);
    parser.tree.root = parser.result_stack[0];
    parser.tree
}

/// Evaluate a constant arithmetic expression from a parse tree root.
pub fn eval_const_expr(root: ParseNode) -> Result<Obj, RuntimeError> {
    fn eval_pn(pn: ParseNode) -> Result<Obj, RuntimeError> {
        if parse_node_is_small_int(pn) {
            return Ok(obj::new_small_int(parse_node_leaf_small_int(pn)));
        }
        if parse_node_is_struct_kind(pn, Rule::ConstObject) {
            let o = parse_node_extract_const_object(pn as *const ParseNodeStruct);
            if obj::is_small_int(o) {
                return Ok(o);
            }
            return Err(RuntimeError::TypeError("expected small int"));
        }
        if !parse_node_is_struct(pn) {
            return Err(RuntimeError::TypeError("expected expression"));
        }
        let pns = pn as *const ParseNodeStruct;
        let kind = parse_node_struct_kind(pns);
        let n = parse_node_struct_num_nodes(pns);
        unsafe {
            let nodes = parse_node_struct_nodes(pns);
            match kind as u8 {
                x if x == Rule::ArithExpr as u8 => {
                    eval_list_binops(nodes, n, eval_pn, arith_token_to_op)
                }
                x if x == Rule::Term as u8 => eval_list_binops(nodes, n, eval_pn, term_token_to_op),
                x if x == Rule::ShiftExpr as u8 => {
                    eval_list_binops(nodes, n, eval_pn, shift_token_to_op)
                }
                x if x == Rule::Factor2 as u8 => {
                    let tok = parse_node_leaf_arg(*nodes.add(0));
                    let v = eval_pn(*nodes.add(1))?;
                    unary_from_token(tok, v)
                }
                x if x == Rule::Power as u8 => {
                    let mut v = eval_pn(*nodes.add(0))?;
                    if n > 1 {
                        let rhs = eval_pn(*nodes.add(1))?;
                        v = runtime::binary_op(BinaryOp::Power, v, rhs)?;
                    }
                    Ok(v)
                }
                x if x == Rule::AtomExprNormal as u8 || x == Rule::AtomExprAwait as u8 => {
                    eval_pn(*nodes.add(0))
                }
                x if x == Rule::AtomParen as u8 => {
                    if n == 1 && parse_node_is_null(*nodes) {
                        return Ok(objtuple::new_tuple(0, None));
                    }
                    eval_pn(*nodes.add(0))
                }
                x if x == Rule::Testlist as u8 || x == Rule::TestlistStarExpr as u8 => {
                    eval_pn(*nodes.add(0))
                }
                x if x == Rule::Test as u8
                    || x == Rule::TestIfExpr as u8
                    || x == Rule::OrTest as u8
                    || x == Rule::AndTest as u8
                    || x == Rule::NotTest2 as u8
                    || x == Rule::Comparison as u8
                    || x == Rule::Expr as u8
                    || x == Rule::XorExpr as u8
                    || x == Rule::AndExpr as u8
                    || x == Rule::NamedexprTest as u8
                    || x == Rule::EvalInput as u8 =>
                {
                    eval_pn(*nodes.add(0))
                }
                _ => Err(RuntimeError::TypeError("unsupported expr in smoke eval")),
            }
        }
    }

    fn eval_list_binops(
        nodes: *const ParseNode,
        n: usize,
        eval: fn(ParseNode) -> Result<Obj, RuntimeError>,
        tok_op: fn(usize) -> Result<BinaryOp, RuntimeError>,
    ) -> Result<Obj, RuntimeError> {
        if n == 0 {
            return Err(RuntimeError::TypeError("empty expr"));
        }
        let mut acc = eval(unsafe { *nodes.add(0) })?;
        let mut i = 1usize;
        while i + 1 < n {
            let op = tok_op(parse_node_leaf_arg(unsafe { *nodes.add(i) }))?;
            let rhs = eval(unsafe { *nodes.add(i + 1) })?;
            acc = runtime::binary_op(op, acc, rhs)?;
            i += 2;
        }
        Ok(acc)
    }

    fn arith_token_to_op(tok: usize) -> Result<BinaryOp, RuntimeError> {
        match TokenKind::from_u8(tok as u8) {
            TokenKind::OpPlus => Ok(BinaryOp::Add),
            TokenKind::OpMinus => Ok(BinaryOp::Subtract),
            _ => Err(RuntimeError::TypeError("bad arith op")),
        }
    }

    fn term_token_to_op(tok: usize) -> Result<BinaryOp, RuntimeError> {
        match TokenKind::from_u8(tok as u8) {
            TokenKind::OpStar => Ok(BinaryOp::Multiply),
            TokenKind::OpDblSlash => Ok(BinaryOp::FloorDivide),
            TokenKind::OpPercent => Ok(BinaryOp::Modulo),
            TokenKind::OpSlash => Err(RuntimeError::TypeError("true division not in smoke path")),
            _ => Err(RuntimeError::TypeError("bad term op")),
        }
    }

    fn shift_token_to_op(tok: usize) -> Result<BinaryOp, RuntimeError> {
        match TokenKind::from_u8(tok as u8) {
            TokenKind::OpDblLess => Ok(BinaryOp::Lshift),
            TokenKind::OpDblMore => Ok(BinaryOp::Rshift),
            _ => Err(RuntimeError::TypeError("bad shift op")),
        }
    }

    fn unary_from_token(tok: usize, v: Obj) -> Result<Obj, RuntimeError> {
        if !obj::is_small_int(v) {
            return Err(RuntimeError::TypeError("unary on non-int"));
        }
        let n = obj::small_int_value(v);
        match TokenKind::from_u8(tok as u8) {
            TokenKind::OpPlus => runtime::unary_op(UnaryOp::Positive, n),
            TokenKind::OpMinus => runtime::unary_op(UnaryOp::Negative, n),
            TokenKind::OpTilde => runtime::unary_op(UnaryOp::Invert, n),
            _ => Err(RuntimeError::TypeError("bad unary op")),
        }
    }

    eval_pn(root)
}

/// Parse `eval_input` and evaluate constant arithmetic (`1+2` smoke path).
pub fn eval_expr(src: &str) -> Result<Obj, ParseError> {
    crate::gc::init();
    qstr::init();
    let src_name = qstr::from_str("<expr>");
    let lex = Lexer::new_from_str_len(src_name, src.trim().as_bytes(), READER_IS_ROM);
    let tree = parse(lex, ParseInputKind::EvalInput);
    match eval_const_expr(tree.root) {
        Ok(o) => Ok(o),
        Err(e) => Err(ParseError::Runtime(e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn init() {
        crate::gc::init();
        qstr::init();
    }

    #[test]
    fn eval_precedence() {
        init();
        let o = eval_expr("1+2*3").unwrap();
        assert_eq!(obj::small_int_value(o), 7);
        let o = eval_expr("(1+2)*3").unwrap();
        assert_eq!(obj::small_int_value(o), 9);
        let o = eval_expr("7//2").unwrap();
        assert_eq!(obj::small_int_value(o), 3);
    }

    #[test]
    fn parse_builds_tree_for_addition() {
        init();
        let src_name = qstr::from_str("<t>");
        let lex = Lexer::new_from_str_len(src_name, b"1+2", READER_IS_ROM);
        let tree = parse(lex, ParseInputKind::EvalInput);
        assert!(!parse_node_is_null(tree.root));
    }
}
