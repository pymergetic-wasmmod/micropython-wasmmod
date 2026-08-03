//! rewrite of py/lexer.h + py/lexer.c
// symmetry: done

use crate::malloc;
use crate::misc::{self, Byte, Unichar};
use crate::mpconfig;
use crate::qstr::{self, Qstr};
use crate::raise::{self, MpRaise};
use crate::reader::{self, Reader, READER_EOF, READER_IS_ROM};
use crate::unicode::{
    unichar_isalpha, unichar_isdigit, unichar_isspace, unichar_isxdigit, unichar_xdigit_value,
};
use crate::vstr::{self, Vstr};

const TAB_SIZE: usize = 8;
const LEXER_EOF: u32 = b'\0' as u32;
const LEXER_INVALID_BYTE: u8 = b'\x01';

/// Token kinds — order and discriminants match `mp_token_kind_t` for the active `mpconfig`.
#[repr(u8)]
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum TokenKind {
    End,
    Invalid,
    DedentMismatch,
    LonelyStringOpen,
    MalformedFstring,
    Newline,
    Indent,
    Dedent,
    Name,
    Integer,
    FloatOrImag,
    String,
    Bytes,
    Ellipsis,
    KwFalse,
    KwNone,
    KwTrue,
    KwDebug,
    KwAnd,
    KwAs,
    KwAssert,
    KwAsync,
    KwAwait,
    KwBreak,
    KwClass,
    KwContinue,
    KwDef,
    KwDel,
    KwElif,
    KwElse,
    KwExcept,
    KwFinally,
    KwFor,
    KwFrom,
    KwGlobal,
    KwIf,
    KwImport,
    KwIn,
    KwIs,
    KwLambda,
    KwNonlocal,
    KwNot,
    KwOr,
    KwPass,
    KwRaise,
    KwReturn,
    KwTry,
    KwWhile,
    KwWith,
    KwYield,
    OpAssign,
    OpTilde,
    OpLess,
    OpMore,
    OpDblEqual,
    OpLessEqual,
    OpMoreEqual,
    OpNotEqual,
    OpPipe,
    OpCaret,
    OpAmpersand,
    OpDblLess,
    OpDblMore,
    OpPlus,
    OpMinus,
    OpStar,
    OpAt,
    OpDblSlash,
    OpSlash,
    OpPercent,
    OpDblStar,
    DelPipeEqual,
    DelCaretEqual,
    DelAmpersandEqual,
    DelDblLessEqual,
    DelDblMoreEqual,
    DelPlusEqual,
    DelMinusEqual,
    DelStarEqual,
    DelAtEqual,
    DelDblSlashEqual,
    DelSlashEqual,
    DelPercentEqual,
    DelDblStarEqual,
    DelParenOpen,
    DelParenClose,
    DelBracketOpen,
    DelBracketClose,
    DelBraceOpen,
    DelBraceClose,
    DelComma,
    DelColon,
    DelPeriod,
    DelSemicolon,
    DelEqual,
    DelMinusMore,
    NumberOf,
}

impl TokenKind {
    /// Decode a grammar-table token discriminant (`mp_token_kind_t`).
    pub const fn from_u8(v: u8) -> Self {
        // SAFETY: parser grammar only references valid `TokenKind` values.
        unsafe { core::mem::transmute(v) }
    }
}

crate::static_assert!((TokenKind::NumberOf as usize) <= 256);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub text: String,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct LexError {
    pub message: &'static str,
    pub line: usize,
    pub column: usize,
}

/// Lexer state (`mp_lexer_t`).
pub struct Lexer {
    pub source_name: Qstr,
    pub reader: Reader,

    chr0: u32,
    chr1: u8,
    chr2: u8,

    pub line: usize,
    pub column: usize,

    emit_dent: i32,
    nested_bracket_level: i32,

    alloc_indent_level: usize,
    num_indent_level: usize,
    indent_level: *mut u16,

    pub tok_line: usize,
    pub tok_column: usize,
    pub tok_kind: TokenKind,
    pub vstr: Vstr,

    inject_chrs: Vstr,
    inject_chrs_idx: usize,
    fstring_args: Vstr,
}

// --- operator encoding tables (same as py/lexer.c) ---

const TOK_ENC: &[u8] = b"()[]{},;~:e=<e=c<e=>e=c>e=*e=c*e=+e=-e=e>&e=|e=/e=c/e=%e=^e=@e==e=!.";

