//! rewrite of shared/runtime/semihosting_arm.c + shared/runtime/semihosting_arm.h
// symmetry: done

const SYS_OPEN: u32 = 0x01;
const SYS_WRITEC: u32 = 0x03;
const SYS_WRITE: u32 = 0x05;
const SYS_READ: u32 = 0x06;
const SYS_READC: u32 = 0x07;
const SYS_EXIT: u32 = 0x18;

const OPEN_MODE_READ: u32 = 0;
const OPEN_MODE_WRITE: u32 = 4;

static mut MP_SEMIHOSTING_STDOUT: i32 = 0;

#[repr(C)]
struct OpenArgs {
    name: *const u8,
    mode: u32,
    name_len: u32,
}

#[repr(C)]
struct ReadWriteArgs {
    fd: u32,
    str: *const u8,
    len: u32,
}

#[cfg(all(target_arch = "arm", any(target_os = "none", target_os = "unknown")))]
fn semihosting_call(num: u32, arg: *const ()) -> u32 {
    let mut num_reg = num;
    let args_reg = arg;
    unsafe {
        core::arch::asm!(
            "bkpt 0xAB",
            inout("r0") num_reg,
            in("r1") args_reg,
            options(nomem)
        );
    }
    num_reg
}

#[cfg(not(all(target_arch = "arm", any(target_os = "none", target_os = "unknown"))))]
fn semihosting_call(num: u32, _arg: *const ()) -> u32 {
    num
}

fn open_console(mode: u32) -> i32 {
    let name = b":tt\0";
    let args = OpenArgs {
        name: name.as_ptr(),
        mode,
        name_len: 3,
    };
    semihosting_call(SYS_OPEN, &args as *const OpenArgs as *const ()) as i32
}

pub fn init() {
    unsafe {
        MP_SEMIHOSTING_STDOUT = open_console(OPEN_MODE_WRITE);
    }
}

pub fn exit(status: i32) {
    let status = if status == 0 { 0x20026 } else { status as u32 };
    semihosting_call(SYS_EXIT, status as usize as *const ());
}

pub fn rx_char() -> i32 {
    semihosting_call(SYS_READC, core::ptr::null()) as i32
}

pub fn rx_chars(str: &mut [u8]) -> i32 {
    let fd = unsafe { MP_SEMIHOSTING_STDOUT as u32 };
    let args = ReadWriteArgs {
        fd,
        str: str.as_ptr(),
        len: str.len() as u32,
    };
    semihosting_call(SYS_READ, &args as *const ReadWriteArgs as *const ()) as i32
}

fn tx_char(c: u8) {
    semihosting_call(SYS_WRITEC, (&c as *const u8) as *const ());
}

pub fn tx_strn(str: &str, len: usize) -> u32 {
    if len == 0 {
        return 0;
    }
    if len == 1 {
        tx_char(str.as_bytes()[0]);
        return 0;
    }
    let fd = unsafe { MP_SEMIHOSTING_STDOUT as u32 };
    let args = ReadWriteArgs {
        fd,
        str: str.as_ptr(),
        len: len as u32,
    };
    semihosting_call(SYS_WRITE, &args as *const ReadWriteArgs as *const ())
}

pub fn tx_strn_cooked(str: &str, len: usize) -> u32 {
    let end = len.min(str.len());
    let bytes = &str.as_bytes()[..end];
    let mut start = 0usize;
    for (i, &byte) in bytes.iter().enumerate() {
        if byte == b'\n' {
            tx_strn(
                unsafe { std::str::from_utf8_unchecked(&bytes[start..i]) },
                i - start,
            );
            tx_char(b'\r');
            start = i;
        }
    }
    tx_strn(
        unsafe { std::str::from_utf8_unchecked(&bytes[start..]) },
        bytes.len() - start,
    )
}
