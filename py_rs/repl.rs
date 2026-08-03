//! rewrite of py/repl.c + py/repl.h
// symmetry: done

use std::sync::OnceLock;

use crate::bc::ModuleContext;
use crate::malloc;
use crate::mpconfig;
use crate::mpprint::{self, Print};
use crate::mpstate;
use crate::obj::{self, Obj, ObjBase, ObjType};
use crate::objdict;
use crate::objmodule::{self, type_module};
use crate::qstr::{self, Qstr};
use crate::runtime;
use crate::unicode::{unichar_isalpha, unichar_isdigit, unichar_isident};

/// `mp_repl_get_psx`
pub fn repl_get_psx(entry: usize) -> Option<String> {
    if !mpconfig::HELPER_REPL || !mpconfig::PY_SYS_PS1_PS2 {
        return None;
    }
    mpstate::with_vm(|vm| {
        let o = match entry {
            0 => vm.sys_ps1,
            1 => vm.sys_ps2,
            _ => return None,
        };
        if o == obj::OBJ_NULL || o == obj::CONST_NONE {
            return None;
        }
        if obj::is_str(o) {
            Some(crate::objstr::str_get_str(o).to_string())
        } else {
            None
        }
    })
}

fn str_startswith_word(str: &str, head: &str) -> bool {
    let str_bytes = str.as_bytes();
    let head_bytes = head.as_bytes();
    for (i, &hb) in head_bytes.iter().enumerate() {
        if str_bytes.get(i) != Some(&hb) {
            return false;
        }
    }
    head_bytes.len() == str.len()
        || str_bytes
            .get(head_bytes.len())
            .map(|&c| !unichar_isident(c as u32))
            .unwrap_or(true)
}

/// `mp_repl_continue_with_input`
pub fn repl_continue_with_input(input: &str) -> bool {
    if !mpconfig::HELPER_REPL {
        return false;
    }

    if input.is_empty() {
        return false;
    }

    let mut starts_with_compound_keyword = input.starts_with('@')
        || str_startswith_word(input, "if")
        || str_startswith_word(input, "while")
        || str_startswith_word(input, "for")
        || str_startswith_word(input, "try")
        || str_startswith_word(input, "with")
        || str_startswith_word(input, "def")
        || str_startswith_word(input, "class");

    if mpconfig::PY_ASYNC_AWAIT {
        starts_with_compound_keyword =
            starts_with_compound_keyword || str_startswith_word(input, "async");
    }

    const Q_NONE: i32 = 0;
    const Q_1_SINGLE: i32 = 1;
    const Q_1_DOUBLE: i32 = 2;
    const Q_3_SINGLE: i32 = 3;
    const Q_3_DOUBLE: i32 = 4;

    let mut n_paren = 0i32;
    let mut n_brack = 0i32;
    let mut n_brace = 0i32;
    let mut in_quote = Q_NONE;
    let bytes = input.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        let ch = bytes[i];
        if ch == b'\'' {
            if (in_quote == Q_NONE || in_quote == Q_3_SINGLE)
                && bytes.get(i + 1) == Some(&b'\'')
                && bytes.get(i + 2) == Some(&b'\'')
            {
                i += 2;
                in_quote = Q_3_SINGLE - in_quote;
            } else if in_quote == Q_NONE || in_quote == Q_1_SINGLE {
                in_quote = Q_1_SINGLE - in_quote;
            }
        } else if ch == b'"' {
            if (in_quote == Q_NONE || in_quote == Q_3_DOUBLE)
                && bytes.get(i + 1) == Some(&b'"')
                && bytes.get(i + 2) == Some(&b'"')
            {
                i += 2;
                in_quote = Q_3_DOUBLE - in_quote;
            } else if in_quote == Q_NONE || in_quote == Q_1_DOUBLE {
                in_quote = Q_1_DOUBLE - in_quote;
            }
        } else if ch == b'\\'
            && matches!(bytes.get(i + 1), Some(b'\'') | Some(b'"') | Some(b'\\'))
            && in_quote != Q_NONE
        {
            i += 1;
        } else if in_quote == Q_NONE {
            match ch {
                b'(' => n_paren += 1,
                b')' => n_paren -= 1,
                b'[' => n_brack += 1,
                b']' => n_brack -= 1,
                b'{' => n_brace += 1,
                b'}' => n_brace -= 1,
                _ => {}
            }
        }
        i += 1;
    }

    if in_quote == Q_3_SINGLE || in_quote == Q_3_DOUBLE {
        return true;
    }

    if (n_paren > 0 || n_brack > 0 || n_brace > 0) && in_quote == Q_NONE {
        return true;
    }

    if bytes.last() == Some(&b'\\') {
        return true;
    }

    if starts_with_compound_keyword && bytes.last() != Some(&b'\n') {
        return true;
    }

    false
}

fn test_qstr(obj: Obj, name: Qstr) -> bool {
    if obj != obj::OBJ_NULL {
        let mut dest = [obj::OBJ_NULL, obj::OBJ_NULL];
        runtime::load_method_protected(obj, name, &mut dest, true);
        return dest[0] != obj::OBJ_NULL;
    }

    let key = obj::new_qstr(name);
    if objmodule::module_get_builtin(name, false) != obj::OBJ_NULL {
        return true;
    }
    let _ = key;
    false
}