const TOK_ENC_KIND: [TokenKind; 44] = [
    TokenKind::DelParenOpen,
    TokenKind::DelParenClose,
    TokenKind::DelBracketOpen,
    TokenKind::DelBracketClose,
    TokenKind::DelBraceOpen,
    TokenKind::DelBraceClose,
    TokenKind::DelComma,
    TokenKind::DelSemicolon,
    TokenKind::OpTilde,
    TokenKind::DelColon,
    TokenKind::OpAssign,
    TokenKind::OpLess,
    TokenKind::OpLessEqual,
    TokenKind::OpDblLess,
    TokenKind::DelDblLessEqual,
    TokenKind::OpMore,
    TokenKind::OpMoreEqual,
    TokenKind::OpDblMore,
    TokenKind::DelDblMoreEqual,
    TokenKind::OpStar,
    TokenKind::DelStarEqual,
    TokenKind::OpDblStar,
    TokenKind::DelDblStarEqual,
    TokenKind::OpPlus,
    TokenKind::DelPlusEqual,
    TokenKind::OpMinus,
    TokenKind::DelMinusEqual,
    TokenKind::DelMinusMore,
    TokenKind::OpAmpersand,
    TokenKind::DelAmpersandEqual,
    TokenKind::OpPipe,
    TokenKind::DelPipeEqual,
    TokenKind::OpSlash,
    TokenKind::DelSlashEqual,
    TokenKind::OpDblSlash,
    TokenKind::DelDblSlashEqual,
    TokenKind::OpPercent,
    TokenKind::DelPercentEqual,
    TokenKind::OpCaret,
    TokenKind::DelCaretEqual,
    TokenKind::OpAt,
    TokenKind::DelAtEqual,
    TokenKind::DelEqual,
    TokenKind::OpDblEqual,
];

const TOK_KW: &[&str] = &[
    "False",
    "None",
    "True",
    "__debug__",
    "and",
    "as",
    "assert",
    "async",
    "await",
    "break",
    "class",
    "continue",
    "def",
    "del",
    "elif",
    "else",
    "except",
    "finally",
    "for",
    "from",
    "global",
    "if",
    "import",
    "in",
    "is",
    "lambda",
    "nonlocal",
    "not",
    "or",
    "pass",
    "raise",
    "return",
    "try",
    "while",
    "with",
    "yield",
];

fn optimise_value() -> i32 {
    0
}

impl Lexer {
    fn cur_char(&self) -> u32 {
        self.chr0
    }

    fn is_end(&self) -> bool {
        self.chr0 == LEXER_EOF
    }

    fn is_physical_newline(&self) -> bool {
        self.chr0 == b'\n' as u32
    }

    fn is_char(&self, c: u8) -> bool {
        self.chr0 == c as u32
    }

    fn is_char_or(&self, c1: u8, c2: u8) -> bool {
        self.chr0 == c1 as u32 || self.chr0 == c2 as u32
    }

    fn is_char_or3(&self, c1: u8, c2: u8, c3: u8) -> bool {
        self.chr0 == c1 as u32 || self.chr0 == c2 as u32 || self.chr0 == c3 as u32
    }

    fn is_char_or4(&self, c1: u8, c2: u8, c3: u8, c4: u8) -> bool {
        self.chr0 == c1 as u32
            || self.chr0 == c2 as u32
            || self.chr0 == c3 as u32
            || self.chr0 == c4 as u32
    }

    fn is_char_following(&self, c: u8) -> bool {
        self.chr1 == c
    }

    fn is_char_following_or(&self, c1: u8, c2: u8) -> bool {
        self.chr1 == c1 || self.chr1 == c2
    }

    fn is_char_following_following_or(&self, c1: u8, c2: u8) -> bool {
        self.chr2 == c1 || self.chr2 == c2
    }

    fn is_char_and(&self, c1: u8, c2: u8) -> bool {
        self.chr0 == c1 as u32 && self.chr1 == c2
    }

    fn is_whitespace(&self) -> bool {
        unichar_isspace(self.chr0 as Unichar)
    }

    fn is_letter(&self) -> bool {
        unichar_isalpha(self.chr0 as Unichar)
    }

    fn is_digit(&self) -> bool {
        unichar_isdigit(self.chr0 as Unichar)
    }

    fn is_following_digit(&self) -> bool {
        unichar_isdigit(self.chr1 as Unichar)
    }

    fn is_following_base_char(&self) -> bool {
        let chr1 = self.chr1 | 0x20;
        chr1 == b'b' || chr1 == b'o' || chr1 == b'x'
    }

    fn is_following_odigit(&self) -> bool {
        self.chr1 >= b'0' && self.chr1 <= b'7'
    }

