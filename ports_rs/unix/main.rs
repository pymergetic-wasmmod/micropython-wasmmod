//! rewrite of ports/unix/main.c
// symmetry: done

use std::io::{self, IsTerminal, Read};

use py_rs::emitglue::{EMIT_OPT_BYTECODE, EMIT_OPT_NATIVE_PYTHON};
use py_rs::emitnative::EMIT_OPT_VIPER;
use py_rs::frozenmod;
use py_rs::modsys;
use py_rs::mpconfig;
use py_rs::mpprint::{self, PrintKind};
use py_rs::mpstate;
use py_rs::nlr;
use py_rs::obj;
use py_rs::objstr;
use py_rs::qstr;
use py_rs::runtime;
use py_rs::scheduler;
use shared_rs::runtime::interrupt_char;
use shared_rs::runtime::pyexec;

fn default_heap_size() -> usize {
    1024 * 1024 * (std::mem::size_of::<usize>() / 4)
}

fn usage(prog: &str) {
    println!(
        "usage: {prog} [<opts>] [-X <implopt>] [-c <command> | -m <module> | <filename> | <expr>] [<args>...]\n\
         Options:\n\
         --version : show version information\n\
         -h|--help : print this help message\n\
         -i : enable inspection via REPL after running command/module/file\n\
         -O[N] : apply bytecode optimizations of level N\n\
         \n\
         Implementation specific options (-X):\n\
           compile-only                 -- parse and compile only\n\
           emit={{bytecode,native,viper}} -- set the default code emitter\n\
           heapsize=<n>[w][K|M] -- set the heap size for the GC (default {})\n",
        default_heap_size()
    );
}

fn version_line() -> String {
    format!(
        "{} {}; {}",
        mpconfig::IMPLEMENTATION_NAME,
        mpconfig::VERSION_STRING,
        mpconfig::PY_SYS_PLATFORM,
    )
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
    compile_only: bool,
    emit_opt: Option<u16>,
}

enum PreProcessResult {
    Exit(i32),
    Continue(PreInitOptions),
}

fn invalid_args(prog: &str) -> PreProcessResult {
    eprintln!("{prog}: invalid command line arguments. Use -h option for help.");
    PreProcessResult::Exit(1)
}

/// Process `-X` and early-exit flags before GC / `runtime::init()` (`pre_process_options` in C).
fn pre_process_options(args: &[String], prog: &str) -> PreProcessResult {
    let mut opts = PreInitOptions {
        heap_size: None,
        compile_only: false,
        emit_opt: None,
    };
    let mut i = 1;
    while i < args.len() {
        let arg = args[i].as_str();
        if arg == "-c" || arg == "-m" {
            break;
        }
        if !arg.starts_with('-') {
            break;
        }
        match arg {
            "-h" | "--help" => {
                usage(prog);
                return PreProcessResult::Exit(0);
            }
            "--version" => {
                println!("{}", version_line());
                return PreProcessResult::Exit(0);
            }
            "-X" => {
                if i + 1 >= args.len() {
                    return invalid_args(prog);
                }
                let impl_opt = args[i + 1].as_str();
                if impl_opt == "compile-only" {
                    opts.compile_only = true;
                } else if impl_opt.starts_with("heapsize=") {
                    match parse_heapsize(impl_opt) {
                        Ok(size) => opts.heap_size = Some(size),
                        Err(()) => return invalid_args(prog),
                    }
                } else if impl_opt.starts_with("emit=") {
                    match parse_emit_opt(impl_opt) {
                        Ok(emit) => opts.emit_opt = Some(emit),
                        Err(()) => return invalid_args(prog),
                    }
                } else {
                    return invalid_args(prog);
                }
                i += 2;
            }
            _ => {
                // `-O`, `-i`, etc. are handled after runtime init.
                i += 1;
            }
        }
    }
    PreProcessResult::Continue(opts)
}

