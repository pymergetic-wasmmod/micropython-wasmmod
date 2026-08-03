//! Wired `pm_mpy_framebuf_*` accessors.
// symmetry: done

use super::framebuf::framebuf_export;
use py_rs::pm::mpy::pm_mpy_obj_t;

/// `pm_mpy_framebuf_FrameBuffer` — return the `FrameBuffer` export from `framebuf`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_framebuf_FrameBuffer() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(framebuf_export("FrameBuffer"))
}

/// `pm_mpy_framebuf_FrameBuffer1` — return the `FrameBuffer1` export from `framebuf`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_framebuf_FrameBuffer1() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(framebuf_export("FrameBuffer1"))
}

/// `pm_mpy_framebuf_MVLSB` — return the `MVLSB` export from `framebuf`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_framebuf_MVLSB() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(framebuf_export("MVLSB"))
}

/// `pm_mpy_framebuf_MONO_VLSB` — return the `MONO_VLSB` export from `framebuf`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_framebuf_MONO_VLSB() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(framebuf_export("MONO_VLSB"))
}

/// `pm_mpy_framebuf_RGB565` — return the `RGB565` export from `framebuf`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_framebuf_RGB565() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(framebuf_export("RGB565"))
}

/// `pm_mpy_framebuf_GS2_HMSB` — return the `GS2_HMSB` export from `framebuf`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_framebuf_GS2_HMSB() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(framebuf_export("GS2_HMSB"))
}

/// `pm_mpy_framebuf_GS4_HMSB` — return the `GS4_HMSB` export from `framebuf`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_framebuf_GS4_HMSB() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(framebuf_export("GS4_HMSB"))
}

/// `pm_mpy_framebuf_GS8` — return the `GS8` export from `framebuf`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_framebuf_GS8() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(framebuf_export("GS8"))
}

/// `pm_mpy_framebuf_MONO_HLSB` — return the `MONO_HLSB` export from `framebuf`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_framebuf_MONO_HLSB() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(framebuf_export("MONO_HLSB"))
}

/// `pm_mpy_framebuf_MONO_HMSB` — return the `MONO_HMSB` export from `framebuf`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_framebuf_MONO_HMSB() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(framebuf_export("MONO_HMSB"))
}
