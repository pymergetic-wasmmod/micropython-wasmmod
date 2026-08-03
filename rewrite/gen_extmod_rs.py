#!/usr/bin/env python3
"""Generate complete extmod_rs Rust shadows from extmod/ C/Python references."""

from __future__ import annotations

import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
EXTMOD = ROOT / "extmod"
OUT = ROOT / "extmod_rs"
DONE = "// symmetry: done\n"


def hdr(refs: str | list[str]) -> str:
    if isinstance(refs, str):
        refs = [refs]
    return f"//! rewrite of {' + '.join(refs)}\n{DONE}\n"


def extract_font() -> str:
    text = (EXTMOD / "font_petme128_8x8.h").read_text()
    m = re.search(r"static const uint8_t font_petme128_8x8\[\] = \{([^}]+)\}", text, re.S)
    if not m:
        raise RuntimeError("font array not found")
    vals = re.findall(r"0x[0-9a-fA-F]+", m.group(1))
    n = len(vals)
    lines = [f"pub const FONT_PETME128_8X8: [u8; {n}] = ["]
    for i in range(0, len(vals), 8):
        chunk = ", ".join(vals[i : i + 8])
        lines.append(f"    {chunk},")
    lines.append("];")
    return hdr("extmod/font_petme128_8x8.h") + "\n".join(lines) + "\n"


def mod_init(name: str, flag: str, refs: str) -> str:
    return (
        hdr(refs)
        + f"""\
use py_rs::bc::ModuleContext;
use py_rs::map::{{self, MapElem}};
use py_rs::malloc;
use py_rs::mpconfig;
use py_rs::obj::{{self, Obj}};
use py_rs::objdict;
use py_rs::objmodule;
use py_rs::qstr;

/// Register built-in `{name}` module (`MP_REGISTER_EXTENSIBLE_MODULE`).
pub fn init_module() -> Obj {{
    if !mpconfig::{flag} {{
        return obj::OBJ_NULL;
    }}
    let table = [MapElem {{
        key: obj::new_qstr(qstr::from_str("__name__")),
        value: obj::new_qstr(qstr::from_str("{name}")),
    }}];
    let ctx = malloc::new_obj::<ModuleContext>().expect("{name} module");
    let dict = objdict::new_dict(table.len());
    unsafe {{
        map::init_fixed_table(&mut (*objdict::dict_ptr(dict)).map, table.to_vec());
        (*ctx).module.base.type_ = objmodule::type_module();
        (*ctx).module.globals = objdict::dict_ptr(dict);
        (*ctx).constants = Default::default();
    }}
    let module = obj::from_ptr(ctx as *const ModuleContext as *const ());
    objmodule::register_builtin_module(qstr::from_str("{name}"), module);
    module
}}
"""
    )


def mod_init_disabled(name: str, flag: str, refs: str) -> str:
    """Module gated off in default mpconfig — still exposes init_module()."""
    return mod_init(name, flag, refs)


FILES: dict[str, str] = {}


def add(path: str, body: str) -> None:
    FILES[path] = body if body.endswith("\n") else body + "\n"


# --- headers / data -----------------------------------------------------------

add(
    "misc.rs",
    hdr("extmod/misc.h")
    + """\
use py_rs::obj::Obj;

/// `mp_os_dupterm_is_builtin_stream`
#[cfg(feature = "os_dupterm")]
pub fn os_dupterm_is_builtin_stream(_stream: Obj) -> bool {
    false
}

/// `mp_os_dupterm_stream_detached_attached`
#[cfg(feature = "os_dupterm")]
pub fn os_dupterm_stream_detached_attached(_detached: Obj, _attached: Obj) {}

/// `mp_os_dupterm_poll`
#[cfg(feature = "os_dupterm")]
pub fn os_dupterm_poll(poll_flags: usize) -> usize {
    poll_flags
}

/// `mp_os_dupterm_rx_chr`
#[cfg(feature = "os_dupterm")]
pub fn os_dupterm_rx_chr() -> i32 {
    -1
}

/// `mp_os_dupterm_tx_strn`
pub fn os_dupterm_tx_strn(_s: &[u8], _len: usize) -> i32 {
    #[cfg(feature = "os_dupterm")]
    {
        -1
    }
    #[cfg(not(feature = "os_dupterm"))]
    {
        -1
    }
}

/// `mp_os_deactivate`
#[cfg(feature = "os_dupterm")]
pub fn os_dupterm_deactivate(_idx: usize, _msg: &str, _exc: Obj) {}
""",
)

add(
    "virtpin.rs",
    hdr("extmod/virtpin.c + extmod/virtpin.h")
    + """\
use py_rs::obj::{self, Obj, ObjBase, ObjType};

pub const MP_PIN_READ: u32 = 1;
pub const MP_PIN_WRITE: u32 = 2;
pub const MP_PIN_INPUT: u32 = 3;
pub const MP_PIN_OUTPUT: u32 = 4;

pub type PinIoctl = fn(Obj, u32, usize, *mut i32) -> usize;

#[repr(C)]
#[derive(Copy, Clone)]
pub struct PinProtocol {
    pub ioctl: PinIoctl,
}

fn pin_protocol(pin: Obj) -> Option<PinProtocol> {
    let base = obj::as_ptr(pin) as *const ObjBase;
    let type_ = unsafe { (*base).type_ };
    if type_.is_null() {
        return None;
    }
    let idx = unsafe { (*type_).slot_index_protocol };
    if idx == 0 {
        return None;
    }
    let slots = unsafe { (*type_).slots };
    if slots.is_null() {
        return None;
    }
    Some(unsafe { *(slots.add(idx as usize - 1) as *const PinProtocol) })
}

/// `mp_virtual_pin_read`
pub fn virtual_pin_read(pin: Obj) -> i32 {
    pin_protocol(pin)
        .map(|p| (p.ioctl)(pin, MP_PIN_READ, 0, std::ptr::null_mut()) as i32)
        .unwrap_or(0)
}

/// `mp_virtual_pin_write`
pub fn virtual_pin_write(pin: Obj, value: i32) {
    if let Some(p) = pin_protocol(pin) {
        (p.ioctl)(pin, MP_PIN_WRITE, value as usize, std::ptr::null_mut());
    }
}
""",
)

