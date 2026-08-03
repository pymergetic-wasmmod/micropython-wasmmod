//! rewrite of shared/runtime/semihosting_rv32.c + shared/runtime/semihosting_rv32.h
// symmetry: done

const MAGIC_SIZE: usize = 4;
const CMDLINE_MIN_BUFFER_SIZE: usize = 80;

const SYS_OPEN: u32 = 0x01;
const SYS_CLOSE: u32 = 0x02;
const SYS_WRITEC: u32 = 0x03;
const SYS_WRITE0: u32 = 0x04;
const SYS_WRITE: u32 = 0x05;
const SYS_READ: u32 = 0x06;
const SYS_READC: u32 = 0x07;
const SYS_FLEN: u32 = 0x0C;
const SYS_REMOVE: u32 = 0x0E;
const SYS_RENAME: u32 = 0x0F;
const SYS_CLOCK: u32 = 0x10;
const SYS_TIME: u32 = 0x11;
const SYS_SYSTEM: u32 = 0x12;
const SYS_ERRNO: u32 = 0x13;
const SYS_GET_CMDLINE: u32 = 0x15;
const SYS_HEAPINFO: u32 = 0x16;
const SYS_EXIT: u32 = 0x18;
const SYS_EXIT_EXTENDED: u32 = 0x20;
const SYS_ELAPSED: u32 = 0x30;
const SYS_TICKFREQ: u32 = 0x31;
const SYS_ISERROR: u32 = 0x08;
const SYS_ISTTY: u32 = 0x09;
const SYS_SEEK: u32 = 0x0A;
const SYS_TMPNAM: u32 = 0x0D;

static mut EXIT_EXTENDED_AVAILABLE: bool = false;
static mut SPLIT_STDOUT_STDERR: bool = false;

pub static mut MP_SEMIHOSTING_STDOUT: i32 = -1;
pub static mut MP_SEMIHOSTING_STDERR: i32 = -1;

#[repr(C)]
pub struct HeapInfo {
    pub heap_base: u32,
    pub heap_limit: u32,
    pub stack_base: u32,
    pub stack_limit: u32,
}

#[repr(C)]
pub struct ElapsedTicks {
    pub ticks: u32,
}

#[cfg(all(
    target_arch = "riscv32",
    any(target_os = "none", target_os = "unknown")
))]
pub fn call(num: u32, arg: *mut ()) -> i32 {
    let mut call_number_register = num;
    let arguments_register = arg;
    unsafe {
        core::arch::asm!(
            ".option push",
            ".option norvc",
            ".align 4",
            "slli   zero, zero, 0x1F",
            "ebreak",
            "srai   zero, zero, 7",
            ".option pop",
            inout("x10") call_number_register,
            in("x11") arguments_register,
            options(nomem)
        );
    }
    call_number_register as i32
}

#[cfg(not(all(
    target_arch = "riscv32",
    any(target_os = "none", target_os = "unknown")
)))]
pub fn call(num: u32, _arg: *mut ()) -> i32 {
    num as i32
}

fn lookup_open_mode(mode: &str) -> i32 {
    let bytes = mode.as_bytes();
    let mut mode_found = match bytes.first() {
        Some(b'r') => 0x00,
        Some(b'w') => 0x04,
        Some(b'a') => 0x08,
        _ => return -1,
    };
    match bytes.get(1) {
        Some(b'b') => mode_found |= 0x01,
        Some(b'+') => mode_found |= 0x02,
        None => return mode_found,
        _ => return -1,
    }
    mode_found
}

pub fn open(file_name: &str, file_mode: &str) -> i32 {
    if file_name.is_empty() || file_mode.is_empty() {
        return -1;
    }
    let file_open_mode = lookup_open_mode(file_mode);
    if file_open_mode < 0 {
        return -1;
    }
    let mut arguments = [
        file_name.as_ptr() as u32,
        file_open_mode as u32,
        file_name.len() as u32,
    ];
    call(SYS_OPEN, arguments.as_mut_ptr() as *mut ())
}

pub fn close(handle: i32) -> i32 {
    let mut arguments = [handle as u32];
    call(SYS_CLOSE, arguments.as_mut_ptr() as *mut ())
}

pub fn writec(character: u8) {
    let mut arguments = [character as u32];
    call(SYS_WRITEC, arguments.as_mut_ptr() as *mut ());
}

pub fn write(handle: i32, data: &[u8]) -> i32 {
    if data.is_empty() {
        return 0;
    }
    let mut arguments = [handle as u32, data.as_ptr() as u32, data.len() as u32];
    call(SYS_WRITE, arguments.as_mut_ptr() as *mut ())
}

pub fn read(handle: i32, data: &mut [u8]) -> i32 {
    if data.is_empty() {
        return 0;
    }
    let mut arguments = [handle as u32, data.as_mut_ptr() as u32, data.len() as u32];
    call(SYS_READ, arguments.as_mut_ptr() as *mut ())
}

pub fn readc() -> i32 {
    call(SYS_READC, core::ptr::null_mut())
}

pub fn flen(handle: i32) -> i32 {
    let mut arguments = [handle as u32];
    call(SYS_FLEN, arguments.as_mut_ptr() as *mut ())
}

pub fn init() {
    check_extended_features_availability();
    unsafe {
        MP_SEMIHOSTING_STDOUT = open(":tt", "w");
        MP_SEMIHOSTING_STDERR = if SPLIT_STDOUT_STDERR {
            open(":tt", "a")
        } else {
            MP_SEMIHOSTING_STDOUT
        };
    }
}

