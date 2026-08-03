//! rewrite of ports/qemu/main.c
//! Host smoke entry for the qemu port crate. Bare-metal firmware paths live
//! under `mcu/` / `mphalport`; this binary validates the shared runtime link
//! with a thin CLI (`-c`, script file, or expression).
// symmetry: done

use py_rs::mpprint::{self, PrintKind};
use py_rs::nlr;
use py_rs::obj;
use py_rs::runtime;
use shared_rs::runtime::pyexec;

fn usage(prog: &str) {
    eprintln!(
        "usage: {prog} [-c <command> | <file.py> | <expr>]\n\
         Host smoke for ports_rs/qemu (not bare-metal firmware)."
    );
}

fn init_runtime() {
    runtime::init();
    let _ = py_rs::modbuiltins::init_builtins_module();
    py_rs::init_builtin_modules();
}

fn handle_uncaught(exc: obj::Obj) -> i32 {
    let ret = pyexec::handle_uncaught_exception(exc);
    if ret & pyexec::FORCED_EXIT != 0 {
        ret & !pyexec::FORCED_EXIT
    } else if ret != 0 {
        1
    } else {
        0
    }
}

fn eval_expr(src: &str) -> i32 {
    let mut nlr_buf = nlr::NlrBuf::default();
    match nlr::protect(&mut nlr_buf, || runtime::eval_str(src)) {
        Ok(o) => {
            if let Ok(s) = runtime::obj_to_string(o) {
                println!("{s}");
            } else {
                obj::print_helper(&mpprint::PLAT_PRINT, o, PrintKind::Str);
                println!();
            }
            0
        }
        Err(exc_val) => handle_uncaught(py_rs::vm::jump_val_to_exception(exc_val)),
    }
}

fn exec_command(cmd: &str) -> i32 {
    let bytes = cmd.as_bytes();
    let v = py_rs::vstr::Vstr {
        alloc: bytes.len(),
        len: bytes.len(),
        buf: bytes.as_ptr() as *mut u8,
        fixed_buf: true,
    };
    let ret = pyexec::vstr(&v, true);
    if ret & pyexec::FORCED_EXIT != 0 {
        ret & !pyexec::FORCED_EXIT
    } else if ret != 0 {
        1
    } else {
        0
    }
}

fn exec_file(path: &str) -> i32 {
    let ret = pyexec::file(path);
    if ret & pyexec::FORCED_EXIT != 0 {
        ret & !pyexec::FORCED_EXIT
    } else if ret != 0 {
        1
    } else {
        0
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let prog = args
        .first()
        .map(String::as_str)
        .unwrap_or("metalpython-qemu");

    init_runtime();
    println!("{}", runtime::banner_line());

    let code = match args.get(1).map(String::as_str) {
        None => {
            // Default smoke: constant fold / eval.
            eval_expr("1+2")
        }
        Some("-h") | Some("--help") => {
            usage(prog);
            0
        }
        Some("-c") => match args.get(2) {
            Some(cmd) => exec_command(cmd),
            None => {
                eprintln!("{prog}: option requires an argument: -c");
                usage(prog);
                2
            }
        },
        Some(path) if path.ends_with(".py") || std::path::Path::new(path).is_file() => {
            exec_file(path)
        }
        Some(expr) => eval_expr(expr),
    };

    if code != 0 {
        std::process::exit(code);
    }
}

/// `gc_collect` for bare-metal qemu firmware.
pub fn gc_collect() {
    py_rs::gc::collect_start();
    gc_collect_regs_and_stack();
    py_rs::gc::collect_end();
}

#[inline(never)]
fn gc_collect_regs_and_stack() {
    let regs = [0u8; 200];
    let start = regs.as_ptr() as usize;
    py_rs::mpstate::with_thread(|t| {
        let stack_top = t.stack_top as usize;
        if stack_top > start {
            let count = (stack_top - start) / core::mem::size_of::<usize>();
            // `collect_root_words` reads the *contents* of each stack slot as a
            // candidate pointer; passing slot addresses themselves (as opposed
            // to their values) would never match anything in the gc heap.
            py_rs::gc::collect_root_words(start as *const u8, count);
        }
    });
}

/// `nlr_jump_fail`
pub fn nlr_jump_fail(_val: *mut ()) -> ! {
    eprintln!("uncaught NLR");
    std::process::exit(1);
}
