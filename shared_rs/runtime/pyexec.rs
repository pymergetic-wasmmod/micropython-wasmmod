//! rewrite of shared/runtime/pyexec.c + shared/runtime/pyexec.h
// symmetry: done

use py_rs::compile;
use py_rs::lexer::Lexer;
use py_rs::mpconfig;
use py_rs::mphal;
use py_rs::nlr;
use py_rs::obj;
use py_rs::parse::{self, ParseInputKind};
use py_rs::qstr;
use py_rs::raise::{self, MpRaise};
use py_rs::reader::READER_IS_ROM;
use py_rs::runtime::{self, HandlePendingBehaviour};
use py_rs::scheduler;
use py_rs::vstr::{self, Vstr};

use crate::readline::{
    self, CHAR_CTRL_A, CHAR_CTRL_B, CHAR_CTRL_C, CHAR_CTRL_D, CHAR_CTRL_E,
};
use crate::runtime::interrupt_char;

#[repr(u8)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum PyexecModeKind {
    FriendlyRepl = 0,
    RawRepl = 1,
}

pub static mut PYEXEC_MODE_KIND: PyexecModeKind = PyexecModeKind::FriendlyRepl;
pub static mut PYEXEC_REPL_ACTIVE: u8 = 0;

pub const FORCED_EXIT: i32 = 0x100;
pub const NORMAL_EXIT: i32 = if mpconfig::PYEXEC_ENABLE_EXIT_CODE_HANDLING {
    0
} else {
    1
};
pub const UNHANDLED_EXCEPTION: i32 = if mpconfig::PYEXEC_ENABLE_EXIT_CODE_HANDLING {
    1
} else {
    0
};
pub const KEYBOARD_INTERRUPT: i32 = 128 + 2;
pub const ABORT: i32 = if mpconfig::PYEXEC_ENABLE_EXIT_CODE_HANDLING {
    128 + 9
} else {
    FORCED_EXIT
};

const EXEC_FLAG_PRINT_EOF: u32 = 1 << 0;
const EXEC_FLAG_ALLOW_DEBUGGING: u32 = 1 << 1;
const EXEC_FLAG_IS_REPL: u32 = 1 << 2;
const EXEC_FLAG_SOURCE_IS_VSTR: u32 = 1 << 4;
const EXEC_FLAG_SOURCE_IS_FILENAME: u32 = 1 << 5;
const EXEC_FLAG_NO_INTERRUPT: u32 = 1 << 7;

