//! rewrite of shared/readline/readline.c + shared/readline/readline.h
// symmetry: done

use std::cell::RefCell;
use std::sync::Mutex;

use py_rs::malloc;
use py_rs::mpconfig;
use py_rs::mphal;
use py_rs::mpprint;
use py_rs::repl;
use py_rs::unicode::unichar_isalnum;
use py_rs::unicode::unichar_isspace;
use py_rs::vstr::{self, Vstr};

pub const CHAR_CTRL_A: i32 = 1;
pub const CHAR_CTRL_B: i32 = 2;
pub const CHAR_CTRL_C: i32 = 3;
pub const CHAR_CTRL_D: i32 = 4;
pub const CHAR_CTRL_E: i32 = 5;
pub const CHAR_CTRL_F: i32 = 6;
pub const CHAR_CTRL_K: i32 = 11;
pub const CHAR_CTRL_N: i32 = 14;
pub const CHAR_CTRL_P: i32 = 16;
pub const CHAR_CTRL_U: i32 = 21;
pub const CHAR_CTRL_W: i32 = 23;

const AUTO_INDENT_ENABLED: u8 = 0x01;
const AUTO_INDENT_JUST_ADDED: u8 = 0x02;

#[derive(Copy, Clone, Eq, PartialEq)]
enum EscapeSeq {
    None,
    Esc,
    EscBracket,
    EscBracketDigit,
    EscO,
}

struct ReadlineState {
    line: *mut Vstr,
    orig_line_len: usize,
    escape_seq: EscapeSeq,
    hist_cur: i32,
    cursor_pos: usize,
    escape_seq_buf: [u8; 1],
    last_nl: u8,
    auto_indent_state: u8,
    prompt: String,
}

static HISTORY: Mutex<Vec<Option<String>>> = Mutex::new(Vec::new());
thread_local! {
    static RL: RefCell<Option<ReadlineState>> = RefCell::new(None);
}

fn history() -> std::sync::MutexGuard<'static, Vec<Option<String>>> {
    let mut hist = HISTORY.lock().expect("readline history lock");
    if hist.is_empty() {
        hist.resize(mpconfig::READLINE_HISTORY_SIZE as usize, None);
    }
    hist
}

fn with_rl<R>(f: impl FnOnce(&mut ReadlineState) -> R) -> R {
    RL.with(|cell| {
        let mut guard = cell.borrow_mut();
        if guard.is_none() {
            *guard = Some(ReadlineState {
                line: core::ptr::null_mut(),
                orig_line_len: 0,
                escape_seq: EscapeSeq::None,
                hist_cur: -1,
                cursor_pos: 0,
                escape_seq_buf: [0],
                last_nl: 0,
                auto_indent_state: 0,
                prompt: String::new(),
            });
        }
        f(guard.as_mut().unwrap())
    })
}

fn move_cursor_back(pos: usize) {
    if pos <= 4 {
        mphal::stdout_tx_strn("\u{8}\u{8}\u{8}\u{8}", pos);
    } else {
        mphal::stdout_tx_str(&format!("\x1b[{pos}D"));
    }
}

fn erase_line_from_cursor(_n_chars: usize) {
    mphal::stdout_tx_str("\x1b[K");
}

fn str_dup_maybe(s: &str) -> Option<String> {
    if s.is_empty() {
        return None;
    }
    Some(s.to_owned())
}

fn process_nl(c: i32, last_nl: &mut u8) -> bool {
    if (c == b'\r' as i32 || c == b'\n' as i32) && (*last_nl == 0 || *last_nl == c as u8) {
        *last_nl = c as u8;
        return true;
    }
    *last_nl = 0;
    false
}

fn cursor_count_word(line: &[u8], cursor_pos: usize, forward: bool) -> usize {
    let mut pos = cursor_pos;
    let mut in_word = false;
    loop {
        if !forward && pos == 0 {
            break;
        }
        if forward && pos == line.len() {
            break;
        }
        let ch = line[pos.saturating_sub(if forward { 0 } else { 1 })];
        if unichar_isalnum(ch as u32) {
            in_word = true;
        } else if in_word {
            break;
        }
        pos = if forward { pos + 1 } else { pos - 1 };
    }
    if forward {
        pos.saturating_sub(cursor_pos)
    } else {
        cursor_pos.saturating_sub(pos)
    }
}

/// `readline_init0`.
pub fn init0() {
    *history() = vec![None; mpconfig::READLINE_HISTORY_SIZE as usize];
}

/// `readline_init`.
pub fn init(line: &mut Vstr, prompt: &str) {
    with_rl(|rl| {
        rl.line = line as *mut Vstr;
        rl.orig_line_len = vstr::len(line);
        rl.escape_seq = EscapeSeq::None;
        rl.escape_seq_buf = [0];
        rl.hist_cur = -1;
        rl.cursor_pos = rl.orig_line_len;
        rl.prompt = prompt.to_owned();
        mphal::stdout_tx_str(prompt);
        if mpconfig::REPL_AUTO_INDENT && vstr::len(line) == 0 {
            rl.auto_indent_state = AUTO_INDENT_ENABLED;
        }
        auto_indent(rl);
    });
}

