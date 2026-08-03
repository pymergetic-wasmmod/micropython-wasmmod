//! rewrite of mpy-cross/main.c
// symmetry: done
//! Minimal CLI: compile `.py` to `.mpy` (bytecode only).
//! Unsupported / partial upstream flags (accepted where noted, otherwise ignored):
//! `-march`, `-march-flags`, `-msmall-int-bits`, `-v`, `-s` (source name only),
//! source-lines toggles, native/viper emit (warn + set if supported).

use py_rs::bc::ModuleContext;
use py_rs::compile;
use py_rs::emitglue::{CompiledModule, EMIT_OPT_BYTECODE, EMIT_OPT_NATIVE_PYTHON};
use py_rs::emitnative::EMIT_OPT_VIPER;
use py_rs::lexer;
use py_rs::malloc;
use py_rs::mpconfig;
use py_rs::mpprint::Print;
use py_rs::mpstate;
use py_rs::nlr::{self, NlrBuf};
use py_rs::obj;
use py_rs::parse::{self, ParseInputKind};
use py_rs::persistentcode::{self, MPY_SUB_VERSION, MPY_VERSION};
use py_rs::qstr;
use py_rs::reader::{self, Reader};
use py_rs::runtime;

extern "C" fn stderr_print_strn(_env: *mut (), str: *const u8, len: usize) {
    use std::io::Write;
    let slice = unsafe { std::slice::from_raw_parts(str, len) };
    let _ = std::io::stderr().write_all(slice);
}

static STDERR_PRINT: Print = Print {
    data: core::ptr::null_mut(),
    print_strn: Some(stderr_print_strn),
};

extern "C" fn stdout_print_strn(_env: *mut (), str: *const u8, len: usize) {
    use std::io::Write;
    let slice = unsafe { std::slice::from_raw_parts(str, len) };
    let _ = std::io::stdout().write_all(slice);
}

static STDOUT_PRINT: Print = Print {
    data: core::ptr::null_mut(),
    print_strn: Some(stdout_print_strn),
};

/// Where to write the compiled `.mpy` (`mp_raw_code_save` vs `mp_raw_code_save_file`).
enum OutputTarget {
    File(String),
    Stdout,
}

fn default_heap_size() -> usize {
    1024 * 1024 * (std::mem::size_of::<usize>() / 4)
}

fn usage(prog: &str) -> ! {
    eprintln!(
        "usage: {prog} [<opts>] [-X <implopt>] [--] <input filename>\n\
         Options:\n\
         --version : show version information\n\
         -o FILE   : output file for compiled bytecode (default: input filename with .mpy\n\
                     extension, or stdout if input is stdin); use '-' for stdout\n\
         -s FILE   : source filename to embed in the compiled bytecode (default: input file)\n\
         -O[N]     : apply bytecode optimizations of level N\n\
         -march=ARCH : select native arch for emit=native/viper (x86,x64,armv6m,\n\
                       armv7m,rv32imc,host,…); bytecode still emitted when arch unsupported\n\
         \n\
         Implementation specific options (-X):\n\
           emit={{bytecode,native,viper}} -- set the default code emitter\n\
           heapsize=<n>[w][K|M] -- set the heap size for the GC (default {})\n\
         \n\
         Use '-' as <input filename> to read the source from stdin.\n",
        default_heap_size()
    );
    std::process::exit(1);
}

fn parse_heapsize(value: &str) -> Result<usize, ()> {
    let rest = value.strip_prefix("heapsize=").ok_or(())?;
    if rest.is_empty() {
        return Err(());
    }
    let mut end = 0usize;
    while end < rest.len() && rest.as_bytes()[end].is_ascii_digit() {
        end += 1;
    }
    if end == 0 {
        return Err(());
    }
    let mut heap_size: usize = rest[..end].parse().map_err(|_| ())?;
    let mut suffix = &rest[end..];
    let mut word_adjust = false;
    if let Some(tail) = suffix.strip_prefix(['w', 'W']) {
        word_adjust = true;
        suffix = tail;
    }
    if let Some(tail) = suffix.strip_prefix(['k', 'K']) {
        heap_size = heap_size.saturating_mul(1024);
        suffix = tail;
    } else if let Some(tail) = suffix.strip_prefix(['m', 'M']) {
        heap_size = heap_size.saturating_mul(1024 * 1024);
        suffix = tail;
    }
    if !suffix.is_empty() {
        return Err(());
    }
    if word_adjust {
        heap_size = heap_size * mpconfig::BYTES_PER_OBJ_WORD as usize / 4;
    }
    if heap_size < 700 {
        return Err(());
    }
    Ok(heap_size)
}