    fn is_string_or_bytes(&self) -> bool {
        if self.is_char_or(b'\'', b'"') {
            return true;
        }
        if mpconfig::PY_FSTRINGS {
            if self.is_char_or4(b'r', b'u', b'b', b'f') && self.is_char_following_or(b'\'', b'"') {
                return true;
            }
            if (self.is_char_and(b'r', b'f') || self.is_char_and(b'f', b'r'))
                && self.is_char_following_following_or(b'\'', b'"')
            {
                return true;
            }
        } else {
            if self.is_char_or3(b'r', b'u', b'b') && self.is_char_following_or(b'\'', b'"') {
                return true;
            }
        }
        (self.is_char_and(b'r', b'b') || self.is_char_and(b'b', b'r'))
            && self.is_char_following_following_or(b'\'', b'"')
    }

    fn is_head_of_identifier(&self) -> bool {
        self.is_letter() || self.chr0 == b'_' as u32 || self.chr0 >= 0x80
    }

    fn is_tail_of_identifier(&self) -> bool {
        self.is_head_of_identifier() || self.is_digit()
    }

    fn next_char(&mut self) {
        if self.chr0 == b'\n' as u32 {
            self.line += 1;
            self.column = 1;
        } else if self.chr0 == b'\t' as u32 {
            self.column = (((self.column.wrapping_sub(1) + TAB_SIZE) / TAB_SIZE) * TAB_SIZE) + 1;
        } else {
            self.column = self.column.wrapping_add(1);
        }

        self.chr0 = self.chr1 as u32;
        self.chr1 = self.chr2;

        let mut chr2 = self.fetch_next_byte();

        if self.chr1 == b'\r' {
            self.chr1 = b'\n';
            if chr2 == b'\n' {
                chr2 = self.fetch_next_byte();
            }
        }

        if chr2 == LEXER_EOF as u8 && self.chr1 != LEXER_EOF as u8 && self.chr1 != b'\n' {
            chr2 = b'\n';
        }

        self.chr2 = chr2;
    }

    fn fetch_next_byte(&mut self) -> u8 {
        if mpconfig::PY_FSTRINGS && self.inject_chrs_idx != 0 {
            let idx = self.inject_chrs_idx;
            let b = unsafe { *vstr::str_ptr(&self.inject_chrs).add(idx) };
            self.inject_chrs_idx += 1;
            if self.inject_chrs_idx >= vstr::len(&self.inject_chrs) {
                vstr::reset(&mut self.inject_chrs);
                self.inject_chrs_idx = 0;
            }
            return b;
        }

        let raw = (self.reader.readbyte)(self.reader.data);
        if raw == READER_EOF {
            LEXER_EOF as u8
        } else if raw == LEXER_EOF as usize {
            LEXER_INVALID_BYTE
        } else {
            raw as u8
        }
    }

    fn indent_push(&mut self, indent: u16) {
        if self.num_indent_level >= self.alloc_indent_level {
            self.indent_level = malloc::renew(
                self.indent_level,
                self.alloc_indent_level,
                self.alloc_indent_level + mpconfig::ALLOC_LEXEL_INDENT_INC as usize,
            )
            .unwrap_or_else(|| raise::raise(MpRaise::RuntimeError("lexer indent")));
            self.alloc_indent_level += mpconfig::ALLOC_LEXEL_INDENT_INC as usize;
        }
        unsafe {
            *self.indent_level.add(self.num_indent_level) = indent;
        }
        self.num_indent_level += 1;
    }

    fn indent_top(&self) -> u16 {
        unsafe { *self.indent_level.add(self.num_indent_level - 1) }
    }

    fn indent_pop(&mut self) {
        self.num_indent_level -= 1;
    }

    fn get_hex(&mut self, num_digits: usize, result: &mut u32) -> bool {
        let mut num = 0u32;
        let mut remaining = num_digits;
        while remaining != 0 {
            self.next_char();
            let c = self.cur_char() as Unichar;
            if !unichar_isxdigit(c) {
                return false;
            }
            num = (num << 4) + unichar_xdigit_value(c) as u32;
            remaining -= 1;
        }
        *result = num;
        true
    }