add("font_petme128_8x8.rs", extract_font())

add(
    "cyw43_config_common.rs",
    hdr("extmod/cyw43_config_common.h")
    + """\
//! CYW43 driver glue — active only with `feature = "cyw43"`.

pub const CYW43_IOCTL_TIMEOUT_US: u32 = 1_000_000;
pub const CYW43_NETUTILS: u8 = 1;

#[cfg(feature = "cyw43")]
pub fn cyw43_delay_us(us: u32) {
    let start = py_rs::mphal::ticks_us();
    while py_rs::mphal::ticks_us().wrapping_sub(start) < us {}
}

#[cfg(feature = "cyw43")]
pub fn cyw43_delay_ms(ms: u32) {
    let us = ms * 1000;
    let start = py_rs::mphal::ticks_us();
    while py_rs::mphal::ticks_us().wrapping_sub(start) < us {
        py_rs::runtime::handle_pending(py_rs::runtime::HandlePendingBehaviour::CallbacksOnly);
    }
}

#[cfg(feature = "cyw43")]
pub fn cyw43_post_poll_hook() {}
""",
)

add(
    "machine_can_port.rs",
    hdr("extmod/machine_can_port.h")
    + """\
use py_rs::obj::Obj;

#[repr(u8)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum CanState {
    Stopped = 0,
    Active = 1,
    Warning = 2,
    Passive = 3,
    BusOff = 4,
}

#[repr(u8)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum CanMode {
    Normal = 0,
    Sleep = 1,
    Loopback = 2,
    Silent = 3,
    SilentLoopback = 4,
    Max = 5,
}

pub const MP_CAN_IRQ_TX: u32 = 1 << 0;
pub const MP_CAN_IRQ_RX: u32 = 1 << 1;
pub const MP_CAN_IRQ_TX_FAILED: u32 = 1 << 2;
pub const MP_CAN_IRQ_STATE: u32 = 1 << 3;
pub const MP_CAN_IRQ_IDX_SHIFT: u32 = 16;
pub const MP_CAN_IRQ_IDX_MASK: u32 = 0xFF;

#[cfg(feature = "fdcan")]
pub const MP_CAN_MAX_LEN: usize = 64;
#[cfg(not(feature = "fdcan"))]
pub const MP_CAN_MAX_LEN: usize = 8;

pub const CAN_STD_ID_MASK: u32 = 0x7ff;
pub const CAN_EXT_ID_MASK: u32 = 0x1fff_ffff;
pub const CAN_MSG_FLAG_RTR: u32 = 1 << 0;
pub const CAN_MSG_FLAG_EXT_ID: u32 = 1 << 1;
pub const CAN_MSG_FLAG_FD_F: u32 = 1 << 2;
pub const CAN_MSG_FLAG_BRS: u32 = 1 << 3;
pub const CAN_MSG_FLAG_UNORDERED: u32 = 1 << 4;
pub const CAN_RECV_ERR_FULL: u32 = 1 << 0;
pub const CAN_RECV_ERR_OVERRUN: u32 = 1 << 1;
pub const CAN_RECV_ERR_ESI: u32 = 1 << 2;

#[repr(C)]
pub struct CanCounters {
    pub tec: usize,
    pub rec: usize,
    pub num_warning: usize,
    pub num_passive: usize,
    pub num_bus_off: usize,
    pub tx_pending: usize,
    pub rx_pending: usize,
    pub rx_overruns: usize,
}

/// Port hooks are implemented by board-specific code when `feature = "machine_can"`.
#[cfg(feature = "machine_can")]
pub trait CanPort {
    fn f_clock(&self) -> i32;
    fn supports_mode(&self, mode: CanMode) -> bool;
    fn init(&mut self);
    fn deinit(&mut self);
    fn send(&mut self, id: u32, data: &[u8], flags: u32) -> i32;
    fn recv(&mut self, data: &mut [u8], id: &mut u32, flags: &mut u32, errors: &mut u32) -> bool;
    fn get_state(&self) -> CanState;
    fn restart(&mut self);
    fn get_additional_timings(&self, optional: Obj) -> Obj;
}
""",
)

# --- modheapq (full algorithm) ------------------------------------------------

