//! rewrite of ports/qemu/mcu/rv32/interrupts.c
// symmetry: done

pub static mut REGISTERS_COPY: [u32; 35] = [0; 35];

const EXCEPTION_CAUSES: [&str; 23] = [
    "Reserved",
    "Supervisor software interrupt",
    "Machine software interrupt",
    "Supervisor timer interrupt",
    "Machine timer interrupt",
    "Supervisor external interrupt",
    "Machine external interrupt",
    "Designated for platform use",
    "Instruction address misaligned",
    "Instruction address fault",
    "Illegal instruction",
    "Breakpoint",
    "Load address misaligned",
    "Load address fault",
    "Store/AMO address misaligned",
    "Store/AMO access fault",
    "Environment call from U-mode",
    "Environment call from S-mode",
    "Environment call from M-mode",
    "Instruction page fault",
    "Load page fault",
    "Store/AMO page fault",
    "Designated for custom use",
];

/// `lookup_cause`
pub fn lookup_cause(mcause: u32) -> &'static str {
    if mcause & 0x8000_0000 != 0 {
        return match mcause & 0x7fff_ffff {
            1 => EXCEPTION_CAUSES[1],
            3 => EXCEPTION_CAUSES[2],
            5 => EXCEPTION_CAUSES[3],
            7 => EXCEPTION_CAUSES[4],
            9 => EXCEPTION_CAUSES[5],
            11 => EXCEPTION_CAUSES[6],
            n if n >= 16 => EXCEPTION_CAUSES[7],
            _ => EXCEPTION_CAUSES[0],
        };
    }
    match mcause {
        0..=7 => EXCEPTION_CAUSES[mcause as usize + 8],
        8 => EXCEPTION_CAUSES[16],
        9 => EXCEPTION_CAUSES[17],
        11 => EXCEPTION_CAUSES[18],
        12 => EXCEPTION_CAUSES[19],
        13 => EXCEPTION_CAUSES[20],
        15 => EXCEPTION_CAUSES[21],
        24..=31 | 48..=63 => EXCEPTION_CAUSES[22],
        _ => EXCEPTION_CAUSES[0],
    }
}

/// `set_interrupt_table` — install vectored mtvec (RV32 only).
#[cfg(target_arch = "riscv32")]
pub fn set_interrupt_table() {
    unsafe {
        core::arch::asm!(
            "csrrci s0, mstatus, 8",
            "csrw   mstatus, s0",
            "csrw   mie, zero",
            "csrw   mip, zero",
            "addi   s0, {mtvec}, 1",
            "csrw   mtvec, s0",
            "csrrsi s0, mstatus, 8",
            "csrw   mstatus, s0",
            mtvec = in(reg) mtvec_table as usize,
            options(nostack)
        );
    }
}

#[cfg(not(target_arch = "riscv32"))]
pub fn set_interrupt_table() {}

/// Vector table symbol referenced from startup (defined in linker script / asm on RV32).
#[cfg(target_arch = "riscv32")]
extern "C" {
    fn mtvec_table();
}

#[cfg(not(target_arch = "riscv32"))]
fn mtvec_table() {}

/// Report machine exception state then exit (firmware path).
pub fn report_exception_and_exit() {
    unsafe {
        eprintln!("\nMACHINE EXCEPTION CAUGHT:\n");
        eprintln!(
            " MEPC={:08X} MTVAL={:08X} MSTATUS={:08X} MCAUSE={:08X} ({})",
            REGISTERS_COPY[31],
            REGISTERS_COPY[33],
            REGISTERS_COPY[34],
            REGISTERS_COPY[32],
            lookup_cause(REGISTERS_COPY[32]),
        );
    }
    super::startup::exit(-1);
}

/// Weak nop handler for unused IRQ slots.
#[cfg(target_arch = "riscv32")]
#[no_mangle]
extern "C" fn mtvec_nop() {}
