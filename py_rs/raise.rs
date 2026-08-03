//! MicroPython-style exception raising via NLR (`mp_raise_*` host path).
// symmetry: done

use crate::nlr;
use crate::obj::Obj;

/// Exception tag encoded in the low bits of an NLR jump value.
#[repr(u8)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum MpRaiseKind {
    TypeError = 1,
    ValueError = 2,
    RuntimeError = 3,
    OverflowError = 4,
    ZeroDivisionError = 5,
    OSError = 6,
    RecursionDepth = 7,
    SyntaxError = 8,
}

/// MicroPython exception payload carried by `nlr_jump`.
#[derive(Debug, Clone)]
pub enum MpRaise {
    TypeError(&'static str),
    ValueError(&'static str),
    RuntimeError(&'static str),
    OverflowError(&'static str),
    ZeroDivisionError,
    OSError(i32),
    AttributeError(&'static str),
    SyntaxError(&'static str),
    RecursionDepth,
}

impl MpRaise {
    fn kind(self) -> MpRaiseKind {
        match self {
            MpRaise::TypeError(_) => MpRaiseKind::TypeError,
            MpRaise::ValueError(_) => MpRaiseKind::ValueError,
            MpRaise::RuntimeError(_) => MpRaiseKind::RuntimeError,
            MpRaise::OverflowError(_) => MpRaiseKind::OverflowError,
            MpRaise::ZeroDivisionError => MpRaiseKind::ZeroDivisionError,
            MpRaise::OSError(_) => MpRaiseKind::OSError,
            MpRaise::AttributeError(_) => MpRaiseKind::TypeError,
            MpRaise::SyntaxError(_) => MpRaiseKind::SyntaxError,
            MpRaise::RecursionDepth => MpRaiseKind::RecursionDepth,
        }
    }

    pub fn message(self) -> Option<&'static str> {
        match self {
            MpRaise::TypeError(m)
            | MpRaise::ValueError(m)
            | MpRaise::RuntimeError(m)
            | MpRaise::OverflowError(m)
            | MpRaise::AttributeError(m)
            | MpRaise::SyntaxError(m) => Some(m),
            MpRaise::ZeroDivisionError => Some("divide by zero"),
            MpRaise::OSError(_) | MpRaise::RecursionDepth => None,
        }
    }
}

/// Pack a raise payload into the usize passed to `nlr::jump`.
pub fn encode(err: MpRaise) -> usize {
    let kind = err.clone().kind() as usize;
    match err {
        MpRaise::TypeError(msg) | MpRaise::ValueError(msg) | MpRaise::RuntimeError(msg) | MpRaise::OverflowError(msg) | MpRaise::AttributeError(msg) | MpRaise::SyntaxError(msg) => {
            ((msg.as_ptr() as usize) << 8) | kind
        }
        MpRaise::ZeroDivisionError | MpRaise::RecursionDepth => kind,
        MpRaise::OSError(code) => ((code as usize) << 8) | kind,
    }
}

fn decode_str(value: usize) -> &'static str {
    let ptr = (value >> 8) as *const u8;
    if ptr.is_null() {
        return "";
    }
    unsafe {
        let mut len = 0usize;
        while *ptr.add(len) != 0 {
            len += 1;
        }
        std::str::from_utf8_unchecked(std::slice::from_raw_parts(ptr, len))
    }
}

/// Decode an NLR jump value back into a raise payload.
pub fn decode(value: usize) -> MpRaise {
    let kind = (value & 0xff) as u8;
    match kind {
        x if x == MpRaiseKind::TypeError as u8 => MpRaise::TypeError(decode_str(value)),
        x if x == MpRaiseKind::ValueError as u8 => MpRaise::ValueError(decode_str(value)),
        x if x == MpRaiseKind::RuntimeError as u8 => MpRaise::RuntimeError(decode_str(value)),
        x if x == MpRaiseKind::OverflowError as u8 => MpRaise::OverflowError(decode_str(value)),
        x if x == MpRaiseKind::ZeroDivisionError as u8 => MpRaise::ZeroDivisionError,
        x if x == MpRaiseKind::OSError as u8 => MpRaise::OSError((value >> 8) as i32),
        x if x == MpRaiseKind::RecursionDepth as u8 => MpRaise::RecursionDepth,
        x if x == MpRaiseKind::SyntaxError as u8 => MpRaise::SyntaxError(decode_str(value)),
        _ => MpRaise::RuntimeError("unknown exception"),
    }
}

/// Raise a MicroPython exception (`mp_raise_*`); never returns.
pub fn raise(err: MpRaise) -> ! {
    nlr::jump(encode(err))
}

/// Raise using an exception object pointer (`mp_raise_obj`).
pub fn raise_obj(exc: Obj) -> ! {
    nlr::jump(exc.0)
}

/// Re-raise a decoded NLR payload.
pub fn reraise(value: usize) -> ! {
    nlr::jump(value)
}