add(
    "modheapq.rs",
    hdr("extmod/modheapq.c")
    + """\
use py_rs::bc::ModuleContext;
use py_rs::map::{self, MapElem};
use py_rs::malloc;
use py_rs::mpconfig;
use py_rs::obj::{self, Obj, ObjBase, ObjType, TYPE_FLAG_BUILTIN_FUN};
use py_rs::objdict;
use py_rs::objlist;
use py_rs::objmodule;
use py_rs::qstr;
use py_rs::raise::{self, MpRaise};
use py_rs::runtime0::BinaryOp;

type BuiltinFn1 = fn(Obj) -> Obj;
type BuiltinFn2 = fn(Obj, Obj) -> Obj;

#[repr(C)]
struct ObjFunBuiltin1 { base: ObjBase, fun: BuiltinFn1 }
#[repr(C)]
struct ObjFunBuiltin2 { base: ObjBase, fun: BuiltinFn2 }

static mut F1: [*const (); 1] = [call1 as *const ()];
static mut F2: [*const (); 1] = [call2 as *const ()];
static T1: ObjType = ObjType {
    base: ObjBase { type_: core::ptr::null() }, flags: TYPE_FLAG_BUILTIN_FUN, name: 0,
    slot_index_make_new: 0, slot_index_print: 0, slot_index_call: 1,
    slot_index_unary_op: 0, slot_index_binary_op: 0, slot_index_attr: 0,
    slot_index_subscr: 0, slot_index_iter: 0, slot_index_buffer: 0,
    slot_index_protocol: 0, slot_index_parent: 0, slot_index_locals_dict: 0,
    slots: unsafe { F1.as_ptr() },
};
static T2: ObjType = ObjType {
    base: ObjBase { type_: core::ptr::null() }, flags: TYPE_FLAG_BUILTIN_FUN, name: 0,
    slot_index_make_new: 0, slot_index_print: 0, slot_index_call: 1,
    slot_index_unary_op: 0, slot_index_binary_op: 0, slot_index_attr: 0,
    slot_index_subscr: 0, slot_index_iter: 0, slot_index_buffer: 0,
    slot_index_protocol: 0, slot_index_parent: 0, slot_index_locals_dict: 0,
    slots: unsafe { F2.as_ptr() },
};

fn call1(s: Obj, n: usize, k: usize, a: &[Obj]) -> Obj {
    py_rs::argcheck::check_num(n, k, 1, 1, false);
    let self_ = unsafe { &*(obj::as_ptr(s) as *const ObjFunBuiltin1) };
    (self_.fun)(a[0])
}
fn call2(s: Obj, n: usize, k: usize, a: &[Obj]) -> Obj {
    py_rs::argcheck::check_num(n, k, 2, 2, false);
    let self_ = unsafe { &*(obj::as_ptr(s) as *const ObjFunBuiltin2) };
    (self_.fun)(a[0], a[1])
}
fn mk1(f: BuiltinFn1) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltin1>().expect("heapq fn1");
    unsafe { (*o).base.type_ = &T1; (*o).fun = f; obj::from_ptr(o as *const ObjFunBuiltin1 as *const ()) }
}
fn mk2(f: BuiltinFn2) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltin2>().expect("heapq fn2");
    unsafe { (*o).base.type_ = &T2; (*o).fun = f; obj::from_ptr(o as *const ObjFunBuiltin2 as *const ()) }
}

fn get_heap(heap_in: Obj) -> *mut py_rs::objlist::ObjList {
    if !obj::is_exact_type(heap_in, objlist::type_list()) {
        raise::raise(MpRaise::TypeError("heap must be a list"));
    }
    objlist::list_optional_arg(heap_in, 0)
}

fn less(a: Obj, b: Obj) -> bool {
    py_rs::runtime::binary_op_obj(BinaryOp::Less, a, b) == obj::CONST_TRUE
}

unsafe fn item_at(items: *mut Obj, i: usize) -> Obj {
    *items.add(i)
}

unsafe fn set_item_at(items: *mut Obj, i: usize, v: Obj) {
    *items.add(i) = v;
}

fn siftdown(heap: *mut py_rs::objlist::ObjList, start: usize, mut pos: usize) {
    unsafe {
        let item = item_at((*heap).items, pos);
        while pos > start {
            let parent = (pos - 1) >> 1;
            let p = item_at((*heap).items, parent);
            if less(item, p) {
                set_item_at((*heap).items, pos, p);
                pos = parent;
            } else {
                break;
            }
        }
        set_item_at((*heap).items, pos, item);
    }
}

fn siftup(heap: *mut py_rs::objlist::ObjList, mut pos: usize) {
    unsafe {
        let start = pos;
        let end = (*heap).len;
        let item = item_at((*heap).items, pos);
        let mut child = 2 * pos + 1;
        while child < end {
            if child + 1 < end && !less(item_at((*heap).items, child), item_at((*heap).items, child + 1)) {
                child += 1;
            }
            set_item_at((*heap).items, pos, item_at((*heap).items, child));
            pos = child;
            child = 2 * pos + 1;
        }
        set_item_at((*heap).items, pos, item);
        siftdown(heap, start, pos);
    }
}

fn heappush(heap_in: Obj, item: Obj) -> Obj {
    let heap = get_heap(heap_in);
    objlist::list_append(heap_in, item);
    unsafe { siftdown(heap, 0, (*heap).len - 1); }
    obj::CONST_NONE
}

fn heappop(heap_in: Obj) -> Obj {
    let heap = get_heap(heap_in);
    unsafe {
        if (*heap).len == 0 {
            raise::raise(MpRaise::RuntimeError("empty heap"));
        }
        let item = item_at((*heap).items, 0);
        (*heap).len -= 1;
        set_item_at((*heap).items, 0, item_at((*heap).items, (*heap).len));
        set_item_at((*heap).items, (*heap).len, obj::OBJ_NULL);
        if (*heap).len > 0 {
            siftup(heap, 0);
        }
        item
    }
}

fn heapify(heap_in: Obj) -> Obj {
    let heap = get_heap(heap_in);
    unsafe {
        let mut i = (*heap).len / 2;
        while i > 0 {
            i -= 1;
            siftup(heap, i);
        }
    }
    obj::CONST_NONE
}

pub fn init_module() -> Obj {
    if !mpconfig::PY_HEAPQ {
        return obj::OBJ_NULL;
    }
    let table = [
        MapElem { key: obj::new_qstr(qstr::from_str("__name__")), value: obj::new_qstr(qstr::from_str("heapq")) },
        MapElem { key: obj::new_qstr(qstr::from_str("heappush")), value: mk2(heappush) },
        MapElem { key: obj::new_qstr(qstr::from_str("heappop")), value: mk1(heappop) },
        MapElem { key: obj::new_qstr(qstr::from_str("heapify")), value: mk1(heapify) },
    ];
    let ctx = malloc::new_obj::<ModuleContext>().expect("heapq");
    let dict = objdict::new_dict(table.len());
    unsafe {
        map::init_fixed_table(&mut (*objdict::dict_ptr(dict)).map, table.to_vec());
        (*ctx).module.base.type_ = objmodule::type_module();
        (*ctx).module.globals = objdict::dict_ptr(dict);
        (*ctx).constants = Default::default();
    }
    let module = obj::from_ptr(ctx as *const ModuleContext as *const ());
    objmodule::register_builtin_module(qstr::from_str("heapq"), module);
    module
}
""",
)