fn auto_indent(rl: &mut ReadlineState) {
    if !mpconfig::REPL_AUTO_INDENT || rl.auto_indent_state & AUTO_INDENT_ENABLED == 0 {
        return;
    }
    unsafe {
        let line = &mut *rl.line;
        if vstr::len(line) > 1 {
            let buf = std::slice::from_raw_parts(vstr::str_ptr(line), vstr::len(line));
            if buf.last() == Some(&b'\n') {
                let mut i = vstr::len(line);
                while i > 0 && buf[i - 1] != b'\n' {
                    i -= 1;
                }
                let mut j = i;
                while j < vstr::len(line) && buf[j] == b' ' {
                    j += 1;
                }
                if j + 1 == vstr::len(line) {
                    let mut n = (j - i) / 4;
                    if vstr::len(line) >= 2 && buf[vstr::len(line) - 2] == b':' {
                        n += 1;
                    }
                    for _ in 0..n {
                        vstr::add_strn(line, b"    ");
                        mphal::stdout_tx_strn("    ", 4);
                        rl.cursor_pos += 4;
                        rl.auto_indent_state |= AUTO_INDENT_JUST_ADDED;
                    }
                }
            }
        }
    }
}

/// `readline_note_newline`.
pub fn note_newline(prompt: &str) {
    with_rl(|rl| {
        unsafe {
            rl.orig_line_len = vstr::len(&*rl.line);
        }
        rl.cursor_pos = rl.orig_line_len;
        rl.prompt = prompt.to_owned();
        mphal::stdout_tx_str(prompt);
        auto_indent(rl);
    });
}

