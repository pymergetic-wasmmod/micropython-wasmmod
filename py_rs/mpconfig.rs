//! rewrite of py/mpconfig.h
//! MetalPython host defaults (unix standard variant). Port overrides live in ports_rs/*/mpconfigport.rs.
//! C macro mapping: `MICROPY_FOO` → `mpconfig::FOO`, `MP_BAR` → `mpconfig::BAR`.
// symmetry: done

/// MicroPython-compatible version numbers (reference tree is 1.29.x).
pub const VERSION_MAJOR: u32 = 1;
pub const VERSION_MINOR: u32 = 29;
pub const VERSION_MICRO: u32 = 0;
pub const VERSION_PRERELEASE: bool = true;

pub const fn make_version(major: u32, minor: u32, patch: u32) -> u32 {
    (major << 16) | (minor << 8) | patch
}

pub const VERSION: u32 = make_version(VERSION_MAJOR, VERSION_MINOR, VERSION_MICRO);

pub const VERSION_STRING: &str = if VERSION_PRERELEASE {
    "1.29.0-preview"
} else {
    "1.29.0"
};

/// Brand string for banners / `sys.implementation.name` (MetalPython, not CPython).
pub const IMPLEMENTATION_NAME: &str = "metalpython";

/// GC heap size for the host smoke path (bytes). Grown later per-port.
pub const GC_HEAP_SIZE: usize = 256 * 1024;

/// Derived from `MICROPY_OBJ_REPR != MICROPY_OBJ_REPR_D` (computed after OBJ_REPR is set).

pub const OBJ_IMMEDIATE_OBJS: bool = OBJ_REPR != OBJ_REPR_D;

// --- ROM level ---
pub const CONFIG_ROM_LEVEL: u32 = 30;
pub const CONFIG_ROM_LEVEL_AT_LEAST_BASIC_FEATURES: bool = true;
pub const CONFIG_ROM_LEVEL_AT_LEAST_CORE_FEATURES: bool = true;
pub const CONFIG_ROM_LEVEL_AT_LEAST_EVERYTHING: bool = false;
pub const CONFIG_ROM_LEVEL_AT_LEAST_EXTRA_FEATURES: bool = true;
pub const CONFIG_ROM_LEVEL_AT_LEAST_FULL_FEATURES: bool = false;
pub const CONFIG_ROM_LEVEL_BASIC_FEATURES: u32 = 20;
pub const CONFIG_ROM_LEVEL_CORE_FEATURES: u32 = 10;
pub const CONFIG_ROM_LEVEL_EVERYTHING: u32 = 50;
pub const CONFIG_ROM_LEVEL_EXTRA_FEATURES: u32 = 30;
pub const CONFIG_ROM_LEVEL_FULL_FEATURES: u32 = 40;
pub const CONFIG_ROM_LEVEL_MINIMUM: u32 = 0;

// --- Object representation ---
pub const OBJ_REPR: u8 = 0;
pub const OBJ_REPR_A: u8 = 0;
pub const OBJ_REPR_B: u8 = 1;
pub const OBJ_REPR_C: u8 = 2;
pub const OBJ_REPR_D: u8 = 3;

// --- Platform word / endianness ---
pub const BITS_PER_BYTE: u8 = 8;
pub const BYTES_PER_OBJ_WORD: u8 = 8;
pub const ENDIANNESS_BIG: bool = false;
pub const ENDIANNESS_LITTLE: bool = true;

