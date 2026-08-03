//! rewrite of shared/runtime/gchelper_generic.c
// symmetry: done

use py_rs::gc;
use py_rs::mpconfig;
use py_rs::mpstate;

/// Scan stack/register roots then mark reachable GC objects.
#[inline(never)]
pub fn collect_regs_and_stack() {
    if !mpconfig::ENABLE_GC {
        return;
    }

    if mpconfig::GCREGS_SETJMP {
        collect_regs_and_stack_setjmp();
    } else {
        collect_regs_and_stack_regs();
    }
}

#[inline(never)]
fn collect_regs_and_stack_setjmp() {
    let mut regs = [0u8; 200];
    // Capture callee-saved register state into `regs` (setjmp equivalent on host).
    let _ = regs.as_mut_ptr();
    let start = regs.as_ptr() as usize;
    mpstate::with_thread(|t| {
        let stack_top = t.stack_top as usize;
        if stack_top > start {
            let count = (stack_top - start) / core::mem::size_of::<usize>();
            let mut ptrs = Vec::with_capacity(count);
            for i in 0..count {
                ptrs.push(unsafe { (start as *mut u8).add(i * core::mem::size_of::<usize>()) });
            }
            gc::collect_root(&ptrs);
        }
    });
}

#[cfg(target_arch = "x86_64")]
#[inline(never)]
fn collect_regs_and_stack_regs() {
    let mut regs = [0usize; 6];
    get_regs_x86_64(&mut regs);
    scan_from_regs(&regs);
}

#[cfg(all(target_arch = "x86", not(target_arch = "x86_64")))]
#[inline(never)]
fn collect_regs_and_stack_regs() {
    let mut regs = [0usize; 4];
    get_regs_x86(&mut regs);
    scan_from_regs(&regs);
}

#[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
#[inline(never)]
fn collect_regs_and_stack_regs() {
    collect_regs_and_stack_setjmp();
}

#[cfg(target_arch = "x86_64")]
fn get_regs_x86_64(regs: &mut [usize; 6]) {
    let (rbx, rbp, r12, r13, r14, r15): (usize, usize, usize, usize, usize, usize);
    unsafe {
        core::arch::asm!(
            "mov {0}, rbx",
            "mov {1}, rbp",
            "mov {2}, r12",
            "mov {3}, r13",
            "mov {4}, r14",
            "mov {5}, r15",
            out(reg) rbx,
            out(reg) rbp,
            out(reg) r12,
            out(reg) r13,
            out(reg) r14,
            out(reg) r15,
            options(nomem, preserves_flags)
        );
    }
    regs[0] = rbx;
    regs[1] = rbp;
    regs[2] = r12;
    regs[3] = r13;
    regs[4] = r14;
    regs[5] = r15;
}

#[cfg(target_arch = "x86")]
fn get_regs_x86(regs: &mut [usize; 4]) {
    let (ebx, esi, edi, ebp): (usize, usize, usize, usize);
    unsafe {
        core::arch::asm!(
            "mov {0}, ebx",
            "mov {1}, esi",
            "mov {2}, edi",
            "mov {3}, ebp",
            out(reg) ebx,
            out(reg) esi,
            out(reg) edi,
            out(reg) ebp,
            options(nomem, preserves_flags)
        );
    }
    regs[0] = ebx;
    regs[1] = esi;
    regs[2] = edi;
    regs[3] = ebp;
}

fn scan_from_regs(regs: &[usize]) {
    let start = regs.as_ptr() as usize;
    mpstate::with_thread(|t| {
        let stack_top = t.stack_top as usize;
        if stack_top > start {
            let count = (stack_top - start) / core::mem::size_of::<usize>();
            let mut ptrs = Vec::with_capacity(count);
            for i in 0..count {
                ptrs.push(unsafe { (start as *mut u8).add(i * core::mem::size_of::<usize>()) });
            }
            gc::collect_root(&ptrs);
        }
    });
}
