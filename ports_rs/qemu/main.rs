//! rewrite of ports/qemu/main.c
// symmetry: done

use py_rs::runtime;

fn main() {
    runtime::init();
    println!("{}", runtime::banner_line());
    let src = std::env::args().nth(1).unwrap_or_else(|| "1+2".into());
    match runtime::eval_source(&src) {
        Ok(s) => println!("{s}"),
        Err(e) => {
            eprintln!("metalpython-qemu: {e:?}");
            std::process::exit(1);
        }
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
            let mut ptrs = Vec::with_capacity(count);
            for i in 0..count {
                ptrs.push(unsafe { (start as *mut u8).add(i * core::mem::size_of::<usize>()) });
            }
            py_rs::gc::collect_root(&ptrs);
        }
    });
}

/// `nlr_jump_fail`
pub fn nlr_jump_fail(_val: *mut ()) -> ! {
    eprintln!("uncaught NLR");
    std::process::exit(1);
}