// --- Memory allocation ---
pub const ALLOC_GC_STACK_SIZE: u8 = 64;
pub const ALLOC_LEXEL_INDENT_INC: u8 = 8;
pub const ALLOC_LEXER_INDENT_INIT: u8 = 10;
pub const ALLOC_PARSE_CHUNK_INIT: u8 = 128;
pub const ALLOC_PARSE_INTERN_STRING_LEN: u8 = 10;
pub const ALLOC_PARSE_RESULT_INC: u8 = 16;
pub const ALLOC_PARSE_RESULT_INIT: u8 = 32;
pub const ALLOC_PARSE_RULE_INC: u8 = 16;
pub const ALLOC_PARSE_RULE_INIT: u8 = 64;
pub const ALLOC_PATH_MAX: usize = 4096;
pub const ALLOC_QSTR_CHUNK_INIT: u8 = 128;
pub const ALLOC_SCOPE_ID_INC: u8 = 6;
pub const ALLOC_SCOPE_ID_INIT: u8 = 4;
pub const BYTES_PER_GC_BLOCK: u8 = 32;
pub const GC_ALLOC_THRESHOLD: bool = true;
pub const GC_CONSERVATIVE_CLEAR: bool = true;
pub const GC_SPLIT_HEAP: bool = false;
pub const GC_SPLIT_HEAP_AUTO: bool = false;
pub const MALLOC_USES_ALLOCATED_SIZE: u8 = 1;
pub const MEM_STATS: bool = true;
pub const PY_GC_COLLECT_RETVAL: bool = true;
pub const PY_MACHINE_MEM_BACKUP: bool = false;
pub const PY_MICROPYTHON_MEM_INFO: bool = true;

// --- Qstr ---
pub const QSTR_BYTES_IN_HASH: usize = 2;
pub const QSTR_BYTES_IN_LEN: usize = 1;
pub const ALLOC_QSTR_ENTRIES_INIT: usize = 10;

// --- NLR architecture backends (native register save; host uses nlrsetjmp) ---
pub const NLR_SETJMP: bool = true;
pub const NLR_X64: bool = cfg!(all(target_arch = "x86_64", not(any(windows)))) && !NLR_SETJMP;
pub const NLR_X86: bool = cfg!(target_arch = "x86") && !NLR_SETJMP;
pub const NLR_THUMB: bool = cfg!(target_arch = "arm") && !NLR_SETJMP;
pub const NLR_AARCH64: bool = cfg!(target_arch = "aarch64") && !NLR_SETJMP;
pub const NLR_RV32I: bool = cfg!(all(target_arch = "riscv32")) && !NLR_SETJMP;
pub const NLR_RV64I: bool = cfg!(all(target_arch = "riscv64")) && !NLR_SETJMP;
pub const NLR_XTENSA: bool = cfg!(target_arch = "xtensa") && !NLR_SETJMP;
pub const NLR_MIPS: bool = cfg!(target_arch = "mips") && !NLR_SETJMP;
pub const NLR_POWERPC: bool = cfg!(target_arch = "powerpc") && !NLR_SETJMP;
pub const NLR_LOONG64: bool = cfg!(target_arch = "loongarch64") && !NLR_SETJMP;
pub const EMIT_ARM: bool = true;
pub const EMIT_BYTECODE_USES_QSTR_TABLE: bool = true;
pub const EMIT_INLINE_ASM: bool = false;
pub const EMIT_INLINE_RV32: bool = false;
pub const EMIT_INLINE_THUMB: bool = false;
pub const EMIT_INLINE_THUMB_FLOAT: bool = false;
pub const EMIT_INLINE_XTENSA: bool = false;
pub const EMIT_INLINE_XTENSA_UNCOMMON_OPCODES: bool = false;
pub const EMIT_MACHINE_CODE: bool = true;
pub const EMIT_NATIVE: bool = true;
pub const EMIT_NATIVE_DEBUG: bool = false;
pub const EMIT_NATIVE_PRELUDE_SEPARATE_FROM_MACHINE_CODE: bool = false;
pub const EMIT_RV32: bool = true;
pub const EMIT_RV32_ZBA: bool = false;
pub const EMIT_RV32_ZCMP: bool = false;
pub const EMIT_THUMB: bool = true;
pub const EMIT_THUMB_ARMV7M: bool = false;
pub const EMIT_X64: bool = true;
pub const EMIT_X86: bool = true;
pub const EMIT_XTENSA: bool = false;
pub const EMIT_XTENSAWIN: bool = false;
pub const ENABLE_NATIVE_CODE: bool = true;