fn compile_source_to_mpy(source: &[u8], filename: &str) -> Vec<u8> {
    use py_rs::bc::ModuleContext;
    use py_rs::compile;
    use py_rs::emitglue::CompiledModule;
    use py_rs::lexer::Lexer;
    use py_rs::malloc;
    use py_rs::mpprint::Print;
    use py_rs::parse::{self, ParseInputKind};
    use py_rs::persistentcode;
    use py_rs::reader;
    use py_rs::vstr::{self, Vstr};

    let file_qstr = qstr::from_str(filename);
    let lex = Lexer::new_from_str_len(file_qstr, source, reader::READER_IS_ROM);
    let mut tree = parse::parse(lex, ParseInputKind::FileInput);
    let ctx = malloc::new_obj::<ModuleContext>().expect("module context");
    let mut cm = CompiledModule {
        context: ctx,
        rc: core::ptr::null(),
        has_native: false,
        n_qstr: 0,
        n_obj: 0,
        arch_flags: 0,
    };
    compile::compile_to_raw_code(&mut tree, file_qstr, false, &mut cm);

    let mut v = Vstr {
        alloc: 0,
        len: 0,
        buf: core::ptr::null_mut(),
        fixed_buf: false,
    };
    let print = Print {
        data: &mut v as *mut Vstr as *mut (),
        print_strn: Some(vstr::vstr_add_strn_print),
    };
    persistentcode::raw_code_save(&cm, &print);
    let saved = unsafe { std::slice::from_raw_parts(v.buf, v.len) }.to_vec();
    vstr::clear(&mut v);
    saved
}

fn register_frozen_test_module() {
    if !(mpconfig::MODULE_FROZEN && mpconfig::MODULE_FROZEN_STR) {
        return;
    }
    let mut names = b"frozentest.py\0".to_vec();
    let content = b"x = 42\n";
    let mut str_sizes = vec![content.len() as u32];
    let mut str_content = content.to_vec();
    str_content.push(0);

    // Freeze MicroPython asyncio package (source) so `import asyncio` works.
    // Paths mirror extmod/asyncio/manifest.py (task.py kept as _asyncio fallback).
    let asyncio_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../extmod/asyncio");
    for fname in [
        "__init__.py",
        "core.py",
        "event.py",
        "funcs.py",
        "lock.py",
        "stream.py",
        "task.py",
        "uasyncio.py",
    ] {
        let path = asyncio_dir.join(fname);
        if let Ok(src) = std::fs::read(&path) {
            names.extend_from_slice(b"asyncio/");
            names.extend_from_slice(fname.as_bytes());
            names.push(0);
            str_sizes.push(src.len() as u32);
            str_content.extend_from_slice(&src);
            str_content.push(0);
        }
    }

    // Freeze python-stdlib `ssl` (thin wrapper over built-in `tls`).
    let ssl_py = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../lib/micropython-lib/python-stdlib/ssl/ssl.py");
    if let Ok(src) = std::fs::read(&ssl_py) {
        names.extend_from_slice(b"ssl.py\0");
        str_sizes.push(src.len() as u32);
        str_content.extend_from_slice(&src);
        str_content.push(0);
    }

    let mut mpy_blobs = Vec::new();
    if mpconfig::MODULE_FROZEN_MPY {
        names.extend_from_slice(b"frozenmpy.py\0");
        mpy_blobs.push(compile_source_to_mpy(b"y = 99\n", "frozenmpy.py"));
    }

    frozenmod::register_frozen_modules(names, str_sizes, str_content, mpy_blobs);
}

fn init_runtime(heap_size: Option<usize>) {
    if mpconfig::PY_THREAD {
        ports_rs_unix::mpthreadport::init();
    }
    if let Some(size) = heap_size {
        py_rs::gc::init_with_size(size);
    }
    runtime::init();
    // Port stack/register scan during `gc.collect()` / auto-collect.
    if mpconfig::ENABLE_GC {
        py_rs::gc::register_collect_hook(ports_rs_unix::gccollect::gc_collect_regs_and_stack);
    }
    let _ = py_rs::modbuiltins::init_builtins_module();
    py_rs::init_builtin_modules();
    register_frozen_test_module();
    extmod_rs::modmachine::register_port_hooks(
        ports_rs_unix::modmachine::machine_mem_get_addr,
        ports_rs_unix::modmachine::machine_idle,
    );
    extmod_rs::init_host();
    if ports_rs_unix::modffi::enabled() {
        let _ = ports_rs_unix::modffi::init_module();
    }
}

fn handle_uncaught_exception(exc: obj::Obj) -> i32 {
    convert_pyexec(pyexec::handle_uncaught_exception(exc))
}

fn print_obj_line(o: obj::Obj) {
    obj::print_helper(&mpprint::PLAT_PRINT, o, PrintKind::Str);
    println!();
}

