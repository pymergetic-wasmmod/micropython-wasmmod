//! rewrite of py/asmbase.c + py/asmbase.h
// symmetry: done

#![allow(clippy::cast_possible_truncation)]

use crate::malloc;
use crate::mpconfig;

pub const MP_ASM_PASS_COMPUTE: u8 = 1;
pub const MP_ASM_PASS_EMIT: u8 = 2;

/// MicroPython assembler base state (`mp_asm_base_t`).
#[repr(C)]
pub struct MpAsmBase {
    pub pass: u8,
    pub suppress: bool,
    pub code_offset: usize,
    pub code_size: usize,
    pub code_base: *mut u8,
    pub max_num_labels: usize,
    pub label_offsets: *mut usize,
}

impl MpAsmBase {
    pub fn suppress_code(&mut self) {
        self.suppress = true;
    }

    pub fn get_code_pos(&self) -> usize {
        self.code_offset
    }

    pub fn get_code_size(&self) -> usize {
        self.code_size
    }

    pub fn get_code(&self) -> *mut u8 {
        self.code_base
    }
}

/// Page-align like `mp_unix_alloc_exec` (`(min_size + 0xfff) & ~0xfff`).
fn exec_page_size(min_size: usize) -> usize {
    (min_size + 0xfff) & !0xfff
}

#[cfg(unix)]
fn plat_alloc_exec(min_size: usize, ptr: &mut *mut u8, size: &mut usize) {
    *size = exec_page_size(min_size);
    let p = unsafe {
        libc::mmap(
            core::ptr::null_mut(),
            *size,
            libc::PROT_READ | libc::PROT_WRITE | libc::PROT_EXEC,
            libc::MAP_PRIVATE | libc::MAP_ANONYMOUS,
            -1,
            0,
        )
    };
    if p == libc::MAP_FAILED {
        *ptr = core::ptr::null_mut();
        *size = 0;
    } else {
        *ptr = p as *mut u8;
    }
}

#[cfg(not(unix))]
fn plat_alloc_exec(min_size: usize, ptr: &mut *mut u8, size: &mut usize) {
    *ptr = malloc::new::<u8>(min_size).expect("exec alloc") as *mut u8;
    *size = min_size;
}

/// True when `plat_alloc_exec` produces memory safe to jump into and the host
/// backend has a verified entry/exit round-trip (see `objfun` tests).
pub fn machine_code_dispatch_supported() -> bool {
    cfg!(unix) && mpconfig::EMIT_X64 && mpconfig::ENABLE_NATIVE_CODE
}

/// Whether the host uses executable `mmap` for native emit buffers (unix).
pub fn plat_alloc_exec_is_rx() -> bool {
    cfg!(unix)
}

#[cfg(unix)]
fn plat_free_exec(code_base: *mut u8, code_size: usize) {
    if !code_base.is_null() && code_size != 0 {
        unsafe {
            libc::munmap(code_base as *mut libc::c_void, code_size);
        }
    }
}

#[cfg(not(unix))]
fn plat_free_exec(code_base: *mut u8, code_size: usize) {
    if !code_base.is_null() && code_size != 0 {
        malloc::del(code_base, code_size);
    }
}

/// `mp_asm_base_init`
pub fn init(base: &mut MpAsmBase, max_num_labels: usize) {
    if !mpconfig::EMIT_MACHINE_CODE {
        return;
    }
    base.max_num_labels = max_num_labels;
    base.label_offsets = malloc::new::<usize>(max_num_labels).expect("label offsets");
}

/// `mp_asm_base_deinit`
pub fn deinit(base: &mut MpAsmBase, free_code: bool) {
    if !mpconfig::EMIT_MACHINE_CODE {
        return;
    }
    if free_code {
        plat_free_exec(base.code_base, base.code_size);
    }
    if !base.label_offsets.is_null() {
        malloc::del(base.label_offsets, base.max_num_labels);
        base.label_offsets = core::ptr::null_mut();
    }
}

/// `mp_asm_base_start_pass`
pub fn start_pass(base: &mut MpAsmBase, pass: i32) {
    if !mpconfig::EMIT_MACHINE_CODE {
        return;
    }
    if pass < MP_ASM_PASS_EMIT as i32 {
        unsafe {
            core::ptr::write_bytes(
                base.label_offsets,
                0xff,
                base.max_num_labels * core::mem::size_of::<usize>(),
            );
        }
    } else {
        plat_alloc_exec(base.code_offset, &mut base.code_base, &mut base.code_size);
        assert!(base.code_size == 0 || !base.code_base.is_null());
    }
    base.pass = pass as u8;
    base.suppress = false;
    base.code_offset = 0;
}

/// `mp_asm_base_get_cur_to_write_bytes`
pub fn get_cur_to_write_bytes(as_in: *mut MpAsmBase, num_bytes_to_write: usize) -> *mut u8 {
    if !mpconfig::EMIT_MACHINE_CODE {
        return core::ptr::null_mut();
    }
    let base = unsafe { &mut *as_in };
    if base.suppress {
        return core::ptr::null_mut();
    }
    let mut c = core::ptr::null_mut();
    if base.pass == MP_ASM_PASS_EMIT {
        assert!(base.code_offset + num_bytes_to_write <= base.code_size);
        c = unsafe { base.code_base.add(base.code_offset) };
    }
    base.code_offset += num_bytes_to_write;
    c
}

/// `mp_asm_base_label_assign`
pub fn label_assign(base: &mut MpAsmBase, label: usize) {
    if !mpconfig::EMIT_MACHINE_CODE {
        return;
    }
    assert!(label < base.max_num_labels);
    base.suppress = false;
    if base.pass < MP_ASM_PASS_EMIT {
        unsafe {
            assert!(*base.label_offsets.add(label) == usize::MAX);
            *base.label_offsets.add(label) = base.code_offset;
        }
    } else {
        unsafe {
            assert!(*base.label_offsets.add(label) == base.code_offset);
        }
    }
}

/// `mp_asm_base_align`
pub fn align(base: &mut MpAsmBase, align_bytes: u32) {
    if !mpconfig::EMIT_MACHINE_CODE {
        return;
    }
    let align = align_bytes as usize;
    base.code_offset = (base.code_offset + align - 1) & !(align - 1);
}

/// `mp_asm_base_data`
pub fn data(base: &mut MpAsmBase, bytesize: u32, mut val: usize) {
    if !mpconfig::EMIT_MACHINE_CODE {
        return;
    }
    let c = get_cur_to_write_bytes(base, bytesize as usize);
    if !c.is_null() {
        unsafe {
            for i in 0..bytesize {
                *c.add(i as usize) = val as u8;
                val >>= 8;
            }
        }
    }
}