/// `MICROPY_MAKE_POINTER_CALLABLE` — arch-specific code pointer adjustment.
#[inline]
pub fn make_pointer_callable(p: *const ()) -> *const () {
    #[cfg(any(target_arch = "arm", target_arch = "thumb"))]
    {
        ((p as usize) | 1) as *const ()
    }
    #[cfg(not(any(target_arch = "arm", target_arch = "thumb")))]
    {
        p
    }
}
pub const FLOAT_USE_NATIVE_FLT16: bool = false;
pub const PERSISTENT_CODE: bool = true;
pub const PERSISTENT_CODE_LOAD: bool = true;
pub const PERSISTENT_CODE_LOAD_NATIVE: bool = true;
pub const PERSISTENT_CODE_SAVE: bool = false;
pub const PERSISTENT_CODE_SAVE_FILE: bool = false;
pub const PERSISTENT_CODE_SAVE_FUN: bool = true;
pub const PERSISTENT_CODE_TRACK_BSS_RODATA: bool = false;
pub const PERSISTENT_CODE_TRACK_FUN_DATA: bool = false;
pub const PY_UCTYPES_NATIVE_C_TYPES: u8 = 1;
pub const VFS_BLOCKDEV_NATIVE: bool = false;

// --- Compiler ---
pub const COMP_ALLOW_TOP_LEVEL_AWAIT: u8 = 0;
pub const COMP_CONST: bool = true;
pub const COMP_CONST_FLOAT: bool = true;
pub const COMP_CONST_FOLDING: bool = true;
pub const COMP_CONST_LITERAL: bool = true;
pub const COMP_CONST_TUPLE: bool = true;
pub const COMP_DOUBLE_TUPLE_ASSIGN: bool = true;
pub const COMP_MODULE_CONST: bool = true;
pub const COMP_RETURN_IF_EXPR: bool = true;
pub const COMP_TRIPLE_TUPLE_ASSIGN: bool = true;
pub const ENABLE_COMPILER: bool = true;

// --- Debugging ---
pub const DEBUG_MP_OBJ_SENTINELS: bool = false;
pub const DEBUG_PARSE_RULE_NAME: bool = false;
pub const DEBUG_PRINTERS: bool = true;
pub const DEBUG_VALGRIND: bool = false;
pub const DEBUG_VERBOSE: bool = false;
pub const DEBUG_VM_STACK_OVERFLOW: u8 = 0;

// --- Optimisations ---
pub const OPT_COMPUTED_GOTO: bool = true;
pub const OPT_LOAD_ATTR_FAST_PATH: bool = true;
pub const OPT_MAP_LOOKUP_CACHE: bool = true;
pub const OPT_MAP_LOOKUP_CACHE_SIZE: u8 = 128;
pub const OPT_MATH_FACTORIAL: bool = true;
pub const OPT_MPZ_BITWISE: bool = true;

// --- Runtime / VM ---
pub const NLR_THUMB_USE_LONG_JUMP: bool = false;
pub const STACKLESS: u8 = 0;
pub const STACKLESS_STRICT: u8 = 0;
pub const STACK_CHECK: bool = true;
pub const STACK_CHECK_MARGIN: u8 = 0;
pub const STACK_SIZE_HARD_IRQ: u8 = 0;

// --- I/O ---
pub const READER_POSIX: bool = true;
pub const READER_VFS: bool = true;
pub const STREAMS_DELEGATE_ERROR: bool = true;
pub const STREAMS_NON_BLOCK: bool = true;
pub const STREAMS_POSIX_API: bool = true;
pub const VFS: bool = true;
pub const VFS_FAT: bool = true;
pub const VFS_LFS1: bool = false;
pub const VFS_LFS2: bool = true;
pub const VFS_POSIX: bool = true;
pub const VFS_POSIX_WRITABLE: bool = true;
pub const VFS_ROM: bool = true;
pub const VFS_ROM_IOCTL: bool = false;
pub const VFS_WRITABLE: bool = true;

