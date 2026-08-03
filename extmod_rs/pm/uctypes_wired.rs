//! Wired `pm_mpy_uctypes_*` accessors.
// symmetry: done

use super::uctypes::uctypes_export;
use py_rs::pm::mpy::pm_mpy_obj_t;

/// `pm_mpy_uctypes_struct` — return the `struct` export from `uctypes`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_uctypes_struct() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(uctypes_export("struct"))
}

/// `pm_mpy_uctypes_sizeof` — return the `sizeof` export from `uctypes`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_uctypes_sizeof() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(uctypes_export("sizeof"))
}

/// `pm_mpy_uctypes_addressof` — return the `addressof` export from `uctypes`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_uctypes_addressof() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(uctypes_export("addressof"))
}

/// `pm_mpy_uctypes_bytes_at` — return the `bytes_at` export from `uctypes`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_uctypes_bytes_at() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(uctypes_export("bytes_at"))
}

/// `pm_mpy_uctypes_bytearray_at` — return the `bytearray_at` export from `uctypes`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_uctypes_bytearray_at() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(uctypes_export("bytearray_at"))
}

/// `pm_mpy_uctypes_NATIVE` — return the `NATIVE` export from `uctypes`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_uctypes_NATIVE() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(uctypes_export("NATIVE"))
}

/// `pm_mpy_uctypes_LITTLE_ENDIAN` — return the `LITTLE_ENDIAN` export from `uctypes`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_uctypes_LITTLE_ENDIAN() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(uctypes_export("LITTLE_ENDIAN"))
}

/// `pm_mpy_uctypes_BIG_ENDIAN` — return the `BIG_ENDIAN` export from `uctypes`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_uctypes_BIG_ENDIAN() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(uctypes_export("BIG_ENDIAN"))
}

/// `pm_mpy_uctypes_VOID` — return the `VOID` export from `uctypes`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_uctypes_VOID() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(uctypes_export("VOID"))
}

/// `pm_mpy_uctypes_UINT8` — return the `UINT8` export from `uctypes`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_uctypes_UINT8() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(uctypes_export("UINT8"))
}

/// `pm_mpy_uctypes_INT8` — return the `INT8` export from `uctypes`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_uctypes_INT8() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(uctypes_export("INT8"))
}

/// `pm_mpy_uctypes_UINT16` — return the `UINT16` export from `uctypes`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_uctypes_UINT16() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(uctypes_export("UINT16"))
}

/// `pm_mpy_uctypes_INT16` — return the `INT16` export from `uctypes`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_uctypes_INT16() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(uctypes_export("INT16"))
}

/// `pm_mpy_uctypes_UINT32` — return the `UINT32` export from `uctypes`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_uctypes_UINT32() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(uctypes_export("UINT32"))
}

/// `pm_mpy_uctypes_INT32` — return the `INT32` export from `uctypes`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_uctypes_INT32() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(uctypes_export("INT32"))
}

/// `pm_mpy_uctypes_UINT64` — return the `UINT64` export from `uctypes`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_uctypes_UINT64() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(uctypes_export("UINT64"))
}

/// `pm_mpy_uctypes_INT64` — return the `INT64` export from `uctypes`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_uctypes_INT64() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(uctypes_export("INT64"))
}

/// `pm_mpy_uctypes_BFUINT8` — return the `BFUINT8` export from `uctypes`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_uctypes_BFUINT8() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(uctypes_export("BFUINT8"))
}

/// `pm_mpy_uctypes_BFINT8` — return the `BFINT8` export from `uctypes`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_uctypes_BFINT8() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(uctypes_export("BFINT8"))
}

/// `pm_mpy_uctypes_BFUINT16` — return the `BFUINT16` export from `uctypes`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_uctypes_BFUINT16() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(uctypes_export("BFUINT16"))
}

/// `pm_mpy_uctypes_BFINT16` — return the `BFINT16` export from `uctypes`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_uctypes_BFINT16() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(uctypes_export("BFINT16"))
}

/// `pm_mpy_uctypes_BFUINT32` — return the `BFUINT32` export from `uctypes`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_uctypes_BFUINT32() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(uctypes_export("BFUINT32"))
}

/// `pm_mpy_uctypes_BFINT32` — return the `BFINT32` export from `uctypes`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_uctypes_BFINT32() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(uctypes_export("BFINT32"))
}

/// `pm_mpy_uctypes_BF_POS` — return the `BF_POS` export from `uctypes`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_uctypes_BF_POS() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(uctypes_export("BF_POS"))
}

/// `pm_mpy_uctypes_BF_LEN` — return the `BF_LEN` export from `uctypes`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_uctypes_BF_LEN() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(uctypes_export("BF_LEN"))
}

/// `pm_mpy_uctypes_FLOAT32` — return the `FLOAT32` export from `uctypes`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_uctypes_FLOAT32() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(uctypes_export("FLOAT32"))
}

/// `pm_mpy_uctypes_FLOAT64` — return the `FLOAT64` export from `uctypes`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_uctypes_FLOAT64() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(uctypes_export("FLOAT64"))
}

/// `pm_mpy_uctypes_SHORT` — return the `SHORT` export from `uctypes`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_uctypes_SHORT() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(uctypes_export("SHORT"))
}

/// `pm_mpy_uctypes_USHORT` — return the `USHORT` export from `uctypes`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_uctypes_USHORT() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(uctypes_export("USHORT"))
}

/// `pm_mpy_uctypes_INT` — return the `INT` export from `uctypes`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_uctypes_INT() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(uctypes_export("INT"))
}

/// `pm_mpy_uctypes_UINT` — return the `UINT` export from `uctypes`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_uctypes_UINT() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(uctypes_export("UINT"))
}

/// `pm_mpy_uctypes_LONG` — return the `LONG` export from `uctypes`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_uctypes_LONG() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(uctypes_export("LONG"))
}

/// `pm_mpy_uctypes_ULONG` — return the `ULONG` export from `uctypes`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_uctypes_ULONG() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(uctypes_export("ULONG"))
}

/// `pm_mpy_uctypes_LONGLONG` — return the `LONGLONG` export from `uctypes`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_uctypes_LONGLONG() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(uctypes_export("LONGLONG"))
}

/// `pm_mpy_uctypes_ULONGLONG` — return the `ULONGLONG` export from `uctypes`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_uctypes_ULONGLONG() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(uctypes_export("ULONGLONG"))
}

/// `pm_mpy_uctypes_PTR` — return the `PTR` export from `uctypes`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_uctypes_PTR() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(uctypes_export("PTR"))
}

/// `pm_mpy_uctypes_ARRAY` — return the `ARRAY` export from `uctypes`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_uctypes_ARRAY() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(uctypes_export("ARRAY"))
}