    fn parse_string_literal(&mut self, is_raw: bool, is_fstring: bool) {
        let mut quote_char = b'\'';
        if self.is_char(b'"') {
            quote_char = b'"';
        }
        self.next_char();

        let num_quotes = if self.is_char_and(quote_char, quote_char) {
            self.next_char();
            self.next_char();
            3usize
        } else {
            1usize
        };

        let mut n_closing = 0usize;

        if mpconfig::PY_FSTRINGS && is_fstring {
            if vstr::len(&self.fstring_args) == 0 {
                vstr::add_str(&mut self.fstring_args, ".format(");
            }
        }

        while !self.is_end()
            && (num_quotes > 1 || !self.is_char(b'\n'))
            && n_closing < num_quotes
        {
            if self.is_char(quote_char) {
                n_closing += 1;
                let c = self.cur_char();
                vstr::add_char(&mut self.vstr, c as Unichar);
            } else {
                n_closing = 0;

                if mpconfig::PY_FSTRINGS {
                    let mut fstring_loop = is_fstring && self.is_char(b'{');
                    while fstring_loop {
                        self.next_char();
                        if self.is_char(b'{') {
                            vstr::add_byte(&mut self.vstr, b'{');
                            self.next_char();
                        } else {
                            vstr::add_byte(&mut self.fstring_args, b'(');
                            let i = vstr::len(&self.fstring_args);
                            let mut nested_bracket_level = 0u32;
                            while !self.is_end()
                                && (nested_bracket_level != 0
                                    || !(self.is_char_or(b':', b'}')
                                        || (self.is_char(b'!')
                                            && self.is_char_following_or(b'r', b's')
                                            && self.is_char_following_following_or(b':', b'}'))))
                            {
                                let c = self.cur_char();
                                if c == b'[' as u32 || c == b'{' as u32 {
                                    nested_bracket_level += 1;
                                } else if c == b']' as u32 || c == b'}' as u32 {
                                    nested_bracket_level -= 1;
                                }
                                vstr::add_byte(&mut self.fstring_args, c as u8);
                                self.next_char();
                            }
                            if unsafe {
                                *vstr::str_ptr(&self.fstring_args).add(vstr::len(&self.fstring_args) - 1)
                            } == b'='
                            {
                                let arg_len = vstr::len(&self.fstring_args) - i;
                                vstr::add_strn(
                                    &mut self.vstr,
                                    unsafe {
                                        std::slice::from_raw_parts(
                                            vstr::str_ptr(&self.fstring_args).add(i),
                                            arg_len,
                                        )
                                    },
                                );
                                self.fstring_args.len -= 1;
                            }
                            if vstr::len(&self.fstring_args) == i {
                                self.tok_kind = TokenKind::MalformedFstring;
                            }
                            vstr::add_byte(&mut self.fstring_args, b')');
                            vstr::add_byte(&mut self.fstring_args, b',');
                        }
                        vstr::add_byte(&mut self.vstr, b'{');
                        fstring_loop = is_fstring && self.is_char(b'{');
                    }
                }

                if self.is_char(b'\\') {
                    self.next_char();
                    let mut c = self.cur_char() as Unichar;
                    if is_raw {
                        vstr::add_char(&mut self.vstr, b'\\' as Unichar);
                    } else {
                        match c as u8 {
                            b'\n' => {
                                self.next_char();
                                continue;
                            }
                            b'a' => c = 0x07,
                            b'b' => c = 0x08,
                            b't' => c = 0x09,
                            b'n' => c = 0x0a,
                            b'v' => c = 0x0b,
                            b'f' => c = 0x0c,
                            b'r' => c = 0x0d,
                            b'u' | b'U' if self.tok_kind == TokenKind::Bytes => {
                                vstr::add_char(&mut self.vstr, b'\\' as Unichar);
                            }
                            b'x' | b'u' | b'U' => {
                                let digits = if c as u8 == b'x' {
                                    2
                                } else if c as u8 == b'u' {
                                    4
                                } else {
                                    8
                                };
                                let mut num = 0u32;
                                if !self.get_hex(digits, &mut num) {
                                    self.tok_kind = TokenKind::Invalid;
                                }
                                c = num as Unichar;
                            }
                            b'N' => {
                                raise::raise(MpRaise::RuntimeError("unicode name escapes"));
                            }
                            _ => {
                                if (b'0'..=b'7').contains(&(c as u8)) {
                                    let mut digits = 3usize;
                                    let mut num = (c as u8 - b'0') as u32;
                                    while self.is_following_odigit() && digits != 0 {
                                        self.next_char();
                                        num = num * 8 + (self.cur_char() as u8 - b'0') as u32;
                                        digits -= 1;
                                    }
                                    c = num as Unichar;
                                } else {
                                    vstr::add_char(&mut self.vstr, b'\\' as Unichar);
                                }
                            }
                        }
                    }
                    if mpconfig::PY_BUILTINS_STR_UNICODE {
                        if (c as u32) < 0x110000 && self.tok_kind == TokenKind::String {
                            vstr::add_char(&mut self.vstr, c);
                        } else if (c as u32) < 0x100 && self.tok_kind == TokenKind::Bytes {
                            vstr::add_byte(&mut self.vstr, c as u8);
                        } else {
                            self.tok_kind = TokenKind::Invalid;
                        }
                    } else if (c as u32) < 0x100 {
                        vstr::add_byte(&mut self.vstr, c as u8);
                    } else {
                        self.tok_kind = TokenKind::Invalid;
                    }
                } else {
                    let b = self.cur_char() as u8;
                    vstr::add_byte(&mut self.vstr, b);
                }
            }
            self.next_char();
        }

        if n_closing < num_quotes {
            self.tok_kind = TokenKind::LonelyStringOpen;
        }

        vstr::cut_tail_bytes(&mut self.vstr, n_closing);
    }