// --- Numeric types ---
pub const FLOAT_FORMAT_IMPL: u8 = 1;
pub const FLOAT_FORMAT_IMPL_APPROX: u8 = 1;
pub const FLOAT_FORMAT_IMPL_BASIC: u8 = 0;
pub const FLOAT_FORMAT_IMPL_EXACT: u8 = 2;
pub const FLOAT_HIGH_QUALITY_HASH: bool = false;
pub const FLOAT_IMPL: u8 = 2;
pub const FLOAT_IMPL_DOUBLE: u8 = 2;
pub const FLOAT_IMPL_FLOAT: u8 = 1;
pub const FLOAT_IMPL_NONE: u8 = 0;
pub const LONGINT_IMPL: u8 = 2;
pub const LONGINT_IMPL_LONGLONG: u8 = 1;
pub const LONGINT_IMPL_MPZ: u8 = 2;
pub const LONGINT_IMPL_NONE: u8 = 0;

// --- Python features ---
pub const MODULE_ATTR_DELEGATION: bool = true;
pub const MODULE_BUILTIN_INIT: bool = true;
pub const MODULE_BUILTIN_SUBPACKAGES: bool = false;
pub const MODULE_DICT_SIZE: u8 = 1;
pub const MODULE_FROZEN: bool = false;
pub const MODULE_FROZEN_MPY: bool = false;
pub const MODULE_FROZEN_STR: bool = false;
pub const MODULE_GETATTR: bool = true;
pub const MODULE_OVERRIDE_MAIN_IMPORT: bool = true;
pub const MODULE___ALL__: bool = true;
pub const MODULE___FILE__: bool = true;
pub const PY_ALL_INPLACE_SPECIAL_METHODS: bool = false;
pub const PY_ALL_SPECIAL_METHODS: bool = true;
pub const PY_ARRAY: bool = true;
pub const PY_ARRAY_SLICE_ASSIGN: bool = true;
pub const PY_ASSIGN_EXPR: bool = true;
pub const PY_ASYNCIO: bool = true;
pub const PY_ASYNCIO_TASK_QUEUE_PUSH_CALLBACK: bool = false;
pub const PY_ASYNC_AWAIT: bool = true;
pub const PY_ATTRTUPLE: bool = true;
pub const PY_BINASCII: bool = true;
pub const PY_BINASCII_CRC32: bool = true;
pub const PY_BLUETOOTH: bool = false;
pub const PY_BLUETOOTH_ENABLE_CENTRAL_MODE: bool = true;
pub const PY_BLUETOOTH_ENABLE_L2CAP_CHANNELS: bool = false;
pub const PY_BOUND_METHOD_FULL_EQUALITY_CHECK: bool = false;
pub const PY_BTREE: bool = false;
pub const PY_BUILTINS_BYTEARRAY: bool = true;
pub const PY_BUILTINS_BYTES_DECODE_ERRORS: bool = true;
pub const PY_BUILTINS_BYTES_HEX: bool = true;
pub const PY_BUILTINS_CODE: u8 = 2;
pub const PY_BUILTINS_CODE_BASIC: u8 = 2;
pub const PY_BUILTINS_CODE_FULL: u8 = 3;
pub const PY_BUILTINS_CODE_MINIMUM: u8 = 1;
pub const PY_BUILTINS_CODE_NONE: bool = false;
pub const PY_BUILTINS_COMPILE: bool = true;
pub const PY_BUILTINS_COMPLEX: bool = true;
pub const PY_BUILTINS_DICT_FROMKEYS: bool = true;
pub const PY_BUILTINS_DIR: bool = true;
pub const PY_BUILTINS_ENUMERATE: bool = true;
pub const PY_BUILTINS_EVAL_EXEC: bool = true;
pub const PY_BUILTINS_EXECFILE: bool = true;
pub const PY_BUILTINS_FILTER: bool = true;
pub const PY_BUILTINS_FLOAT: bool = true;
pub const PY_BUILTINS_FROZENSET: bool = true;
pub const PY_BUILTINS_HELP: bool = true;
pub const PY_BUILTINS_HELP_COLUMN_WIDTH: u8 = 18;
pub const PY_BUILTINS_HELP_MODULES: bool = true;
pub const PY_BUILTINS_HELP_NUM_COLUMNS: u8 = 4;
pub const PY_BUILTINS_INPUT: bool = true;
pub const PY_BUILTINS_MEMORYVIEW: bool = true;
pub const PY_BUILTINS_MEMORYVIEW_ITEMSIZE: bool = true;
pub const PY_BUILTINS_MIN_MAX: bool = true;
pub const PY_BUILTINS_NEXT2: bool = true;
pub const PY_BUILTINS_NOTIMPLEMENTED: bool = true;
pub const PY_BUILTINS_POW3: bool = true;
pub const PY_BUILTINS_PROPERTY: bool = true;
pub const PY_BUILTINS_RANGE_ATTRS: bool = true;
pub const PY_BUILTINS_RANGE_BINOP: bool = false;
pub const PY_BUILTINS_REVERSED: bool = true;
pub const PY_BUILTINS_ROUND_INT: bool = true;
pub const PY_BUILTINS_SET: bool = true;
pub const PY_BUILTINS_SLICE: bool = true;
pub const PY_BUILTINS_SLICE_ATTRS: bool = true;
pub const PY_BUILTINS_SLICE_INDICES: bool = true;
pub const PY_BUILTINS_STR_CENTER: bool = true;
pub const PY_BUILTINS_STR_COUNT: bool = true;
pub const PY_BUILTINS_STR_OP_MODULO: bool = true;
pub const PY_BUILTINS_STR_PARTITION: bool = true;
pub const PY_BUILTINS_STR_SPLITLINES: bool = true;
pub const PY_BUILTINS_STR_UNICODE: bool = true;
pub const PY_BUILTINS_STR_UNICODE_CHECK: bool = true;
pub const PY_CMATH: bool = true;
pub const PY_COLLECTIONS: bool = true;
pub const PY_COLLECTIONS_DEQUE: bool = true;
pub const PY_COLLECTIONS_DEQUE_ITER: bool = true;
pub const PY_COLLECTIONS_DEQUE_SUBSCR: bool = true;
pub const PY_COLLECTIONS_NAMEDTUPLE__ASDICT: bool = false;
pub const PY_COLLECTIONS_ORDEREDDICT: bool = true;
pub const PY_CRYPTOLIB: bool = false;
pub const PY_CRYPTOLIB_CONSTS: bool = false;
pub const PY_CRYPTOLIB_CTR: bool = false;
pub const PY_DEFLATE: bool = true;
pub const PY_DEFLATE_COMPRESS: bool = false;
pub const PY_DELATTR_SETATTR: bool = true;
pub const PY_DESCRIPTORS: bool = true;
pub const PY_ERRNO: bool = true;
pub const PY_ERRNO_ERRORCODE: bool = true;
pub const PY_FFI: bool = false;
pub const PY_FRAMEBUF: bool = true;
pub const PY_FSTRINGS: bool = true;
pub const PY_FUNCTION_ATTRS: bool = true;
pub const PY_FUNCTION_ATTRS_CODE: bool = true;
pub const PY_GC: bool = true;
pub const PY_GENERATOR_PEND_THROW: bool = true;
pub const PY_HASHLIB: bool = true;
pub const PY_HASHLIB_MD5: bool = false;
pub const PY_HASHLIB_SHA1: bool = false;
pub const PY_HASHLIB_SHA256: bool = true;
pub const PY_HEAPQ: bool = true;
pub const PY_IO: bool = true;
pub const PY_IO_BUFFEREDWRITER: bool = false;
pub const PY_IO_BYTESIO: bool = true;
pub const PY_IO_IOBASE: bool = true;
pub const PY_JSON: bool = true;
pub const PY_JSON_SEPARATORS: bool = true;
pub const PY_LWIP: bool = false;
pub const PY_LWIP_SOCK_RAW: bool = false;
pub const PY_MACHINE: bool = true;
pub const PY_MACHINE_BITSTREAM: u8 = 1;
pub const PY_MACHINE_FREQ_NUM_ARGS_MAX: u8 = 1;
pub const PY_MACHINE_I2C: bool = false;
pub const PY_MACHINE_I2C_TRANSFER_WRITE1: bool = false;
pub const PY_MACHINE_MEMX: bool = true;
pub const PY_MACHINE_PIN_BASE: bool = true;
pub const PY_MACHINE_PULSE: bool = true;
pub const PY_MACHINE_PWM: bool = true;
pub const PY_MACHINE_PWM_DUTY: bool = true;
pub const PY_MACHINE_RESET: bool = false;
pub const PY_MACHINE_SIGNAL: bool = true;
pub const PY_MACHINE_SOFTI2C: bool = true;
pub const PY_MACHINE_SOFTSPI: bool = true;
pub const PY_MACHINE_SPI: bool = false;
pub const PY_MACHINE_SPI_LSB: bool = true;
pub const PY_MACHINE_SPI_MSB: bool = false;
pub const PY_MACHINE_TIMER: bool = true;
pub const PY_MACHINE_UART: bool = true;
pub const PY_MACHINE_UART_SENDBREAK: bool = true;
pub const PY_MACHINE_UART_READCHAR_WRITECHAR: bool = true;
pub const PY_MACHINE_UART_IRQ: bool = false;
pub const PY_MACHINE_WDT: bool = true;
pub const PY_MACHINE_WDT_TIMEOUT_MS: bool = true;
pub const PY_MARSHAL: bool = true;
pub const PY_NETWORK: bool = true;
pub const PY_NETWORK_HOSTNAME_DEFAULT: &'static str = "mpy-unix";
pub const PY_NETWORK_HOSTNAME_MAX_LEN: usize = 32;
pub const PY_MATH: bool = true;
pub const PY_MATH_ATAN2_FIX_INFNAN: bool = false;
pub const PY_MATH_CONSTANTS: bool = true;
pub const PY_MATH_FACTORIAL: bool = true;
pub const PY_MATH_FMOD_FIX_INFNAN: bool = false;
pub const PY_MATH_GAMMA_FIX_NEGINF: bool = false;
pub const PY_MATH_ISCLOSE: bool = true;
pub const PY_MATH_MODF_FIX_NEGZERO: bool = false;
pub const PY_MATH_POW_FIX_NAN: bool = false;
pub const PY_MATH_SPECIAL_FUNCTIONS: bool = true;
pub const PY_MICROPYTHON: bool = true;
pub const PY_MICROPYTHON_HEAP_LOCKED: bool = false;
pub const PY_MICROPYTHON_RINGIO: bool = true;
pub const PY_MICROPYTHON_STACK_USE: bool = true;
pub const PY_ONEWIRE: bool = false;
pub const PY_OS: bool = true;
/// Number of dupterm slots (`MICROPY_PY_OS_DUPTERM`); 0 disables dupterm.
pub const PY_OS_DUPTERM: usize = 3;
pub const PY_OS_DUPTERM_NOTIFY: bool = false;
pub const PY_OS_DUPTERM_BUILTIN_STREAM: bool = false;
pub const PY_OS_ERRNO: bool = true;
pub const PY_OS_GETENV_PUTENV_UNSETENV: bool = true;
pub const PY_OS_STATVFS: bool = true;
pub const PY_OS_SYSTEM: bool = true;
pub const PY_OS_URANDOM: bool = true;
pub const PY_PLATFORM: bool = true;
pub const PY_RANDOM: bool = true;
pub const PY_RANDOM_EXTRA_FUNCS: bool = true;
pub const PY_RE: bool = true;
pub const PY_REVERSE_SPECIAL_METHODS: bool = true;
pub const PY_RE_DEBUG: bool = false;
pub const PY_RE_MATCH_GROUPS: bool = false;
pub const PY_RE_MATCH_SPAN_START_END: bool = false;
pub const PY_RE_SUB: bool = true;
pub const PY_SELECT: bool = true;
pub const PY_SELECT_POSIX_OPTIMISATIONS: bool = true;
pub const PY_SELECT_SELECT: bool = false;
pub const PY_SOCKET: bool = true;
pub const PY_SOCKET_LISTEN_BACKLOG_DEFAULT: u8 = 128;
pub const PY_SSL: bool = true;
pub const PY_SSL_DTLS: bool = false;
pub const PY_SSL_ECDSA_SIGN_ALT: bool = false;
pub const PY_SSL_FINALISER: bool = true;
pub const PY_SSL_MBEDTLS_NEED_ACTIVE_CONTEXT: bool = false;
pub const PY_STRUCT: bool = true;
pub const PY_STRUCT_UNSAFE_TYPECODES: u8 = 1;
pub const PY_STR_BYTES_CMP_WARN: bool = true;
pub const PY_SYS: bool = true;
pub const PY_SYS_ARGV: bool = true;
pub const PY_SYS_ATEXIT: bool = true;
pub const PY_SYS_ATTR_DELEGATION: bool = false;
pub const PY_SYS_EXC_INFO: bool = true;
pub const PY_SYS_EXECUTABLE: bool = true;
pub const PY_SYS_EXIT: bool = true;
pub const PY_SYS_GETSIZEOF: bool = false;
pub const PY_SYS_INTERN: bool = false;
pub const PY_SYS_MAXSIZE: bool = true;
pub const PY_SYS_MODULES: bool = true;
pub const PY_SYS_PATH: bool = true;
pub const PY_SYS_PATH_ARGV_DEFAULTS: bool = false;
pub const PY_SYS_PATH_DEFAULT: &'static str = ".frozen:~/.micropython/lib:/usr/lib/micropython";
pub const PY_SYS_PLATFORM: &'static str = "linux";
pub const PY_SYS_PS1_PS2: bool = true;
pub const PY_SYS_SETTRACE: bool = false;
pub const PY_SYS_STDFILES: bool = true;
pub const PY_SYS_STDIO_BUFFER: bool = true;
pub const PY_SYS_TRACEBACKLIMIT: bool = false;
pub const PY_THREAD: bool = true;
pub const PY_THREAD_GIL: bool = false;
pub const PY_THREAD_GIL_VM_DIVISOR: u8 = 32;
pub const PY_THREAD_RECURSIVE_MUTEX: bool = true;
pub const PY_TIME: bool = true;
pub const PY_TIME_CUSTOM_SLEEP: bool = true;
pub const PY_TIME_GMTIME_LOCALTIME_MKTIME: bool = false;
pub const PY_TIME_TICKS_PERIOD: u64 = 4611686018427387904;
pub const PY_TIME_TIME_TIME_NS: bool = true;
pub const PY_TSTRINGS: bool = false;
pub const PY_UCTYPES: bool = true;
pub const PY_VFS: bool = true;
pub const PY_WASM: bool = false;
pub const PY_WEAKREF: bool = false;
pub const PY_WEBSOCKET: bool = true;