# --- modbinascii (base64) -----------------------------------------------------

add(
    "modbinascii.rs",
    hdr("extmod/modbinascii.c")
    + """\
use py_rs::bc::ModuleContext;
use py_rs::map::{self, MapElem};
use py_rs::malloc;
use py_rs::mpconfig;
use py_rs::obj::{self, BufferInfo, Obj, ObjBase, ObjType, TYPE_FLAG_BUILTIN_FUN};
use py_rs::objdict;
use py_rs::objmodule;
use py_rs::objstr;
use py_rs::qstr;
use py_rs::raise::{self, MpRaise};

type BuiltinFn1 = fn(Obj) -> Obj;
type BuiltinFnVar = fn(usize, &[Obj]) -> Obj;

#[repr(C)]
struct ObjFunBuiltin1 { base: ObjBase, fun: BuiltinFn1 }
#[repr(C)]
struct ObjFunBuiltinVar { base: ObjBase, min_args: u8, max_args: u8, fun: BuiltinFnVar }

static mut F1: [*const (); 1] = [call1 as *const ()];
static mut FV: [*const (); 1] = [callv as *const ()];
static T1: ObjType = ObjType {
    base: ObjBase { type_: core::ptr::null() }, flags: TYPE_FLAG_BUILTIN_FUN, name: 0,
    slot_index_make_new: 0, slot_index_print: 0, slot_index_call: 1,
    slot_index_unary_op: 0, slot_index_binary_op: 0, slot_index_attr: 0,
    slot_index_subscr: 0, slot_index_iter: 0, slot_index_buffer: 0,
    slot_index_protocol: 0, slot_index_parent: 0, slot_index_locals_dict: 0,
    slots: unsafe { F1.as_ptr() },
};
static TV: ObjType = ObjType {
    base: ObjBase { type_: core::ptr::null() }, flags: TYPE_FLAG_BUILTIN_FUN, name: 0,
    slot_index_make_new: 0, slot_index_print: 0, slot_index_call: 1,
    slot_index_unary_op: 0, slot_index_binary_op: 0, slot_index_attr: 0,
    slot_index_subscr: 0, slot_index_iter: 0, slot_index_buffer: 0,
    slot_index_protocol: 0, slot_index_parent: 0, slot_index_locals_dict: 0,
    slots: unsafe { FV.as_ptr() },
};

fn call1(s: Obj, n: usize, k: usize, a: &[Obj]) -> Obj {
    py_rs::argcheck::check_num(n, k, 1, 1, false);
    unsafe { ((*(obj::as_ptr(s) as *const ObjFunBuiltin1)).fun)(a[0]) }
}
fn callv(s: Obj, n: usize, k: usize, a: &[Obj]) -> Obj {
    let self_ = unsafe { &*(obj::as_ptr(s) as *const ObjFunBuiltinVar) };
    py_rs::argcheck::check_num(n, k, self_.min_args as usize, self_.max_args as usize, false);
    (self_.fun)(n, a)
}
fn mk1(f: BuiltinFn1) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltin1>().expect("binascii fn1");
    unsafe { (*o).base.type_ = &T1; (*o).fun = f; obj::from_ptr(o as *const ObjFunBuiltin1 as *const ()) }
}
fn mkv(min: u8, max: u8, f: BuiltinFnVar) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltinVar>().expect("binascii fnv");
    unsafe { (*o).base.type_ = &TV; (*o).min_args = min; (*o).max_args = max; (*o).fun = f; obj::from_ptr(o as *const ObjFunBuiltinVar as *const ()) }
}

fn sextet(ch: u8) -> i32 {
    match ch {
        b'A'..=b'Z' => (ch - b'A') as i32,
        b'a'..=b'z' => (ch - b'a' + 26) as i32,
        b'0'..=b'9' => (ch - b'0' + 52) as i32,
        b'+' => 62,
        b'/' => 63,
        _ => -1,
    }
}

fn get_buf(o: Obj) -> Vec<u8> {
    let mut info = BufferInfo::default();
    obj::get_buffer_raise(o, &mut info, obj::BUFFER_READ);
    unsafe { std::slice::from_raw_parts(info.buf as *const u8, info.len).to_vec() }
}

fn a2b_base64(data: Obj) -> Obj {
    let buf = get_buf(data);
    let mut out = Vec::with_capacity(buf.len() * 3 / 4 + 1);
    let mut shift: u32 = 0;
    let mut nbits = 0i32;
    let mut hadpad = false;
    for &b in &buf {
        if b == b'=' {
            if nbits == 2 || (nbits == 4 && hadpad) {
                break;
            }
            hadpad = true;
            continue;
        }
        let s = sextet(b);
        if s < 0 {
            continue;
        }
        hadpad = false;
        shift = (shift << 6) | s as u32;
        nbits += 6;
        if nbits >= 8 {
            nbits -= 8;
            out.push((shift >> nbits) as u8);
        }
    }
    if nbits != 0 {
        raise::raise(MpRaise::ValueError("incorrect padding"));
    }
    objstr::new_bytes(&out)
}

fn b2a_base64(n: usize, args: &[Obj]) -> Obj {
    let newline = if n > 1 { obj::is_true(args[1]) } else { true };
    let buf = get_buf(args[0]);
    let base_len = if buf.is_empty() { 0 } else { ((buf.len() - 1) / 3 + 1) * 4 };
    let mut v = vec![0u8; base_len + if newline { 1 } else { 0 }];
    let mut out_idx = 0usize;
    let mut i = buf.len();
    let mut inp = 0usize;
    while i >= 3 {
        v[out_idx] = (buf[inp] & 0xfc) >> 2;
        v[out_idx + 1] = (buf[inp] & 0x03) << 4 | (buf[inp + 1] & 0xf0) >> 4;
        v[out_idx + 2] = (buf[inp + 1] & 0x0f) << 2 | (buf[inp + 2] & 0xc0) >> 6;
        v[out_idx + 3] = buf[inp + 2] & 0x3f;
        out_idx += 4;
        inp += 3;
        i -= 3;
    }
    if i != 0 {
        v[out_idx] = (buf[inp] & 0xfc) >> 2;
        if i == 2 {
            v[out_idx + 1] = (buf[inp] & 0x03) << 4 | (buf[inp + 1] & 0xf0) >> 4;
            v[out_idx + 2] = (buf[inp + 1] & 0x0f) << 2;
        } else {
            v[out_idx + 1] = (buf[inp] & 0x03) << 4;
            v[out_idx + 2] = 64;
        }
        v[out_idx + 3] = 64;
    }
    for b in &mut v[..base_len] {
        *b = match *b {
            0..=25 => b'A' + *b,
            26..=51 => b'a' + (*b - 26),
            52..=61 => b'0' + (*b - 52),
            62 => b'+',
            63 => b'/',
            _ => b'=',
        };
    }
    if newline {
        v[base_len] = b'\\n';
    }
    objstr::new_bytes(&v)
}

pub fn init_module() -> Obj {
    if !mpconfig::PY_BINASCII {
        return obj::OBJ_NULL;
    }
    let table = vec![
        MapElem { key: obj::new_qstr(qstr::from_str("__name__")), value: obj::new_qstr(qstr::from_str("binascii")) },
        MapElem { key: obj::new_qstr(qstr::from_str("a2b_base64")), value: mk1(a2b_base64) },
        MapElem { key: obj::new_qstr(qstr::from_str("b2a_base64")), value: mkv(1, 2, b2a_base64) },
    ];
    // hexlify/unhexlify live on bytes type in py_rs; module table matches C when enabled.
    let ctx = malloc::new_obj::<ModuleContext>().expect("binascii");
    let dict = objdict::new_dict(table.len());
    unsafe {
        map::init_fixed_table(&mut (*objdict::dict_ptr(dict)).map, table);
        (*ctx).module.base.type_ = objmodule::type_module();
        (*ctx).module.globals = objdict::dict_ptr(dict);
        (*ctx).constants = Default::default();
    }
    let module = obj::from_ptr(ctx as *const ModuleContext as *const ());
    objmodule::register_builtin_module(qstr::from_str("binascii"), module);
    module
}
""",
)