enum ExecSource<'a> {
    Str(&'a str),
    Vstr(&'a Vstr),
    Filename(&'a str),
}

fn parse_compile_execute(source: ExecSource<'_>, input_kind: ParseInputKind, exec_flags: u32) -> i32 {
    let mut nlr_buf = nlr::NlrBuf::default();
    let result = match nlr::protect(&mut nlr_buf, || {
        if !(exec_flags & EXEC_FLAG_NO_INTERRUPT != 0) {
            interrupt_char::set_interrupt_char(CHAR_CTRL_C);
        }

        let src_name = qstr::from_str("<stdin>");
        let mut tree = match source {
            ExecSource::Str(s) => {
                let lex = Lexer::new_from_str_len(src_name, s.as_bytes(), READER_IS_ROM);
                parse::parse(lex, input_kind)
            }
            ExecSource::Vstr(v) => {
                let bytes =
                    unsafe { std::slice::from_raw_parts(vstr::str_ptr(v), vstr::len(v)) };
                let lex = Lexer::new_from_str_len(src_name, bytes, READER_IS_ROM);
                parse::parse(lex, input_kind)
            }
            ExecSource::Filename(path) => {
                let filename = qstr::from_str(path);
                let lex = Lexer::new_from_file(filename);
                parse::parse(lex, input_kind)
            }
        };

        let module_fun = compile::compile(
            &mut tree,
            src_name,
            exec_flags & EXEC_FLAG_IS_REPL != 0,
        );

        if !mpconfig::PYEXEC_COMPILE_ONLY {
            runtime::call_function_0(module_fun);
        }
        interrupt_char::set_interrupt_char(-1);
        scheduler::handle_pending(HandlePendingBehaviour::CallbacksAndExceptions);
        NORMAL_EXIT
    }) {
        Ok(ret) => ret,
        Err(_) => {
            interrupt_char::set_interrupt_char(-1);
            scheduler::handle_pending(HandlePendingBehaviour::CallbacksAndClearExceptions);
            if exec_flags & EXEC_FLAG_PRINT_EOF != 0 {
                mphal::stdout_tx_strn("\x04", 1);
            }
            UNHANDLED_EXCEPTION
        }
    };

    if exec_flags & EXEC_FLAG_PRINT_EOF != 0 {
        mphal::stdout_tx_strn("\x04", 1);
    }
    result
}

/// `pyexec_vstr`.
pub fn vstr(str: &Vstr, allow_keyboard_interrupt: bool) -> i32 {
    let exec_flags = if allow_keyboard_interrupt {
        EXEC_FLAG_SOURCE_IS_VSTR
    } else {
        EXEC_FLAG_SOURCE_IS_VSTR | EXEC_FLAG_NO_INTERRUPT
    };
    parse_compile_execute(ExecSource::Vstr(str), ParseInputKind::FileInput, exec_flags)
}

/// `pyexec_file`.
pub fn file(filename: &str) -> i32 {
    parse_compile_execute(
        ExecSource::Filename(filename),
        ParseInputKind::FileInput,
        EXEC_FLAG_SOURCE_IS_FILENAME,
    )
}

/// `pyexec_file_if_exists`.
pub fn file_if_exists(filename: &str) -> i32 {
    if py_rs::builtinimport::import_stat(filename) != py_rs::builtinimport::ImportStat::File {
        return 1;
    }
    file(filename)
}

fn stdio_mode_raw() {}
fn stdio_mode_orig() {}

/// `pyexec_raw_repl`.
pub fn raw_repl() -> i32 {
    if !mpconfig::ENABLE_COMPILER {
        raise::raise(MpRaise::RuntimeError("compiler disabled"));
    }
    stdio_mode_raw();
    mphal::stdout_tx_str("raw REPL; CTRL-B to exit\r\n");
    loop {
        let mut line = vstr::new(32);
        mphal::stdout_tx_str(">");
        loop {
            let c = mphal::stdin_rx_chr();
            unsafe {
                let v = &mut *line;
                match c {
                    x if x == CHAR_CTRL_D => break,
                    x if x == CHAR_CTRL_C => vstr::reset(v),
                    x if x == CHAR_CTRL_B => {
                        mphal::stdout_tx_str("\r\n");
                        vstr::clear(v);
                        vstr::free(line);
                        stdio_mode_orig();
                        return 0;
                    }
                    _ => vstr::add_byte(v, c as u8),
                }
            }
        }
        mphal::stdout_tx_str("OK");
        unsafe {
            if vstr::len(&*line) == 0 {
                mphal::stdout_tx_str("\r\n");
                vstr::clear(&mut *line);
                vstr::free(line);
                stdio_mode_orig();
                return FORCED_EXIT;
            }
        }
        stdio_mode_orig();
        let ret = vstr(
            unsafe { &*line },
            true,
        ) | if mpconfig::PYEXEC_ENABLE_EXIT_CODE_HANDLING { 0 } else { 0 };
        vstr::free(line);
        if ret & FORCED_EXIT != 0 {
            return ret;
        }
        stdio_mode_raw();
    }
}

/// `pyexec_friendly_repl`.
pub fn friendly_repl() -> i32 {
    if !mpconfig::ENABLE_COMPILER {
        raise::raise(MpRaise::RuntimeError("compiler disabled"));
    }
    stdio_mode_raw();
    mphal::stdout_tx_str(&format!("{}\r\n", runtime::banner_line()));
    loop {
        let mut line = vstr::new(32);
        let ps1 = repl_get_ps1();
        let ret = readline::readline(unsafe { &mut *line }, &ps1);
        let mut parse_input_kind = ParseInputKind::SingleInput;
        match ret {
            x if x == CHAR_CTRL_A => {
                mphal::stdout_tx_str("\r\n");
                vstr::clear(unsafe { &mut *line });
                vstr::free(line);
                unsafe {
                    PYEXEC_MODE_KIND = PyexecModeKind::RawRepl;
                }
                stdio_mode_orig();
                return 0;
            }
            x if x == CHAR_CTRL_D => {
                mphal::stdout_tx_str("\r\n");
                vstr::clear(unsafe { &mut *line });
                vstr::free(line);
                stdio_mode_orig();
                return FORCED_EXIT;
            }
            x if x == CHAR_CTRL_E => {
                mphal::stdout_tx_str(
                    "\r\npaste mode; Ctrl-C to cancel, Ctrl-D to finish\r\n=== ",
                );
                vstr::reset(unsafe { &mut *line });
                loop {
                    let c = mphal::stdin_rx_chr();
                    if c == CHAR_CTRL_C {
                        mphal::stdout_tx_str("\r\n");
                        break;
                    } else if c == CHAR_CTRL_D {
                        mphal::stdout_tx_str("\r\n");
                        break;
                    } else {
                        unsafe {
                            vstr::add_byte(&mut *line, c as u8);
                        }
                        if c == b'\r' as i32 {
                            mphal::stdout_tx_str("\r\n=== ");
                        } else {
                            mphal::stdout_tx_strn(&format!("{}", c as u8), 1);
                        }
                    }
                }
                parse_input_kind = ParseInputKind::FileInput;
            }
            0 => {
                let input = unsafe {
                    std::str::from_utf8_unchecked(std::slice::from_raw_parts(
                        vstr::str_ptr(&*line),
                        vstr::len(&*line),
                    ))
                };
                while py_rs::repl::repl_continue_with_input(input) {
                    unsafe {
                        vstr::add_byte(&mut *line, b'\n');
                    }
                    let ps2 = repl_get_ps2();
                    if readline::readline(unsafe { &mut *line }, &ps2) == CHAR_CTRL_C {
                        mphal::stdout_tx_str("\r\n");
                        vstr::free(line);
                        continue;
                    }
                }
            }
            _ => {
                vstr::free(line);
                continue;
            }
        }
        if unsafe { vstr::len(&*line) } == 0 {
            vstr::free(line);
            continue;
        }
        stdio_mode_orig();
        let exec_flags = EXEC_FLAG_ALLOW_DEBUGGING | EXEC_FLAG_IS_REPL | EXEC_FLAG_SOURCE_IS_VSTR;
        let ret = parse_compile_execute(
            ExecSource::Vstr(unsafe { &*line }),
            parse_input_kind,
            exec_flags,
        );
        vstr::free(line);
        if ret & FORCED_EXIT != 0 {
            return ret;
        }
        stdio_mode_raw();
    }
}

fn repl_get_ps1() -> String {
    py_rs::repl::repl_get_psx(0).unwrap_or_else(|| ">>> ".to_owned())
}

fn repl_get_ps2() -> String {
    py_rs::repl::repl_get_psx(1).unwrap_or_else(|| "... ".to_owned())
}

/// `pyexec_event_repl_init`.
pub fn event_repl_init() {
    readline::init0();
}

/// `pyexec_event_repl_process_char`.
pub fn event_repl_process_char(c: i32) -> i32 {
    if !mpconfig::REPL_EVENT_DRIVEN {
        return 0;
    }
    unsafe {
        PYEXEC_REPL_ACTIVE = 1;
    }
    let res = readline::process_char(c);
    unsafe {
        PYEXEC_REPL_ACTIVE = 0;
    }
    res
}

/// `pyexec_frozen_module`.
pub fn frozen_module(_name: &str, _allow_keyboard_interrupt: bool) -> i32 {
    if !mpconfig::MODULE_FROZEN {
        mphal::stdout_tx_str("could not find module\n");
        UNHANDLED_EXCEPTION
    } else {
        UNHANDLED_EXCEPTION
    }
}

/// Current REPL mode.
pub fn mode_kind() -> PyexecModeKind {
    unsafe { PYEXEC_MODE_KIND }
}

pub fn set_mode_kind(mode: PyexecModeKind) {
    unsafe {
        PYEXEC_MODE_KIND = mode;
    }
}
