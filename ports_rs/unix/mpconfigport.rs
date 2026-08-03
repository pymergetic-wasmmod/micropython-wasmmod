//! rewrite of ports/unix/mpconfigport.h + ports/unix/variants/standard/mpconfigvariant.h (reference)
// symmetry: done

/// Port state aliases VM state (`MP_STATE_PORT`).
pub const STATE_PORT_IS_VM: bool = true;

/// `MICROPY_PY_SYS_PLATFORM`
#[cfg(all(target_os = "macos", target_env = "macos"))]
pub const PY_SYS_PLATFORM: &str = "darwin";
#[cfg(target_os = "freebsd")]
pub const PY_SYS_PLATFORM: &str = "freebsd";
#[cfg(not(any(all(target_os = "macos", target_env = "macos"), target_os = "freebsd")))]
pub const PY_SYS_PLATFORM: &str = "linux";

/// `MICROPY_PY_SYS_PATH_DEFAULT`
#[cfg(target_os = "freebsd")]
pub const PY_SYS_PATH_DEFAULT: &str = ".frozen:~/.micropython/lib:/usr/local/lib/micropython";
#[cfg(not(target_os = "freebsd"))]
pub const PY_SYS_PATH_DEFAULT: &str = ".frozen:~/.micropython/lib:/usr/lib/micropython";

pub const ENABLE_FINALISER: bool = true;
pub const VFS: bool = true;
pub const READER_VFS: bool = true;
pub const HELPER_LEXER_UNIX: bool = true;
pub const VFS_POSIX: bool = true;
pub const READER_POSIX: bool = true;
pub const EPOCH_IS_1970: bool = true;
pub const TIMESTAMP_IMPL: u8 = 2; // MICROPY_TIMESTAMP_IMPL_TIME_T
pub const SELECT_REMAINING_TIME: bool = true;
pub const STACKLESS: u8 = 0;
pub const STACKLESS_STRICT: u8 = 0;
pub const PY_THREAD: bool = true;
pub const PY_THREAD_GIL: bool = false;
pub const PY_THREAD_RECURSIVE_MUTEX: bool = true;

pub const PY_MACHINE_INCLUDEFILE: &str = "ports/unix/modmachine.c";

pub const FATFS_ENABLE_LFN: bool = true;
pub const FATFS_RPATH: u8 = 2;
pub const FATFS_MAX_SS: u16 = 4096;
pub const FATFS_LFN_CODE_PAGE: u16 = 437;

pub const ALLOC_PATH_MAX: usize = libc::PATH_MAX as usize;
pub const MODULE_OVERRIDE_MAIN_IMPORT: bool = true;
pub const PY_SYS_PATH_ARGV_DEFAULTS: bool = false;
pub const PY_SYS_EXECUTABLE: bool = true;
pub const PYEXEC_COMPILE_ONLY: bool = false;
pub const PYEXEC_ENABLE_EXIT_CODE_HANDLING: bool = true;
pub const PY_SOCKET_LISTEN_BACKLOG_DEFAULT: u32 = 128;

/// Linux can access physical memory via `/dev/mem`.
#[cfg(target_os = "linux")]
pub const PLAT_DEV_MEM: bool = true;
#[cfg(not(target_os = "linux"))]
pub const PLAT_DEV_MEM: bool = false;

pub const PY_BLUETOOTH_ENABLE_CENTRAL_MODE: bool = true;

pub const DIRENT_HAVE_D_TYPE: bool = true;
pub const DIRENT_HAVE_D_INO: bool = true;

/// Unix port enables socket, termios, ffi (variant defaults).
pub const PY_SOCKET: bool = true;
pub const PY_TERMIOS: bool = true;
pub const PY_FFI: bool = true;
pub const PY_JNI: bool = false;

/// Coverage instrumentation (coverage variant only).
pub const UNIX_COVERAGE: bool = false;

/// `mp_off_t` width matches C `long` on LP64 hosts.
pub type OffT = i64;

/// Native emitter selection follows host arch (see C `#if defined(__x86_64__)` etc.).
pub fn emit_native_for_host() -> &'static str {
    if cfg!(target_arch = "x86_64") {
        "x64"
    } else if cfg!(target_arch = "x86") {
        "x86"
    } else if cfg!(all(target_arch = "arm", target_feature = "thumb")) {
        "thumb"
    } else if cfg!(target_arch = "arm") {
        "arm"
    } else if cfg!(all(target_arch = "riscv32")) {
        "rv32"
    } else if cfg!(all(target_arch = "riscv64")) {
        "rv64"
    } else {
        "none"
    }
}

/// `MICROPY_UNIX_MACHINE_IDLE` → `sched_yield()`.
pub fn machine_idle() {
    unsafe {
        libc::sched_yield();
    }
}