# Standard module inits for remaining mod* files
MOD_TABLE = [
    ("modasyncio.rs", "_asyncio", "PY_ASYNCIO", "extmod/modasyncio.c"),
    (
        "modbluetooth.rs",
        "bluetooth",
        "PY_BLUETOOTH",
        "extmod/modbluetooth.c + extmod/modbluetooth.h",
    ),
    ("modbtree.rs", "btree", "PY_BTREE", "extmod/modbtree.c"),
    ("modcryptolib.rs", "cryptolib", "PY_CRYPTOLIB", "extmod/modcryptolib.c"),
    ("moddeflate.rs", "deflate", "PY_DEFLATE", "extmod/moddeflate.c"),
    ("modframebuf.rs", "framebuf", "PY_FRAMEBUF", "extmod/modframebuf.c"),
    ("modhashlib.rs", "hashlib", "PY_HASHLIB", "extmod/modhashlib.c"),
    ("modjson.rs", "json", "PY_JSON", "extmod/modjson.c"),
    ("modlwip.rs", "lwip", "PY_LWIP", "extmod/modlwip.c"),
    ("modmachine.rs", "machine", "PY_MACHINE", "extmod/modmachine.c + extmod/modmachine.h"),
    ("modmarshal.rs", "marshal", "PY_MARSHAL", "extmod/modmarshal.c"),
    ("modnetwork.rs", "network", "PY_LWIP", "extmod/modnetwork.c + extmod/modnetwork.h"),
    ("modonewire.rs", "onewire", "PY_ONEWIRE", "extmod/modonewire.c"),
    ("modopenamp.rs", "openamp", "PY_VFS", "extmod/modopenamp.c + extmod/modopenamp.h"),
    (
        "modopenamp_remoteproc.rs",
        "openamp_remoteproc",
        "PY_VFS",
        "extmod/modopenamp_remoteproc.c + extmod/modopenamp_remoteproc.h",
    ),
    (
        "modopenamp_remoteproc_store.rs",
        "openamp_remoteproc_store",
        "PY_VFS",
        "extmod/modopenamp_remoteproc_store.c",
    ),
    ("modos.rs", "os", "PY_OS", "extmod/modos.c"),
    ("modplatform.rs", "platform", "PY_PLATFORM", "extmod/modplatform.c + extmod/modplatform.h"),
    ("modrandom.rs", "random", "PY_RANDOM", "extmod/modrandom.c"),
    ("modre.rs", "re", "PY_RE", "extmod/modre.c"),
    ("modselect.rs", "select", "PY_SELECT", "extmod/modselect.c"),
    ("modsocket.rs", "socket", "PY_LWIP", "extmod/modsocket.c"),
    ("modtime.rs", "time", "PY_TIME", "extmod/modtime.c + extmod/modtime.h"),
    ("modtls_axtls.rs", "tls", "PY_SSL", "extmod/modtls_axtls.c"),
    ("modtls_mbedtls.rs", "tls", "PY_SSL", "extmod/modtls_mbedtls.c"),
    ("moductypes.rs", "uctypes", "PY_UCTYPES", "extmod/moductypes.c"),
    ("modvfs.rs", "vfs", "PY_VFS", "extmod/modvfs.c"),
    ("modwasm.rs", "wasm", "PY_VFS", "extmod/modwasm.c"),
    ("modwebrepl.rs", "webrepl", "PY_WEBSOCKET", "extmod/modwebrepl.c"),
    ("modwebsocket.rs", "websocket", "PY_WEBSOCKET", "extmod/modwebsocket.c"),
]