fn eval_expr(src: &str) -> i32 {
    let mut nlr_buf = nlr::NlrBuf::default();
    match nlr::protect(&mut nlr_buf, || runtime::eval_str(src)) {
        Ok(o) => {
            if let Ok(s) = runtime::obj_to_string(o) {
                println!("{s}");
            } else {
                print_obj_line(o);
            }
            0
        }
        Err(exc_val) => handle_uncaught_exception(py_rs::vm::jump_val_to_exception(exc_val)),
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
    convert_pyexec(pyexec::vstr(&v, true))
}

fn exec_module_import(module: &str) -> obj::Obj {
    let import_args = [
        objstr::new_str(module.as_bytes()),
        obj::CONST_NONE,
        obj::CONST_NONE,
        obj::CONST_FALSE,
        obj::new_small_int(0),
    ];
    let mod_obj = py_rs::builtinimport::builtin___import___default(5, &import_args);

    let mut dest = [obj::OBJ_NULL, obj::OBJ_NULL];
    runtime::load_method_protected(mod_obj, qstr::from_str("__path__"), &mut dest, true);
    if dest[0] != obj::OBJ_NULL {
        let main_name = format!("{module}.__main__");
        let import_args = [
            objstr::new_str(main_name.as_bytes()),
            obj::CONST_NONE,
            obj::CONST_NONE,
            obj::CONST_FALSE,
            obj::new_small_int(0),
        ];
        py_rs::builtinimport::builtin___import___default(5, &import_args);
    }
    mod_obj
}

fn exec_module(module: &str) -> i32 {
    if let Some(path) = modsys::locate_module_path(module) {
        let canonical = path.canonicalize().unwrap_or(path);
        modsys::set_sys_argv0(&canonical.to_string_lossy());
    }
    let mut nlr_buf = nlr::NlrBuf::default();
    match nlr::protect(&mut nlr_buf, || exec_module_import(module)) {
        Ok(_) => 0,
        Err(exc_val) => {
            interrupt_char::set_interrupt_char(-1);
            scheduler::handle_pending(
                py_rs::runtime::HandlePendingBehaviour::CallbacksAndClearExceptions,
            );
            handle_uncaught_exception(py_rs::vm::jump_val_to_exception(exc_val))
        }
    }
}

fn set_sys_executable_from_env(prog: &str) {
    let path = std::env::current_exe()
        .ok()
        .and_then(|p| p.canonicalize().ok())
        .or_else(|| std::path::Path::new(prog).canonicalize().ok())
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|| prog.to_string());
    modsys::set_sys_executable(&path);
}

fn exec_stdin_script() -> i32 {
    let mut src = String::new();
    if io::stdin().read_to_string(&mut src).is_err() {
        return 1;
    }
    if src.is_empty() {
        return 0;
    }
    exec_command(&src)
}

fn convert_pyexec(ret: i32) -> i32 {
    if ret & pyexec::FORCED_EXIT != 0 {
        ret & 0xff
    } else {
        ret
    }
}

fn exec_file(path: &str, prog: &str) -> i32 {
    modsys::set_script_sys_path(path);
    let ret = convert_pyexec(pyexec::file(path));
    if ret == 0 {
        return 0;
    }
    if ret == pyexec::UNHANDLED_EXCEPTION {
        return ret;
    }
    let path_bytes = path.as_bytes();
    if path_bytes.len() >= 3 && path_bytes[path_bytes.len() - 3] == b'm' {
        return ret;
    }
    match std::fs::read_to_string(path) {
        Ok(src) => exec_command(&src),
        Err(e) => {
            eprintln!("{prog}: can't open file '{path}': {e}");
            2
        }
    }
}

enum RunMode {
    Repl,
    Stdin,
    Command(String),
    Module(String),
    File(String),
    Expr(String),
}

struct ParsedCli {
    inspect: bool,
    mode: RunMode,
    sys_argv: Vec<String>,
}