/// `readline_process_char`.
pub fn process_char(c: i32) -> i32 {
    with_rl(|rl| {
        let last_line_len = unsafe { vstr::len(&*rl.line) };
        let mut redraw_step_back = 0usize;
        let mut redraw_from_cursor = false;
        let mut redraw_step_forward = 0usize;

        if rl.escape_seq == EscapeSeq::None {
            if CHAR_CTRL_A <= c
                && c <= CHAR_CTRL_E
                && unsafe { vstr::len(&*rl.line) } == rl.orig_line_len
            {
                return c;
            } else if c == CHAR_CTRL_A {
                redraw_step_back = rl.cursor_pos - rl.orig_line_len;
            } else if c == CHAR_CTRL_C {
                return c;
            } else if c == CHAR_CTRL_E {
                redraw_step_forward = last_line_len - rl.cursor_pos;
            } else if process_nl(c, &mut rl.last_nl) {
                mphal::stdout_tx_str("\r\n");
                let line_text = unsafe {
                    std::str::from_utf8_unchecked(std::slice::from_raw_parts(
                        vstr::str_ptr(&*rl.line).add(rl.orig_line_len),
                        vstr::len(&*rl.line) - rl.orig_line_len,
                    ))
                };
                push_history(line_text);
                return 0;
            } else if c == 27 {
                rl.escape_seq = EscapeSeq::Esc;
            } else if c == 8 || c == 127 {
                if rl.cursor_pos > rl.orig_line_len {
                    let nspace = if mpconfig::REPL_AUTO_INDENT { 1 } else { 1 };
                    unsafe {
                        vstr::cut_out_bytes(&mut *rl.line, rl.cursor_pos - nspace, nspace);
                    }
                    redraw_step_back = nspace;
                    redraw_from_cursor = true;
                }
            } else if mpconfig::HELPER_REPL && c == 9 {
                let compl = b"    ";
                unsafe {
                    for &b in compl {
                        vstr::ins_byte(&mut *rl.line, rl.cursor_pos, b);
                        rl.cursor_pos += 1;
                    }
                }
                redraw_from_cursor = true;
                redraw_step_forward = compl.len();
            } else if (32..=126).contains(&c) {
                unsafe {
                    vstr::ins_byte(&mut *rl.line, rl.cursor_pos, c as u8);
                }
                redraw_from_cursor = true;
                redraw_step_forward = 1;
            }
        } else if rl.escape_seq == EscapeSeq::Esc {
            rl.escape_seq = match c as u8 {
                b'[' => EscapeSeq::EscBracket,
                b'O' => EscapeSeq::EscO,
                _ => EscapeSeq::None,
            };
        } else if rl.escape_seq == EscapeSeq::EscBracket {
            rl.escape_seq = EscapeSeq::None;
            match c as u8 {
                b'A' => {
                    let hist = history();
                    if rl.hist_cur + 1 < mpconfig::READLINE_HISTORY_SIZE as i32
                        && hist[(rl.hist_cur + 1) as usize].is_some()
                    {
                        rl.hist_cur += 1;
                        unsafe {
                            let line = &mut *rl.line;
                            line.len = rl.orig_line_len;
                            if let Some(ref entry) = hist[rl.hist_cur as usize] {
                                vstr::add_str(line, entry);
                            }
                        }
                        redraw_step_back = rl.cursor_pos - rl.orig_line_len;
                        redraw_from_cursor = true;
                        redraw_step_forward = last_line_len - rl.orig_line_len;
                    }
                }
                b'B' => {
                    if rl.hist_cur >= 0 {
                        rl.hist_cur -= 1;
                        unsafe {
                            let line = &mut *rl.line;
                            line.len = rl.orig_line_len;
                            if rl.hist_cur >= 0 {
                                if let Some(ref entry) = history()[rl.hist_cur as usize] {
                                    vstr::add_str(line, entry);
                                }
                            }
                        }
                        redraw_step_back = rl.cursor_pos - rl.orig_line_len;
                        redraw_from_cursor = true;
                        redraw_step_forward = unsafe { vstr::len(&*rl.line) } - rl.orig_line_len;
                    }
                }
                b'C' if rl.cursor_pos < last_line_len => redraw_step_forward = 1,
                b'D' if rl.cursor_pos > rl.orig_line_len => redraw_step_back = 1,
                b'H' => redraw_step_back = rl.cursor_pos - rl.orig_line_len,
                b'F' => redraw_step_forward = last_line_len - rl.cursor_pos,
                b'3' => {
                    if rl.cursor_pos < last_line_len {
                        unsafe {
                            vstr::cut_out_bytes(&mut *rl.line, rl.cursor_pos, 1);
                        }
                        redraw_from_cursor = true;
                    }
                }
                _ => {}
            }
        } else {
            rl.escape_seq = EscapeSeq::None;
        }

        if redraw_step_back > 0 {
            move_cursor_back(redraw_step_back);
            rl.cursor_pos -= redraw_step_back;
        }
        if redraw_from_cursor {
            let cur_len = unsafe { vstr::len(&*rl.line) };
            if cur_len < last_line_len {
                erase_line_from_cursor(last_line_len - rl.cursor_pos);
            }
            unsafe {
                let bytes = std::slice::from_raw_parts(
                    vstr::str_ptr(&*rl.line).add(rl.cursor_pos),
                    cur_len - rl.cursor_pos,
                );
                mphal::stdout_tx_strn(std::str::from_utf8(bytes).unwrap_or(""), bytes.len());
            }
            move_cursor_back(cur_len - (rl.cursor_pos + redraw_step_forward));
            rl.cursor_pos += redraw_step_forward;
        } else if redraw_step_forward > 0 {
            unsafe {
                let bytes = std::slice::from_raw_parts(
                    vstr::str_ptr(&*rl.line).add(rl.cursor_pos),
                    redraw_step_forward,
                );
                mphal::stdout_tx_strn(std::str::from_utf8(bytes).unwrap_or(""), bytes.len());
            }
            rl.cursor_pos += redraw_step_forward;
        }

        rl.auto_indent_state &= !AUTO_INDENT_JUST_ADDED;
        -1
    })
}

/// `readline`.
pub fn readline(line: &mut Vstr, prompt: &str) -> i32 {
    init(line, prompt);
    loop {
        let c = mphal::stdin_rx_chr();
        let r = process_char(c);
        if r >= 0 {
            return r;
        }
    }
}

/// `readline_push_history`.
pub fn push_history(line: &str) {
    if line.is_empty() {
        return;
    }
    let mut hist = history();
    if hist[0].as_deref() == Some(line) {
        return;
    }
    if let Some(most_recent) = str_dup_maybe(line) {
        for i in (1..mpconfig::READLINE_HISTORY_SIZE as usize).rev() {
            hist[i] = hist[i - 1].take();
        }
        hist[0] = Some(most_recent);
    }
}

/// Allocate a fresh line buffer for REPL use.
pub fn new_line(alloc: usize) -> *mut Vstr {
    malloc::new_obj::<Vstr>()
        .map(|ptr| {
            unsafe {
                vstr::init(&mut *ptr, alloc);
            }
            ptr
        })
        .unwrap_or_else(|| {
            py_rs::raise::raise(py_rs::raise::MpRaise::RuntimeError("readline line alloc"));
        })
}

/// Helper used by tab completion.
pub fn autocomplete(line: &str, cur_len: usize) -> (usize, Option<String>) {
    if mpconfig::HELPER_REPL {
        let mut out = None;
        let len = repl::repl_autocomplete(line, cur_len, &mpprint::PLAT_PRINT, &mut out);
        (len, out)
    } else {
        (0, None)
    }
}

/// Returns whether whitespace precedes the cursor (tab-to-spaces path).
pub fn cursor_preceded_by_space(line: &[u8], cursor_pos: usize) -> bool {
    cursor_pos > 0 && unichar_isspace(line[cursor_pos - 1] as u32)
}