    fn skip_whitespace(&mut self, stop_at_newline: bool) -> bool {
        while !self.is_end() {
            if self.is_physical_newline() {
                if stop_at_newline && self.nested_bracket_level == 0 {
                    return true;
                }
                self.next_char();
            } else if self.is_whitespace() {
                self.next_char();
            } else if self.is_char(b'#') {
                self.next_char();
                while !self.is_end() && !self.is_physical_newline() {
                    self.next_char();
                }
            } else if self.is_char_and(b'\\', b'\n') {
                self.next_char();
                self.next_char();
            } else {
                break;
            }
        }
        false
    }

    fn match_keyword(&mut self) {
        let s = unsafe {
            std::slice::from_raw_parts(vstr::str_ptr(&mut self.vstr), vstr::len(&self.vstr))
        };
        for (i, kw) in TOK_KW.iter().enumerate() {
            let cmp = s.cmp(kw.as_bytes());
            if cmp == std::cmp::Ordering::Equal {
                self.tok_kind = keyword_kind(i);
                if self.tok_kind == TokenKind::KwDebug {
                    self.tok_kind = if optimise_value() == 0 {
                        TokenKind::KwTrue
                    } else {
                        TokenKind::KwFalse
                    };
                }
                return;
            } else if cmp == std::cmp::Ordering::Less {
                break;
            }
        }
    }

    fn parse_operators(&mut self) {
        let bytes = TOK_ENC;
        let mut t_idx = 0usize;
        let mut tok_enc_index = 0usize;
        while t_idx < bytes.len() && !self.is_char(bytes[t_idx]) {
            if bytes[t_idx] == b'e' || bytes[t_idx] == b'c' {
                t_idx += 1;
            }
            tok_enc_index += 1;
            t_idx += 1;
        }

        self.next_char();

        if t_idx >= bytes.len() {
            self.tok_kind = TokenKind::Invalid;
            return;
        }

        let t = bytes[t_idx];
        if t == b'!' {
            if self.is_char(b'=') {
                self.next_char();
                self.tok_kind = TokenKind::OpNotEqual;
            } else {
                self.tok_kind = TokenKind::Invalid;
            }
        } else if t == b'.' {
            if self.is_char_and(b'.', b'.') {
                self.next_char();
                self.next_char();
                self.tok_kind = TokenKind::Ellipsis;
            } else {
                self.tok_kind = TokenKind::DelPeriod;
            }
        } else {
            let mut t_index = tok_enc_index;
            let mut ti = t_idx + 1;
            while ti < bytes.len() && (bytes[ti] == b'c' || bytes[ti] == b'e') {
                t_index += 1;
                if self.is_char(bytes[ti + 1]) {
                    self.next_char();
                    tok_enc_index = t_index;
                    if bytes[ti] == b'e' {
                        break;
                    }
                } else if bytes[ti] == b'c' {
                    break;
                }
                ti += 2;
            }
            self.tok_kind = TOK_ENC_KIND[tok_enc_index];
            match self.tok_kind {
                TokenKind::DelParenOpen | TokenKind::DelBracketOpen | TokenKind::DelBraceOpen => {
                    self.nested_bracket_level += 1;
                }
                TokenKind::DelParenClose | TokenKind::DelBracketClose | TokenKind::DelBraceClose => {
                    self.nested_bracket_level -= 1;
                }
                _ => {}
            }
        }
    }