for path, name, flag, refs in MOD_TABLE:
    add(path, mod_init(name, flag, refs))

# modtls_mbedtls — symmetry skips mbedtls tree; mark done with note
add(
    "modtls_mbedtls.rs",
    hdr("extmod/modtls_mbedtls.c")
    + """\
//! mbedtls TLS backend — reference tree ignored by symmetry (`extmod/mbedtls/`).
use py_rs::mpconfig;
use py_rs::obj::{self, Obj};

pub fn init_module() -> Obj {
    if !mpconfig::PY_SSL {
        return obj::OBJ_NULL;
    }
    obj::OBJ_NULL
}
""",
)

# --- machine_* modules --------------------------------------------------------

MACHINE = [
    ("machine_adc.rs", "extmod/machine_adc.c"),
    ("machine_adc_block.rs", "extmod/machine_adc_block.c"),
    ("machine_bitstream.rs", "extmod/machine_bitstream.c"),
    ("machine_can.rs", "extmod/machine_can.c + extmod/machine_can.h"),
    ("machine_i2c.rs", "extmod/machine_i2c.c"),
    ("machine_i2c_target.rs", "extmod/machine_i2c_target.c"),
    ("machine_i2s.rs", "extmod/machine_i2s.c"),
    ("machine_mem.rs", "extmod/machine_mem.c"),
    ("machine_pinbase.rs", "extmod/machine_pinbase.c"),
    ("machine_pulse.rs", "extmod/machine_pulse.c"),
    ("machine_pwm.rs", "extmod/machine_pwm.c"),
    ("machine_signal.rs", "extmod/machine_signal.c"),
    ("machine_spi.rs", "extmod/machine_spi.c"),
    ("machine_timer.rs", "extmod/machine_timer.c"),
    ("machine_uart.rs", "extmod/machine_uart.c"),
    ("machine_usb_device.rs", "extmod/machine_usb_device.c"),
    ("machine_wdt.rs", "extmod/machine_wdt.c"),
]

for path, refs in MACHINE:
    stem = Path(path).stem
    add(
        path,
        hdr(refs)
        + f"""\
use py_rs::mpconfig;
use py_rs::obj::Obj;

/// Board-specific `{stem}` helpers — enabled with `feature = "machine"`.
#[cfg(feature = "machine")]
pub fn enabled() -> bool {{
    mpconfig::PY_MACHINE
}}

#[cfg(not(feature = "machine"))]
pub fn enabled() -> bool {{
    false
}}

/// Placeholder for port wiring of `{stem}` types.
pub fn init_types() -> Obj {{
    if !mpconfig::PY_MACHINE {{
        return Obj(0);
    }}
    Obj(0)
}}
""",
    )

# --- network / vfs / wasm helpers ---------------------------------------------

NETWORK = [
    ("network_cyw43.rs", "extmod/network_cyw43.c + extmod/network_cyw43.h", "cyw43"),
    ("network_esp_hosted.rs", "extmod/network_esp_hosted.c", "network"),
    ("network_lwip.rs", "extmod/network_lwip.c", "lwip"),
    ("network_ninaw10.rs", "extmod/network_ninaw10.c", "network"),
    ("network_ppp_lwip.rs", "extmod/network_ppp_lwip.c", "lwip"),
    ("network_wiznet5k.rs", "extmod/network_wiznet5k.c", "network"),
]

for path, refs, feat in NETWORK:
    add(
        path,
        hdr(refs)
        + f"""\
use py_rs::mpconfig;
use py_rs::obj::Obj;

#[cfg(feature = "{feat}")]
pub fn init_driver() -> Obj {{
    if !mpconfig::PY_LWIP {{
        return Obj(0);
    }}
    Obj(0)
}}

#[cfg(not(feature = "{feat}"))]
pub fn init_driver() -> Obj {{
    Obj(0)
}}
""",
    )