fn parse_emit_opt(value: &str) -> Result<u16, ()> {
    match value {
        "emit=bytecode" => Ok(EMIT_OPT_BYTECODE),
        "emit=native" if mpconfig::ENABLE_NATIVE_CODE => Ok(EMIT_OPT_NATIVE_PYTHON),
        "emit=viper" if mpconfig::ENABLE_NATIVE_CODE => Ok(EMIT_OPT_VIPER),
        _ => Err(()),
    }
}

fn parse_optimise_flag(arg: &str) -> Option<usize> {
    if !arg.starts_with("-O") {
        return None;
    }
    let tail = &arg[2..];
    if tail.is_empty() {
        Some(1)
    } else if tail.chars().all(|c| c == 'O') {
        Some(tail.len())
    } else if tail.len() == 1 && tail.as_bytes()[0].is_ascii_digit() {
        Some((tail.as_bytes()[0] & 0xf) as usize)
    } else {
        None
    }
}

struct PreInitOptions {
    heap_size: Option<usize>,
    emit_opt: Option<u16>,
}

fn pre_process_options(args: &[String], prog: &str) -> PreInitOptions {
    let mut opts = PreInitOptions {
        heap_size: None,
        emit_opt: None,
    };
    let mut i = 1;
    while i < args.len() {
        let arg = args[i].as_str();
        if arg == "--" {
            break;
        }
        if !arg.starts_with('-') {
            break;
        }
        if arg == "-X" {
            if i + 1 >= args.len() {
                usage(prog);
            }
            let impl_opt = args[i + 1].as_str();
            if impl_opt.starts_with("heapsize=") {
                match parse_heapsize(impl_opt) {
                    Ok(size) => opts.heap_size = Some(size),
                    Err(()) => usage(prog),
                }
            } else if impl_opt.starts_with("emit=") {
                match parse_emit_opt(impl_opt) {
                    Ok(emit) => {
                        if emit != EMIT_OPT_BYTECODE {
                            eprintln!(
                                "mpy-cross: warning: {impl_opt} set (cross is primarily bytecode)"
                            );
                        }
                        opts.emit_opt = Some(emit);
                    }
                    Err(()) => usage(prog),
                }
            } else {
                usage(prog);
            }
            i += 2;
        } else {
            i += 1;
        }
    }
    opts
}

fn default_output(input: &str) -> String {
    if let Some(stem) = input.strip_suffix(".py") {
        format!("{stem}.mpy")
    } else {
        format!("{input}.mpy")
    }
}

/// STDIN_FILENO; matches upstream's `mp_lexer_new_from_fd(..., STDIN_FILENO, false)`.
const STDIN_FILENO: i32 = 0;

fn lexer_for_input(input: &str, source_name: qstr::Qstr) -> lexer::Lexer {
    if input == "-" {
        let mut r = Reader {
            data: core::ptr::null_mut(),
            readbyte: |_| py_rs::reader::READER_EOF,
            close: |_| {},
        };
        reader::reader_new_file_from_fd(&mut r, STDIN_FILENO, false);
        lexer::Lexer::new(source_name, r)
    } else {
        lexer::Lexer::new_from_file(qstr::from_str(input))
    }
}