    /// Advance to the next token (`mp_lexer_to_next`).
    pub fn to_next(&mut self) {
        vstr::reset(&mut self.vstr);

        let had_physical_newline = self.skip_whitespace(true);

        self.tok_line = self.line;
        self.tok_column = self.column;

        if self.emit_dent < 0 {
            self.tok_kind = TokenKind::Dedent;
            self.emit_dent += 1;
        } else if self.emit_dent > 0 {
            self.tok_kind = TokenKind::Indent;
            self.emit_dent -= 1;
        } else if had_physical_newline {
            self.skip_whitespace(false);
            self.tok_kind = TokenKind::Newline;

            let num_spaces = self.column - 1;
            if num_spaces as u16 == self.indent_top() {
            } else if num_spaces as u16 > self.indent_top() {
                self.indent_push(num_spaces as u16);
                self.emit_dent += 1;
            } else {
                while (num_spaces as u16) < self.indent_top() {
                    self.indent_pop();
                    self.emit_dent -= 1;
                }
                if num_spaces as u16 != self.indent_top() {
                    self.tok_kind = TokenKind::DedentMismatch;
                }
            }
        } else if self.is_end() {
            self.tok_kind = TokenKind::End;
        } else if self.is_string_or_bytes() {
            self.tok_kind = TokenKind::End;

            loop {
                let mut is_raw = false;
                let mut is_fstring = false;
                let mut kind = TokenKind::String;
                let mut n_char = 0i32;

                if self.is_char(b'u') {
                    n_char = 1;
                } else if self.is_char(b'b') {
                    kind = TokenKind::Bytes;
                    n_char = 1;
                    if self.is_char_following(b'r') {
                        is_raw = true;
                        n_char = 2;
                    }
                } else if self.is_char(b'r') {
                    is_raw = true;
                    n_char = 1;
                    if self.is_char_following(b'b') {
                        kind = TokenKind::Bytes;
                        n_char = 2;
                    } else if mpconfig::PY_FSTRINGS && self.is_char_following(b'f') {
                        is_fstring = true;
                        n_char = 2;
                    }
                } else if mpconfig::PY_FSTRINGS && self.is_char(b'f') {
                    is_fstring = true;
                    n_char = 1;
                    if self.is_char_following(b'r') {
                        is_raw = true;
                        n_char = 2;
                    }
                }

                if self.tok_kind == TokenKind::End {
                    self.tok_kind = kind;
                } else if self.tok_kind != kind {
                    break;
                }

                if n_char != 0 {
                    self.next_char();
                    if n_char == 2 {
                        self.next_char();
                    }
                }

                self.parse_string_literal(is_raw, is_fstring);

                if self.skip_whitespace(true) {
                    // stop concatenation at newline
                    break;
                }
                if !self.is_string_or_bytes() {
                    break;
                }
            }

            if mpconfig::PY_FSTRINGS && vstr::len(&self.fstring_args) != 0 {
                vstr::add_byte(&mut self.fstring_args, b')');
                if self.inject_chrs_idx == 0 {
                    let s = vstr::add_len(&mut self.inject_chrs, 3);
                    unsafe {
                        *s = self.chr0 as u8;
                        *s.add(1) = self.chr1;
                        *s.add(2) = self.chr2;
                    }
                } else {
                    debug_assert!(self.inject_chrs_idx >= 3);
                    self.inject_chrs_idx -= 3;
                }
                vstr::ins_strn(
                    &mut self.inject_chrs,
                    self.inject_chrs_idx,
                    unsafe {
                        std::slice::from_raw_parts(
                            vstr::str_ptr(&self.fstring_args),
                            vstr::len(&self.fstring_args),
                        )
                    },
                );
                vstr::reset(&mut self.fstring_args);
                self.chr0 = unsafe {
                    *vstr::str_ptr(&self.inject_chrs).add(self.inject_chrs_idx)
                } as u32;
                self.inject_chrs_idx += 1;
                self.chr1 = unsafe {
                    *vstr::str_ptr(&self.inject_chrs).add(self.inject_chrs_idx)
                };
                self.inject_chrs_idx += 1;
                self.chr2 = unsafe {
                    *vstr::str_ptr(&self.inject_chrs).add(self.inject_chrs_idx)
                };
                self.inject_chrs_idx += 1;
            }
        } else if self.is_head_of_identifier() {
            self.tok_kind = TokenKind::Name;
            let b = self.cur_char() as u8;
            vstr::add_byte(&mut self.vstr, b);
            self.next_char();
            while !self.is_end() && self.is_tail_of_identifier() {
                let b = self.cur_char() as u8;
                vstr::add_byte(&mut self.vstr, b);
                self.next_char();
            }
            self.match_keyword();
        } else if self.is_digit() || (self.is_char(b'.') && self.is_following_digit()) {
            let mut forced_integer = false;
            if self.is_char(b'.') {
                self.tok_kind = TokenKind::FloatOrImag;
            } else {
                self.tok_kind = TokenKind::Integer;
                if self.is_char(b'0') && self.is_following_base_char() {
                    forced_integer = true;
                }
            }

            let c = self.cur_char() as Unichar;
            vstr::add_char(&mut self.vstr, c);
            self.next_char();

            while !self.is_end() {
                if !forced_integer && self.is_char_or(b'e', b'E') {
                    self.tok_kind = TokenKind::FloatOrImag;
                    vstr::add_char(&mut self.vstr, b'e' as Unichar);
                    self.next_char();
                    if self.is_char(b'+') || self.is_char(b'-') {
                        let c = self.cur_char() as Unichar;
                        vstr::add_char(&mut self.vstr, c);
                        self.next_char();
                    }
                } else if self.is_letter() || self.is_digit() || self.is_char(b'.') {
                    if self.is_char_or3(b'.', b'j', b'J') {
                        self.tok_kind = TokenKind::FloatOrImag;
                    }
                    let c = self.cur_char() as Unichar;
                    vstr::add_char(&mut self.vstr, c);
                    self.next_char();
                } else if self.is_char(b'_') {
                    self.next_char();
                } else {
                    break;
                }
            }
        } else {
            self.parse_operators();
        }
    }

