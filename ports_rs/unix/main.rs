//! rewrite of ports/unix/main.c
// symmetry: done

use py_rs::runtime;

/// Compile-only flag (`mp_compile_only`).
pub static COMPILE_ONLY: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// Default GC heap size for unix (scaled by pointer width).
pub fn default_heap_size() -> usize {
    1024 * 1024 * (core::mem::size_of::<usize>() / 4)
}

fn main() {
    if py_rs::mpconfig::PY_THREAD {
        ports_rs_unix::mpthreadport::init();
    }
    runtime::init();
    extmod_rs::modmachine::register_port_hooks(
        ports_rs_unix::modmachine::machine_mem_get_addr,
        ports_rs_unix::modmachine::machine_idle,
    );
    extmod_rs::init_host();
    if ports_rs_unix::modffi::enabled() {
        let _ = ports_rs_unix::modffi::init_module();
    }
    println!("{}", runtime::banner_line());
    let src = std::env::args().nth(1).unwrap_or_else(|| "1+2".into());
    match runtime::eval_source(&src) {
        Ok(s) => println!("{s}"),
        Err(e) => {
            eprintln!("metalpython: {e:?}");
            std::process::exit(1);
        }
    }
}
