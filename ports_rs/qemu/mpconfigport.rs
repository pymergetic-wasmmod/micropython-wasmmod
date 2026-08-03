//! rewrite of ports/qemu/mpconfigport.h
// symmetry: done

pub const CONFIG_ROM_LEVEL: u32 = 30; // MICROPY_CONFIG_ROM_LEVEL_EXTRA_FEATURES

pub const MALLOC_USES_ALLOCATED_SIZE: u8 = 1;
pub const PERSISTENT_CODE_LOAD: bool = true;
pub const MEM_STATS: bool = true;
pub const READER_VFS: bool = true;
pub const ENABLE_GC: bool = true;
pub const ENABLE_EMERGENCY_EXCEPTION_BUF: bool = true;
pub const LONGINT_IMPL: u8 = 2; // MPZ
pub const WARNINGS: bool = true;
pub const PY_SYS_PLATFORM: &str = "qemu";
pub const PY_ASYNCIO: bool = false;
pub const PY_MACHINE: bool = true;
pub const PY_MACHINE_INCLUDEFILE: &str = "ports/qemu/modmachine.c";
pub const PY_MACHINE_RESET: bool = true;
pub const PY_MACHINE_PIN_BASE: bool = true;
pub const VFS: bool = true;
pub const VFS_ROM: bool = true;
pub const VFS_ROM_IOCTL: bool = false;

#[cfg(target_arch = "riscv64")]
pub const SSIZE_MAX: i64 = 0x7fff_ffff_ffff_ffff;
#[cfg(not(target_arch = "riscv64"))]
pub const SSIZE_MAX: i64 = 0x7fff_ffff;

pub type OffT = i64;

pub const NEED_LOG2: bool = true;

pub const STATE_PORT_IS_VM: bool = true;

/// RISC-V RV32 emitter flags (when building for rv32).
#[cfg(target_arch = "riscv32")]
pub const EMIT_RV32: bool = true;
#[cfg(not(target_arch = "riscv32"))]
pub const EMIT_RV32: bool = false;

pub const EMIT_RV32_ZBA: bool = cfg!(target_arch = "riscv32");
pub const EMIT_RV32_ZCMP: bool = cfg!(target_arch = "riscv32");
pub const EMIT_INLINE_RV32: bool = cfg!(target_arch = "riscv32");

#[cfg(target_arch = "riscv64")]
pub const PERSISTENT_CODE_LOAD_NATIVE: bool = true;
#[cfg(not(target_arch = "riscv64"))]
pub const PERSISTENT_CODE_LOAD_NATIVE: bool = false;

/// `QEMU_SOC_*` selection for UART backend (host builds default to VIRT).
pub fn qemu_soc() -> QemuSoc {
    if cfg!(feature = "qemu_soc_stm32") {
        QemuSoc::Stm32
    } else if cfg!(feature = "qemu_soc_nrf51") {
        QemuSoc::Nrf51
    } else if cfg!(feature = "qemu_soc_mps3") {
        QemuSoc::Mps3
    } else if cfg!(feature = "qemu_soc_mps2") {
        QemuSoc::Mps2
    } else if cfg!(feature = "qemu_soc_imx6") {
        QemuSoc::Imx6
    } else if cfg!(feature = "qemu_soc_powernv") {
        QemuSoc::Powernv
    } else {
        QemuSoc::Virt
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum QemuSoc {
    Stm32,
    Nrf51,
    Mps2,
    Mps3,
    Imx6,
    Virt,
    Powernv,
}