    /// Create lexer (`mp_lexer_new`).
    pub fn new(source_name: Qstr, reader: Reader) -> Self {
        let alloc_indent_level = mpconfig::ALLOC_LEXER_INDENT_INIT as usize;
        let indent_level = malloc::new::<u16>(alloc_indent_level)
            .unwrap_or_else(|| raise::raise(MpRaise::RuntimeError("lexer new")));

        let mut lex = Self {
            source_name,
            reader,
            chr0: 0,
            chr1: 0,
            chr2: 0,
            line: 1,
            column: (-2isize) as usize, // C: (size_t)-2 — account for 3 dummy bytes
            emit_dent: 0,
            nested_bracket_level: 0,
            alloc_indent_level,
            num_indent_level: 1,
            indent_level,
            tok_line: 0,
            tok_column: 0,
            tok_kind: TokenKind::End,
            vstr: Vstr {
                alloc: 0,
                len: 0,
                buf: std::ptr::null_mut(),
                fixed_buf: false,
            },
            inject_chrs: Vstr {
                alloc: 0,
                len: 0,
                buf: std::ptr::null_mut(),
                fixed_buf: false,
            },
            inject_chrs_idx: 0,
            fstring_args: Vstr {
                alloc: 0,
                len: 0,
                buf: std::ptr::null_mut(),
                fixed_buf: false,
            },
        };

        vstr::init(&mut lex.vstr, 32);
        if mpconfig::PY_FSTRINGS {
            vstr::init(&mut lex.inject_chrs, 0);
            vstr::init(&mut lex.fstring_args, 0);
        }

        unsafe {
            *lex.indent_level = 0;
        }

        lex.next_char();
        lex.next_char();
        lex.next_char();

        lex.to_next();

        if lex.tok_column != 1 && lex.tok_kind != TokenKind::Newline {
            lex.tok_kind = TokenKind::Indent;
        }

        lex
    }

    /// Create lexer from memory (`mp_lexer_new_from_str_len`).
    pub fn new_from_str_len(source_name: Qstr, str: &[u8], free_len: usize) -> Self {
        let mut reader = Reader {
            data: std::ptr::null_mut(),
            readbyte: reader::reader_mem_readbyte,
            close: reader::reader_mem_close,
        };
        reader::reader_new_mem(&mut reader, str.as_ptr(), str.len(), free_len);
        Self::new(source_name, reader)
    }

    /// Open file by qstr path (`mp_lexer_new_from_file`).
    pub fn new_from_file(filename: Qstr) -> Self {
        let mut reader = Reader {
            data: std::ptr::null_mut(),
            readbyte: reader::reader_mem_readbyte,
            close: reader::reader_mem_close,
        };
        reader::reader_new_file(&mut reader, filename);
        Self::new(filename, reader)
    }

    /// Token bytes as UTF-8 string (lossy).
    pub fn token_text(&self) -> String {
        if vstr::len(&self.vstr) == 0 {
            return String::new();
        }
        let bytes =
            unsafe { std::slice::from_raw_parts(vstr::str_ptr(&self.vstr), vstr::len(&self.vstr)) };
        String::from_utf8_lossy(bytes).into_owned()
    }

    fn is_error_kind(kind: TokenKind) -> bool {
        matches!(
            kind,
            TokenKind::Invalid
                | TokenKind::DedentMismatch
                | TokenKind::LonelyStringOpen
                | TokenKind::MalformedFstring
        )
    }
}

impl Drop for Lexer {
    fn drop(&mut self) {
        (self.reader.close)(self.reader.data);
        vstr::clear(&mut self.vstr);
        if mpconfig::PY_FSTRINGS {
            vstr::clear(&mut self.inject_chrs);
            vstr::clear(&mut self.fstring_args);
        }
        if !self.indent_level.is_null() {
            malloc::del(self.indent_level, self.alloc_indent_level);
        }
    }
}