fn find_completions(
    s_start: &str,
    obj: Obj,
    match_len: &mut usize,
    q_first: &mut Qstr,
    q_last: &mut Qstr,
) -> Option<String> {
    let s_len = s_start.len();
    let mut match_str: Option<String> = None;
    *match_len = 0;
    *q_first = 0;
    *q_last = 0;
    let nqstr = qstr::total();
    for q in 1..nqstr {
        let Some((d_bytes, d_len)) = qstr::qstr_data(q) else {
            continue;
        };
        let d_str = std::str::from_utf8(&d_bytes[..d_len]).unwrap_or("");
        if s_len == 0 && d_str.starts_with('_') {
            continue;
        }
        if s_len <= d_len && d_str.as_bytes().starts_with(s_start.as_bytes()) {
            if test_qstr(obj, q) {
                if match_str.is_none() {
                    match_str = Some(d_str.to_string());
                    *match_len = d_len;
                } else if let Some(ref prev) = match_str {
                    for j in s_len..=(*match_len).min(d_len) {
                        if prev.as_bytes().get(j) != d_str.as_bytes().get(j) {
                            *match_len = j;
                            break;
                        }
                    }
                }
                if *q_first == 0 {
                    *q_first = q;
                }
                *q_last = q;
            }
        }
    }
    match_str
}

fn print_completions(print: &Print, s_start: &str, obj: Obj, q_first: Qstr, q_last: Qstr) {
    const WORD_SLOT_LEN: i32 = 16;
    const MAX_LINE_LEN: i32 = 4 * WORD_SLOT_LEN;
    let s_len = s_start.len();

    let mut line_len = MAX_LINE_LEN;
    for q in q_first..=q_last {
        let Some((d_bytes, d_len)) = qstr::qstr_data(q) else {
            continue;
        };
        let d_str = std::str::from_utf8(&d_bytes[..d_len]).unwrap_or("");
        if s_len == 0 && d_str.starts_with('_') {
            continue;
        }
        if s_len <= d_len && d_str.as_bytes().starts_with(s_start.as_bytes()) && test_qstr(obj, q) {
            let mut gap = (line_len + WORD_SLOT_LEN - 1) / WORD_SLOT_LEN * WORD_SLOT_LEN - line_len;
            if gap < 2 {
                gap += WORD_SLOT_LEN;
            }
            if line_len + gap + d_len as i32 <= MAX_LINE_LEN {
                for _ in 0..gap {
                    mpprint::print_str(print, " ");
                }
                mpprint::print_str(print, d_str);
                line_len += gap + d_len as i32;
            } else {
                let _ = mpprint::printf(print, "\n%s", std::iter::once(mpprint::VaArg::Str(d_str)));
                line_len = d_len as i32;
            }
        }
    }
    mpprint::print_str(print, "\n");
}

fn repl_main_module() -> Obj {
    static MAIN: OnceLock<Obj> = OnceLock::new();
    *MAIN.get_or_init(|| {
        let ctx = malloc::new_obj::<ModuleContext>().expect("main module alloc");
        unsafe {
            (*ctx).module.base = ObjBase {
                type_: type_module() as *const ObjType,
            };
            (*ctx).module.globals = objdict::dict_ptr(mpstate::with_vm(|vm| vm.dict_main));
            (*ctx).constants = Default::default();
            obj::from_ptr(ctx as *const ModuleContext as *const ())
        }
    })
}

/// `mp_repl_autocomplete`
pub fn repl_autocomplete(
    input: &str,
    len: usize,
    print: &Print,
    compl_str: &mut Option<String>,
) -> usize {
    if !mpconfig::HELPER_REPL {
        return 0;
    }

    let org_str = &input[..len.min(input.len())];
    let top = org_str.len();
    let mut start = 0usize;
    for i in (0..top).rev() {
        let ch = org_str.as_bytes()[i];
        if !(unichar_isalpha(ch as u32) || unichar_isdigit(ch as u32) || ch == b'_' || ch == b'.') {
            start = i + 1;
            break;
        }
    }

    let mut obj = repl_main_module();
    let mut cursor = start;
    let mut s_start = &org_str[cursor..];
    let mut s_len = 0usize;

    loop {
        s_start = &org_str[cursor..];
        if let Some(dot) = s_start.find('.') {
            s_len = dot;
            cursor += dot + 1;
        } else {
            s_len = s_start.len();
            break;
        }

        let word = &s_start[..s_len];
        let q = qstr::find_strn(word.as_bytes());
        if q == qstr::QSTR_NULL {
            return 0;
        }
        let mut dest = [obj::OBJ_NULL, obj::OBJ_NULL];
        runtime::load_method_protected(obj, q, &mut dest, true);
        obj = dest[0];
        if obj == obj::OBJ_NULL {
            return 0;
        }
    }

    s_start = &org_str[cursor..][..s_len];

    const IMPORT_STR: &str = "import ";
    if len >= 7 && org_str.starts_with(IMPORT_STR) {
        obj = obj::OBJ_NULL;
    }

    let mut match_len = 0usize;
    let mut q_first = 0;
    let mut q_last = 0;
    let match_str = find_completions(s_start, obj, &mut match_len, &mut q_first, &mut q_last);

    if q_first == 0 {
        if start == 0 && !s_start.is_empty() && s_start.len() < IMPORT_STR.len() {
            if IMPORT_STR.starts_with(s_start) {
                *compl_str = Some(IMPORT_STR[s_start.len()..].to_string());
                return IMPORT_STR.len() - s_start.len();
            }
        }
        return 0;
    }

    if q_first == q_last || match_len > s_start.len() {
        if let Some(ref m) = match_str {
            if m.len() >= s_start.len() {
                *compl_str = Some(m[s_start.len()..].to_string());
                return match_len - s_start.len();
            }
        }
        return 0;
    }

    print_completions(print, s_start, obj, q_first, q_last);
    usize::MAX
}