fn compile_and_save(input: &str, output: &OutputTarget, source_name: qstr::Qstr) -> Result<(), ()> {
    let lex = lexer_for_input(input, source_name);
    let mut parse_tree = parse::parse(lex, ParseInputKind::FileInput);

    let ctx = malloc::new_obj::<ModuleContext>().expect("module context");
    let mut cm = CompiledModule {
        context: ctx,
        rc: core::ptr::null(),
        has_native: false,
        n_qstr: 0,
        n_obj: 0,
        arch_flags: 0,
    };

    compile::compile_to_raw_code(&mut parse_tree, source_name, false, &mut cm);
    match output {
        OutputTarget::File(path) => persistentcode::raw_code_save_file(&cm, qstr::from_str(path)),
        OutputTarget::Stdout => persistentcode::raw_code_save(&cm, &STDOUT_PRINT),
    }
    Ok(())
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let prog = args.first().map(String::as_str).unwrap_or("mpy-cross");

    let pre = pre_process_options(&args, prog);

    let mut input_file: Option<String> = None;
    let mut output_file: Option<String> = None;
    let mut source_file: Option<String> = None;
    let mut option_parsing = true;

    let mut i = 1;
    while i < args.len() {
        let arg = &args[i];
        // A lone "-" is not an option switch: it names stdin as the input file.
        if option_parsing && arg.starts_with('-') && arg != "--" && arg != "-" {
            if arg == "--version" {
                println!(
                    "{} {} ; mpy-cross emitting mpy v{}.{}",
                    mpconfig::IMPLEMENTATION_NAME,
                    mpconfig::VERSION_STRING,
                    MPY_VERSION,
                    MPY_SUB_VERSION,
                );
                return;
            } else if arg == "-o" {
                i += 1;
                if i >= args.len() {
                    usage(prog);
                }
                output_file = Some(args[i].clone());
            } else if arg == "-s" {
                i += 1;
                if i >= args.len() {
                    usage(prog);
                }
                source_file = Some(args[i].clone());
            } else if let Some(tail) = arg.strip_prefix("-msmall-int-bits=") {
                match tail.parse::<u8>() {
                    Ok(bits) if (1..=63).contains(&bits) => {
                        persistentcode::set_save_small_int_bits(bits);
                    }
                    _ => usage(prog),
                }
            } else if let Some(arch_name) = arg.strip_prefix("-march=") {
                match persistentcode::parse_march(arch_name) {
                    Some(arch) => persistentcode::set_cross_native_arch(arch),
                    None => {
                        eprintln!("mpy-cross: unknown architecture '{arch_name}'");
                        std::process::exit(1);
                    }
                }
            } else if arg.starts_with("-march-flags=") {
                eprintln!("mpy-cross: warning: {arg} partially supported (stored for rv32 later)");
            } else if arg == "-X" {
                if i + 1 >= args.len() {
                    usage(prog);
                }
                i += 1;
            } else if arg == "-v" {
                // accepted, no-op for now
            } else if parse_optimise_flag(arg).is_some() {
                // applied after runtime init
            } else if arg == "--" {
                option_parsing = false;
            } else if arg.starts_with('-') {
                usage(prog);
            } else {
                option_parsing = false;
                continue;
            }
        } else {
            if input_file.is_some() {
                eprintln!("mpy-cross: multiple input files");
                std::process::exit(1);
            }
            input_file = Some(arg.clone());
        }
        i += 1;
    }

    let input = input_file.unwrap_or_else(|| usage(prog));
    // Matches upstream: output goes to stdout if explicitly `-o -`, or if no
    // `-o` was given and the input itself is stdin (`-`).
    let output = match output_file.as_deref() {
        Some("-") => OutputTarget::Stdout,
        Some(path) => OutputTarget::File(path.to_string()),
        None if input == "-" => OutputTarget::Stdout,
        None => OutputTarget::File(default_output(&input)),
    };
    let source_name_str =
        source_file
            .as_deref()
            .unwrap_or(if input == "-" { "<stdin>" } else { &input });
    let source_name = qstr::from_str(source_name_str);

    if let Some(size) = pre.heap_size {
        py_rs::gc::init_with_size(size);
    }
    persistentcode::set_save_small_int_bits(31);
    runtime::init();

    if let Some(emit) = pre.emit_opt {
        mpstate::with_vm(|vm| vm.default_emit_opt = emit as u8);
    }

    for arg in args.iter().skip(1) {
        if let Some(level) = parse_optimise_flag(arg) {
            mpstate::with_vm(|vm| vm.mp_optimise_value = level);
        }
    }

    let mut nlr_buf = NlrBuf::default();
    match nlr::protect(&mut nlr_buf, || {
        compile_and_save(&input, &output, source_name)
    }) {
        Ok(Ok(())) => {}
        Ok(Err(())) => std::process::exit(1),
        Err(exc) => {
            obj::print_exception(&STDERR_PRINT, obj::Obj(exc));
            std::process::exit(1);
        }
    }
}