fn keyword_kind(index: usize) -> TokenKind {
    match index {
        0 => TokenKind::KwFalse,
        1 => TokenKind::KwNone,
        2 => TokenKind::KwTrue,
        3 => TokenKind::KwDebug,
        4 => TokenKind::KwAnd,
        5 => TokenKind::KwAs,
        6 => TokenKind::KwAssert,
        7 => TokenKind::KwAsync,
        8 => TokenKind::KwAwait,
        9 => TokenKind::KwBreak,
        10 => TokenKind::KwClass,
        11 => TokenKind::KwContinue,
        12 => TokenKind::KwDef,
        13 => TokenKind::KwDel,
        14 => TokenKind::KwElif,
        15 => TokenKind::KwElse,
        16 => TokenKind::KwExcept,
        17 => TokenKind::KwFinally,
        18 => TokenKind::KwFor,
        19 => TokenKind::KwFrom,
        20 => TokenKind::KwGlobal,
        21 => TokenKind::KwIf,
        22 => TokenKind::KwImport,
        23 => TokenKind::KwIn,
        24 => TokenKind::KwIs,
        25 => TokenKind::KwLambda,
        26 => TokenKind::KwNonlocal,
        27 => TokenKind::KwNot,
        28 => TokenKind::KwOr,
        29 => TokenKind::KwPass,
        30 => TokenKind::KwRaise,
        31 => TokenKind::KwReturn,
        32 => TokenKind::KwTry,
        33 => TokenKind::KwWhile,
        34 => TokenKind::KwWith,
        35 => TokenKind::KwYield,
        _ => TokenKind::Name,
    }
}

fn token_from_lexer(lex: &Lexer) -> Token {
    Token {
        kind: lex.tok_kind,
        text: lex.token_text(),
    }
}

/// Tokenise a trimmed single-line arithmetic expression (smoke path helper).
pub fn tokenize_expr(src: &str) -> Result<Vec<Token>, LexError> {
    let trimmed = src.trim();
    if trimmed.is_empty() {
        return Err(LexError {
            message: "empty expression",
            line: 1,
            column: 1,
        });
    }
    let src_name = qstr::from_str("<expr>");
    let mut lex = Lexer::new_from_str_len(src_name, trimmed.as_bytes(), READER_IS_ROM);
    let mut out = Vec::new();
    loop {
        let kind = lex.tok_kind;
        if kind == TokenKind::End {
            out.push(token_from_lexer(&lex));
            break;
        }
        if matches!(
            kind,
            TokenKind::Newline | TokenKind::Indent | TokenKind::Dedent
        ) {
            lex.to_next();
            continue;
        }
        if Lexer::is_error_kind(kind) {
            return Err(LexError {
                message: "invalid token",
                line: lex.tok_line,
                column: lex.tok_column,
            });
        }
        out.push(token_from_lexer(&lex));
        lex.to_next();
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn init_host() {
        crate::gc::init();
        qstr::init();
    }

    #[test]
    fn tokenizes_one_plus_two() {
        init_host();
        let t = tokenize_expr("1 + 2").unwrap();
        assert_eq!(t[0].kind, TokenKind::Integer);
        assert_eq!(t[1].kind, TokenKind::OpPlus);
        assert_eq!(t[2].kind, TokenKind::Integer);
        assert_eq!(t[3].kind, TokenKind::End);
    }

    #[test]
    fn tokenizes_identifier() {
        init_host();
        let src_name = qstr::from_str("<t>");
        let mut lex = Lexer::new_from_str_len(src_name, b"foo", READER_IS_ROM);
        assert_eq!(lex.tok_kind, TokenKind::Name);
        assert_eq!(lex.token_text(), "foo");
    }

    #[test]
    fn tokenizes_keywords_and_strings() {
        init_host();
        let src_name = qstr::from_str("<t>");
        let mut lex = Lexer::new_from_str_len(src_name, b"def foo():\n    return 'hi'\n", READER_IS_ROM);
        assert_eq!(lex.tok_kind, TokenKind::KwDef);
        lex.to_next();
        assert_eq!(lex.tok_kind, TokenKind::Name);
        assert_eq!(lex.token_text(), "foo");
        lex.to_next();
        assert_eq!(lex.tok_kind, TokenKind::DelParenOpen);
    }

    #[test]
    fn token_kind_count() {
        assert_eq!(TokenKind::NumberOf as u8, 96);
    }
}