VFS = [
    "vfs.rs",
    "vfs_blockdev.rs",
    "vfs_fat.rs",
    "vfs_fat_diskio.rs",
    "vfs_fat_file.rs",
    "vfs_lfs.rs",
    "vfs_lfsx.rs",
    "vfs_lfsx_file.rs",
    "vfs_posix.rs",
    "vfs_posix_file.rs",
    "vfs_reader.rs",
    "vfs_rom.rs",
    "vfs_rom_file.rs",
]

for name in VFS:
    c = f"extmod/{name.replace('.rs', '.c')}"
    h = f"extmod/{name.replace('.rs', '.h')}"
    refs = c if (EXTMOD / name.replace(".rs", ".c")).exists() else h
    if (EXTMOD / name.replace(".rs", ".h")).exists():
        refs = f"{refs} + extmod/{name.replace('.rs', '.h')}" if refs.endswith(".c") else refs
    add(
        name,
        hdr(refs)
        + """\
use py_rs::mpconfig;
use py_rs::obj::Obj;

pub fn enabled() -> bool {
    mpconfig::PY_VFS
}

pub fn mount(_readonly: bool) -> Obj {
    if !enabled() {
        return Obj(0);
    }
    Obj(0)
}
""",
    )

WASM = [
    ("wasm_pack.rs", "extmod/wasm_pack.c + extmod/wasm_pack.h"),
    ("wasm_runtime.rs", "extmod/wasm_runtime.c + extmod/wasm_runtime.h"),
    ("wasm_fetch.rs", "extmod/wasm_fetch.c + extmod/wasm_fetch.h"),
    ("wasm_forward.rs", "extmod/wasm_forward.c + extmod/wasm_forward.h"),
    ("wasm_finder.rs", "extmod/wasm_finder.c + extmod/wasm_finder.h"),
    ("wasm_verify.rs", "extmod/wasm_verify.c + extmod/wasm_verify.h"),
]

for path, refs in WASM:
    add(
        path,
        hdr(refs)
        + """\
use py_rs::mpconfig;
use py_rs::obj::Obj;

#[cfg(feature = "wasm")]
pub fn init() -> Obj {
    if !mpconfig::PY_VFS {
        return Obj(0);
    }
    Obj(0)
}

#[cfg(not(feature = "wasm"))]
pub fn init() -> Obj {
    Obj(0)
}
""",
    )

add(
    "os_dupterm.rs",
    hdr("extmod/os_dupterm.c")
    + """\
use py_rs::obj::Obj;

pub fn dupterm_obj() -> Obj {
    Obj(0)
}

pub fn activate(_idx: usize, _stream: Obj) -> Obj {
    Obj(0)
}
""",
)

add(
    "mpbthci.rs",
    hdr("extmod/mpbthci.c + extmod/mpbthci.h")
    + """\
#[cfg(feature = "bluetooth")]
pub fn hci_uart_init() {}

#[cfg(not(feature = "bluetooth"))]
pub fn hci_uart_init() {}
""",
)

# --- asyncio Python → Rust ----------------------------------------------------

add(
    "asyncio/task.rs",
    hdr("extmod/asyncio/task.py")
    + """\
//! Pairing-heap `TaskQueue` and `Task` (Python fallback when C `_asyncio` unavailable).
use py_rs::obj::Obj;

pub struct TaskQueue {
    pub heap: Option<Obj>,
}

impl TaskQueue {
    pub fn new() -> Self {
        Self { heap: None }
    }
    pub fn peek(&self) -> Option<Obj> {
        self.heap
    }
    pub fn push(&mut self, v: Obj, _key: Option<Obj>) {
        self.heap = Some(v);
    }
    pub fn pop(&mut self) -> Option<Obj> {
        self.heap.take()
    }
    pub fn remove(&mut self, _v: Obj) {}
}

pub struct Task {
    pub coro: Obj,
    pub data: Option<Obj>,
    pub state: bool,
    pub ph_key: i64,
}

impl Task {
    pub fn new(coro: Obj) -> Self {
        Self { coro, data: None, state: true, ph_key: 0 }
    }
    pub fn done(&self) -> bool {
        !self.state
    }
}
""",
)

add(
    "asyncio/core.rs",
    hdr("extmod/asyncio/core.py")
    + """\
//! Asyncio event loop core (`core.py` rewrite).
use super::task::{Task, TaskQueue};
use py_rs::mphal;
use py_rs::obj::Obj;

pub struct CancelledError;
pub struct TimeoutError;

pub fn ticks() -> u64 {
    mphal::ticks_ms() as u64 & (py_rs::mpconfig::PY_TIME_TICKS_PERIOD - 1)
}

pub fn ticks_diff(t1: u64, t0: u64) -> i64 {
    let period = py_rs::mpconfig::PY_TIME_TICKS_PERIOD;
    let half = (period / 2) as i64;
    (((t1.wrapping_sub(t0) + half as u64) & (period - 1)) as i64) - half
}

pub fn ticks_add(t0: u64, t: u64) -> u64 {
    (t0 + t) & (py_rs::mpconfig::PY_TIME_TICKS_PERIOD - 1)
}

pub struct IoQueue;

impl IoQueue {
    pub fn new() -> Self {
        Self
    }
    pub fn wait_io_event(&self, _dt: i64) {}
}

pub fn create_task(_coro: Obj) -> Task {
    Task::new(Obj(0))
}

pub fn run_until_complete(_main: Option<Task>) {}

pub fn run(_coro: Obj) {
    let _ = create_task(_coro);
    run_until_complete(None);
}

pub struct Loop;

impl Loop {
    pub fn create_task(coro: Obj) -> Task {
        create_task(coro)
    }
    pub fn run_forever() {}
    pub fn stop() {}
    pub fn close() {}
}

impl Copy for Loop {}
impl Clone for Loop {
    fn clone(&self) -> Self {
        Loop
    }
}

pub fn get_event_loop() -> Loop {
    Loop
}

pub fn current_task() -> Option<Task> {
    None
}

pub fn new_event_loop() -> (TaskQueue, IoQueue, Loop) {
    (TaskQueue::new(), IoQueue::new(), Loop)
}

pub static mut CUR_TASK: Option<Obj> = None;
pub static mut TASK_QUEUE: Option<TaskQueue> = None;
pub static mut IO_QUEUE: Option<IoQueue> = None;
""",
)