fn parse_cli(args: &[String]) -> Result<ParsedCli, i32> {
    let prog = args.first().map(String::as_str).unwrap_or("metalpython");

    if args.len() == 1 {
        if io::stdin().is_terminal() {
            return Ok(ParsedCli {
                inspect: false,
                mode: RunMode::Repl,
                sys_argv: Vec::new(),
            });
        }
        return Ok(ParsedCli {
            inspect: false,
            mode: RunMode::Stdin,
            sys_argv: Vec::new(),
        });
    }

    let mut inspect = false;
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "-h" | "--help" | "--version" => {
                i += 1;
            }
            "-i" => {
                inspect = true;
                i += 1;
            }
            "-v" => {
                mpstate::with_vm(|vm| vm.mp_verbose_flag += 1);
                i += 1;
            }
            "-X" => {
                i += 2;
            }
            arg if parse_optimise_flag(arg).is_some() => {
                if let Some(level) = parse_optimise_flag(arg) {
                    mpstate::with_vm(|vm| vm.mp_optimise_value = level);
                }
                i += 1;
            }
            "-c" => {
                if i + 1 >= args.len() {
                    eprintln!("{prog}: option requires an argument: -c");
                    return Err(1);
                }
                let cmd = args[i + 1].clone();
                let sys_argv = std::iter::once("-c".to_string())
                    .chain(args[i + 2..].iter().cloned())
                    .collect();
                return Ok(ParsedCli {
                    inspect,
                    mode: RunMode::Command(cmd),
                    sys_argv,
                });
            }
            "-m" => {
                if i + 1 >= args.len() {
                    eprintln!("{prog}: option requires an argument: -m");
                    return Err(1);
                }
                let module = args[i + 1].clone();
                let sys_argv = std::iter::once("-m".to_string())
                    .chain(args[i + 2..].iter().cloned())
                    .collect();
                return Ok(ParsedCli {
                    inspect,
                    mode: RunMode::Module(module),
                    sys_argv,
                });
            }
            arg if arg.starts_with('-') => {
                eprintln!("{prog}: unknown option '{arg}'");
                return Err(1);
            }
            path => {
                if std::path::Path::new(path).is_file() {
                    let sys_argv = std::iter::once(path.to_string())
                        .chain(args[i + 1..].iter().cloned())
                        .collect();
                    return Ok(ParsedCli {
                        inspect,
                        mode: RunMode::File(path.to_string()),
                        sys_argv,
                    });
                }
                if i + 1 == args.len() {
                    return Ok(ParsedCli {
                        inspect,
                        mode: RunMode::Expr(path.to_string()),
                        sys_argv: Vec::new(),
                    });
                }
                eprintln!("{prog}: can't open file '{path}'");
                return Err(2);
            }
        }
    }

    Ok(ParsedCli {
        inspect,
        mode: RunMode::Repl,
        sys_argv: Vec::new(),
    })
}

fn run_mode(mode: RunMode, prog: &str) -> i32 {
    match mode {
        RunMode::Repl => {
            pyexec::event_repl_init();
            convert_pyexec(pyexec::friendly_repl())
        }
        RunMode::Stdin => exec_stdin_script(),
        RunMode::Command(cmd) => exec_command(&cmd),
        RunMode::Module(module) => exec_module(&module),
        RunMode::File(path) => exec_file(&path, prog),
        RunMode::Expr(src) => eval_expr(&src),
    }
}

fn maybe_repl(inspect: bool, prev_ret: i32) -> i32 {
    if !inspect {
        return prev_ret;
    }
    pyexec::event_repl_init();
    let repl_ret = convert_pyexec(pyexec::friendly_repl());
    if prev_ret != 0 {
        prev_ret
    } else {
        repl_ret
    }
}

fn dispatch(args: &[String]) -> i32 {
    let prog = args.first().map(String::as_str).unwrap_or("metalpython");
    match parse_cli(args) {
        Err(code) => code,
        Ok(parsed) => {
            if !parsed.sys_argv.is_empty() {
                let argv_refs: Vec<&str> = parsed.sys_argv.iter().map(String::as_str).collect();
                modsys::set_sys_argv(&argv_refs);
            }
            let ret = run_mode(parsed.mode, prog);
            maybe_repl(parsed.inspect, ret)
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let prog = args.first().map(String::as_str).unwrap_or("metalpython");

    // Wire unbuffered libc stdio + raw termios for interactive REPL.
    py_rs::mphal::register_stdio_port(py_rs::mphal::StdioPort {
        stdin_rx_chr: ports_rs_unix::unix_mphal::stdin_rx_chr,
        stdout_tx_strn: ports_rs_unix::unix_mphal::stdout_tx_strn,
        stdio_mode_raw: ports_rs_unix::unix_mphal::stdio_mode_raw,
        stdio_mode_orig: ports_rs_unix::unix_mphal::stdio_mode_orig,
    });

    let pre = match pre_process_options(&args, prog) {
        PreProcessResult::Exit(code) => {
            std::process::exit(code);
        }
        PreProcessResult::Continue(opts) => opts,
    };

    if pre.compile_only {
        mpstate::set_compile_only(true);
    }

    init_runtime(pre.heap_size);

    set_sys_executable_from_env(prog);

    if let Some(emit) = pre.emit_opt {
        mpstate::with_vm(|vm| vm.default_emit_opt = emit as u8);
    }

    let code = dispatch(&args);
    py_rs::modsys::run_atexit();
    if code != 0 {
        std::process::exit(code);
    }
}