fn check_extended_features_availability() {
    let features_handle = open(":semihosting-features", "r");
    if features_handle < 0 {
        return;
    }
    if flen(features_handle) < MAGIC_SIZE as i32 {
        close(features_handle);
        return;
    }
    let mut magic_buffer = [0x53u8, 0x48, 0x46, 0x42];
    let mut verify = [0u8; MAGIC_SIZE];
    if read(features_handle, &mut verify) != 0 || verify != magic_buffer {
        close(features_handle);
        return;
    }
    let mut features_byte = 0u8;
    if read(features_handle, std::slice::from_mut(&mut features_byte)) != 0 {
        close(features_handle);
        return;
    }
    close(features_handle);
    unsafe {
        EXIT_EXTENDED_AVAILABLE = features_byte & 0x01 != 0;
        SPLIT_STDOUT_STDERR = features_byte & 0x02 != 0;
    }
}

fn write_to_debug_console(string: &str, length: usize) -> i32 {
    if length == 0 {
        return 0;
    }
    if length == 1 {
        writec(string.as_bytes()[0]);
        return 0;
    }
    let fd = unsafe { MP_SEMIHOSTING_STDOUT };
    write(fd, &string.as_bytes()[..length])
}

pub fn tx_strn(string: &str, length: usize) -> i32 {
    write_to_debug_console(string, length)
}

pub fn tx_strn_cooked(string: &str, length: usize) -> i32 {
    if length == 0 {
        return 0;
    }
    let end = length.min(string.len());
    let bytes = &string.as_bytes()[..end];
    let mut current_offset = 0usize;
    for (index, &byte) in bytes.iter().enumerate() {
        if byte != b'\n' {
            continue;
        }
        write_to_debug_console(
            unsafe { std::str::from_utf8_unchecked(&bytes[current_offset..index]) },
            index - current_offset,
        );
        writec(b'\r');
        current_offset = index;
    }
    write_to_debug_console(
        unsafe { std::str::from_utf8_unchecked(&bytes[current_offset..]) },
        bytes.len() - current_offset,
    )
}

pub fn terminate(code: u32, subcode: u32) {
    if unsafe { EXIT_EXTENDED_AVAILABLE } {
        exit_extended(code, subcode);
    } else {
        exit(code, subcode);
    }
}

pub fn exit(code: u32, subcode: u32) {
    let mut arguments = [code, subcode];
    call(SYS_EXIT, arguments.as_mut_ptr() as *mut ());
    loop {}
}

pub fn exit_extended(code: u32, subcode: u32) {
    let mut arguments = [code, subcode];
    call(SYS_EXIT_EXTENDED, arguments.as_mut_ptr() as *mut ());
    loop {}
}

pub fn rx_char() -> i32 {
    readc()
}

pub fn get_cmdline(buffer: &mut [u8]) -> i32 {
    if buffer.len() < CMDLINE_MIN_BUFFER_SIZE {
        return -1;
    }
    let mut arguments = [buffer.as_mut_ptr() as u32, buffer.len() as u32];
    call(SYS_GET_CMDLINE, arguments.as_mut_ptr() as *mut ())
}

pub fn heapinfo(block: &mut HeapInfo) {
    let mut arguments = [block as *mut HeapInfo as u32];
    call(SYS_HEAPINFO, arguments.as_mut_ptr() as *mut ());
}

pub fn errno() -> i32 {
    call(SYS_ERRNO, core::ptr::null_mut())
}

pub fn clock() -> i32 {
    call(SYS_CLOCK, core::ptr::null_mut())
}

pub fn time() -> i32 {
    call(SYS_TIME, core::ptr::null_mut())
}

pub fn tickfreq() -> i32 {
    call(SYS_TICKFREQ, core::ptr::null_mut())
}

pub fn elapsed(ticks: &mut ElapsedTicks) -> i32 {
    let mut arguments = [ticks as *mut ElapsedTicks as u32];
    call(SYS_ELAPSED, arguments.as_mut_ptr() as *mut ())
}

pub fn iserror(code: i32) -> i32 {
    let mut arguments = [code as u32];
    call(SYS_ISERROR, arguments.as_mut_ptr() as *mut ())
}

pub fn istty(handle: i32) -> i32 {
    let mut arguments = [handle as u32];
    call(SYS_ISTTY, arguments.as_mut_ptr() as *mut ())
}

pub fn seek(handle: i32, offset: u32) -> i32 {
    let mut arguments = [handle as u32, offset];
    call(SYS_SEEK, arguments.as_mut_ptr() as *mut ())
}

pub fn remove(file_name: &str) -> i32 {
    if file_name.is_empty() {
        return -1;
    }
    let mut arguments = [file_name.as_ptr() as u32, file_name.len() as u32];
    call(SYS_REMOVE, arguments.as_mut_ptr() as *mut ())
}

pub fn rename(old_name: &str, new_name: &str) -> i32 {
    if old_name.is_empty() || new_name.is_empty() {
        return -1;
    }
    let mut arguments = [
        old_name.as_ptr() as u32,
        old_name.len() as u32,
        new_name.as_ptr() as u32,
        new_name.len() as u32,
    ];
    call(SYS_RENAME, arguments.as_mut_ptr() as *mut ())
}

pub fn system(command: &str) -> i32 {
    if command.is_empty() {
        return -1;
    }
    let mut arguments = [command.as_ptr() as u32, command.len() as u32];
    call(SYS_SYSTEM, arguments.as_mut_ptr() as *mut ())
}

pub fn tmpnam(identifier: u8, buffer: &mut [u8]) -> i32 {
    if buffer.is_empty() {
        return -1;
    }
    let mut arguments = [
        buffer.as_mut_ptr() as u32,
        identifier as u32,
        buffer.len() as u32,
    ];
    call(SYS_TMPNAM, arguments.as_mut_ptr() as *mut ())
}