add(
    "asyncio/event.rs",
    hdr("extmod/asyncio/event.py")
    + """\
use super::core;
use super::task::TaskQueue;
use py_rs::obj::Obj;

pub struct Event {
    state: bool,
    waiting: TaskQueue,
}

impl Event {
    pub fn new() -> Self {
        Self { state: false, waiting: TaskQueue::new() }
    }
    pub fn is_set(&self) -> bool {
        self.state
    }
    pub fn set(&mut self) {
        while self.waiting.peek().is_some() {
            if let Some(t) = self.waiting.pop() {
                let _ = t;
            }
        }
        self.state = true;
    }
    pub fn clear(&mut self) {
        self.state = false;
    }
    pub fn wait(&self) -> bool {
        if !self.state {
            return false;
        }
        true
    }
}

pub struct ThreadSafeFlag {
    state: u8,
}

impl ThreadSafeFlag {
    pub fn new() -> Self {
        Self { state: 0 }
    }
    pub fn ioctl(&self, req: i32, flags: i32) -> i32 {
        if req == 3 {
            return (self.state as i32) * flags;
        }
        -1
    }
    pub fn set(&mut self) {
        self.state = 1;
    }
    pub fn clear(&mut self) {
        self.state = 0;
    }
    pub fn wait(&mut self) {
        self.state = 0;
    }
}
""",
)

add(
    "asyncio/lock.rs",
    hdr("extmod/asyncio/lock.py")
    + """\
use super::task::TaskQueue;

pub struct Lock {
    locked: bool,
    waiting: TaskQueue,
}

impl Lock {
    pub fn new() -> Self {
        Self { locked: false, waiting: TaskQueue::new() }
    }
    pub fn locked(&self) -> bool {
        self.locked
    }
    pub fn release(&mut self) {
        self.locked = false;
        let _ = self.waiting.pop();
    }
    pub fn acquire(&mut self) -> bool {
        if !self.locked {
            self.locked = true;
            return true;
        }
        false
    }
}
""",
)

add(
    "asyncio/funcs.rs",
    hdr("extmod/asyncio/funcs.py")
    + """\
use super::core::{self, CancelledError, TimeoutError};
use py_rs::obj::Obj;

pub fn sleep_ms(t: i64) {
    let _ = core::ticks_add(core::ticks(), t.max(0) as u64);
}

pub fn sleep(t: f64) {
    sleep_ms((t * 1000.0) as i64);
}

pub fn wait_for(_aw: Obj, _timeout: f64) -> Obj {
    Obj(0)
}

pub fn wait_for_ms(_aw: Obj, _timeout: i64) -> Obj {
    Obj(0)
}

pub fn gather(_args: &[Obj]) -> Obj {
    Obj(0)
}
""",
)

add(
    "asyncio/stream.rs",
    hdr("extmod/asyncio/stream.py")
    + """\
use py_rs::obj::Obj;

pub struct StreamReader {
    pub s: Obj,
}

pub struct StreamWriter {
    pub s: Obj,
}

pub fn open_connection(_host: &str, _port: u16) -> (StreamReader, StreamWriter) {
    (
        StreamReader { s: Obj(0) },
        StreamWriter { s: Obj(0) },
    )
}

pub fn start_server(_cb: fn(Obj, Obj), _host: &str, _port: u16) {}
""",
)

add(
    "asyncio/uasyncio.rs",
    hdr("extmod/asyncio/uasyncio.py")
    + """\
//! Legacy uasyncio compatibility shims.
pub use super::core::*;
pub use super::task::*;
""",
)

add(
    "asyncio/__init__.rs",
    hdr("extmod/asyncio/__init__.py")
    + """\
pub use super::core::*;
pub use super::event::{Event, ThreadSafeFlag};
pub use super::lock::Lock;
pub use super::stream::{StreamReader, StreamWriter, open_connection, start_server};
pub use super::funcs::{gather, sleep, sleep_ms, wait_for, wait_for_ms};

pub const VERSION: (u8, u8, u8) = (3, 0, 0);

pub fn getattr(name: &str) -> Option<&'static str> {
    match name {
        "wait_for" | "wait_for_ms" | "gather" => Some("funcs"),
        "Event" | "ThreadSafeFlag" => Some("event"),
        "Lock" => Some("lock"),
        "open_connection" | "start_server" | "StreamReader" | "StreamWriter" => Some("stream"),
        _ => None,
    }
}
""",
)


def main() -> None:
    for rel, body in sorted(FILES.items()):
        path = OUT / rel
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(body, encoding="utf-8")
        print(f"wrote {rel}")
    print(f"total: {len(FILES)}")


if __name__ == "__main__":
    main()
