//! rewrite of shared/libc/printf.c
// symmetry: done

use py_rs::mpconfig;
use py_rs::mpprint::{self, Print, VaArg};

use super::string0;

pub static INTERNAL_PRINTF_PRINTER: Print = Print {
    data: std::ptr::null_mut(),
    print_strn: mpprint::PLAT_PRINT.print_strn,
};

pub fn debug_printf(fmt: &str, args: &[VaArg<'_>]) -> i32 {
    if !mpconfig::DEBUG_PRINTERS {
        return 0;
    }
    mpprint::vprintf(&INTERNAL_PRINTF_PRINTER, fmt, args.iter().copied())
}

struct StrnPrintEnv {
    cur: *mut u8,
    remain: usize,
}

extern "C" fn strn_print_strn(data: *mut (), str: *const u8, len: usize) {
    let env = unsafe { &mut *(data as *mut StrnPrintEnv) };
    let len = len.min(env.remain);
    unsafe {
        string0::memcpy(env.cur, str, len);
        env.cur = env.cur.add(len);
    }
    env.remain -= len;
}

pub fn vsnprintf(str: *mut u8, size: usize, fmt: &str, args: &[VaArg<'_>]) -> i32 {
    if !mpconfig::USE_INTERNAL_PRINTF {
        return -1;
    }
    let mut env = StrnPrintEnv {
        cur: str,
        remain: size,
    };
    let mut print = Print {
        data: &mut env as *mut StrnPrintEnv as *mut (),
        print_strn: Some(strn_print_strn),
    };
    let len = mpprint::vprintf(&mut print, fmt, args.iter().copied());
    if size > 0 {
        unsafe {
            if env.remain == 0 {
                env.cur.sub(1).write(0);
            } else {
                env.cur.write(0);
            }
        }
    }
    len
}

pub fn snprintf(str: *mut u8, size: usize, fmt: &str, args: &[VaArg<'_>]) -> i32 {
    vsnprintf(str, size, fmt, args)
}

pub fn vprintf(fmt: &str, args: &[VaArg<'_>]) -> i32 {
    if !mpconfig::USE_INTERNAL_PRINTF {
        return -1;
    }
    mpprint::vprintf(&INTERNAL_PRINTF_PRINTER, fmt, args.iter().copied())
}

pub fn printf(fmt: &str, args: &[VaArg<'_>]) -> i32 {
    vprintf(fmt, args)
}

pub fn putchar(c: i32) -> i32 {
    let chr = c as u8;
    if let Some(f) = INTERNAL_PRINTF_PRINTER.print_strn {
        f(
            INTERNAL_PRINTF_PRINTER.data,
            &chr as *const u8,
            1,
        );
    }
    c
}

pub fn puts(s: &str) -> i32 {
    if let Some(f) = INTERNAL_PRINTF_PRINTER.print_strn {
        f(INTERNAL_PRINTF_PRINTER.data, s.as_ptr(), s.len());
    }
    putchar(b'\n' as i32)
}