// --- Miscellaneous ---
pub const ASYNC_KBD_INTR: bool = true;
pub const BLUETOOTH_NIMBLE: bool = false;
pub const BUILD_DATE: &'static str = "2026-01-01";
pub const BUILTIN_METHOD_CHECK_SELF_ARG: bool = true;
pub const CAN_OVERRIDE_BUILTINS: bool = true;
pub const CPYTHON_COMPAT: bool = true;
pub const DYNAMIC_COMPILER: bool = false;
pub const EMERGENCY_EXCEPTION_BUF_SIZE: usize = 256;
pub const ENABLE_DOC_STRING: bool = false;
pub const ENABLE_EMERGENCY_EXCEPTION_BUF: bool = true;
pub const ENABLE_EXTERNAL_IMPORT: bool = true;
pub const ENABLE_FINALISER: bool = true;
pub const ENABLE_GC: bool = true;
pub const ENABLE_PYSTACK: bool = false;
pub const ENABLE_SCHEDULER: bool = true;
pub const ENABLE_SOURCE_LINE: bool = true;
pub const ENABLE_VM_ABORT: bool = false;
pub const EPOCH_IS_1970: bool = false;
pub const EPOCH_IS_2000: bool = true;
pub const ERROR_REPORTING: u8 = 3;
pub const ERROR_REPORTING_DETAILED: u8 = 3;
pub const ERROR_REPORTING_NONE: bool = false;
pub const ERROR_REPORTING_NORMAL: u8 = 2;
pub const ERROR_REPORTING_TERSE: bool = true;
pub const FATFS_ENABLE_LFN: bool = true;
pub const FATFS_LFN_CODE_PAGE: u16 = 437;
pub const FATFS_MAX_SS: u16 = 4096;
pub const FATFS_RPATH: u8 = 2;
pub const FULL_CHECKS: bool = true;
pub const GCREGS_SETJMP: bool = true;
pub const GIT_TAG: &'static str = "v1.29.0";
pub const HAS_FILE_READER: bool = true;
pub const HELPER_LEXER_UNIX: bool = true;
pub const HELPER_REPL: bool = true;
pub const HW_BOARD_NAME: bool = false;
pub const HW_MCU_NAME: bool = false;
pub const KBD_EXCEPTION: bool = true;
pub const LOADED_MODULES_DICT_SIZE: u8 = 3;
pub const MULTIPLE_INHERITANCE: bool = true;
pub const PLATFORM_COMPILER: &'static str = "rustc";
pub const PREVIEW_VERSION_2: bool = false;
pub const PYEXEC_COMPILE_ONLY: bool = true;
pub const PYEXEC_ENABLE_EXIT_CODE_HANDLING: bool = true;
pub const PYEXEC_ENABLE_VM_ABORT: bool = false;
pub const PYSTACK_ALIGN: usize = 8;
pub const PYSTACK_DEBUG: bool = false;
pub const READLINE_HISTORY_SIZE: u8 = 50;
pub const REPL_AUTO_INDENT: bool = true;
pub const REPL_EMACS_EXTRA_WORDS_MOVE: bool = true;
pub const REPL_EMACS_KEYS: bool = true;
pub const REPL_EMACS_WORDS_MOVE: bool = true;
pub const REPL_EVENT_DRIVEN: bool = false;
pub const REPL_INFO: bool = false;
pub const SCHEDULER_DEPTH: u8 = 4;
pub const SCHEDULER_STATIC_NODES: bool = false;
pub const SELECT_REMAINING_TIME: bool = true;
pub const SSL_MBEDTLS: bool = true;
pub const TIMESTAMP_IMPL: u8 = 2;
pub const TIMESTAMP_IMPL_LONG_LONG: u8 = 0;
pub const TIMESTAMP_IMPL_TIME_T: u8 = 2;
pub const TIMESTAMP_IMPL_UINT: u8 = 1;
pub const TIME_SUPPORT_Y1969_AND_BEFORE: bool = false;
pub const TIME_SUPPORT_Y2100_AND_BEYOND: bool = false;
pub const TRACKED_ALLOC: bool = false;
pub const USE_GCC_MUL_OVERFLOW_INTRINSIC: bool = false;
pub const USE_INTERNAL_ERRNO: bool = false;
pub const USE_INTERNAL_PRINTF: bool = true;
pub const USE_READLINE_HISTORY: u8 = 1;
pub const WARNINGS: bool = true;
pub const WARNINGS_CATEGORY: bool = false;
pub const INT_MAX: u64 = 9223372036854775807;
pub const INT_MIN: i64 = -9223372036854775808;
pub const INT_TYPE: u8 = 0;
pub const INT_TYPE_INT64: u8 = 1;
pub const INT_TYPE_INTPTR: u8 = 0;
pub const INT_TYPE_OTHER: u8 = 2;
pub const OBJ_WORD_MSBIT_HIGH: u64 = 9223372036854775808;
pub const SMALL_INT_POSITIVE_MASK: u64 = 4611686018427387903;
pub const SSIZE_MAX: u64 = 9223372036854775807;
pub const UINT_MAX: u64 = 18446744073709551615;
