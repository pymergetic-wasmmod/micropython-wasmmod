//! MicroPython-style exception raising via NLR (`mp_raise_*` host path).
// symmetry: done

use crate::nlr;
use crate::obj::Obj;

/// Exception tag encoded in the low bits of an NLR jump value.
///
/// All kind tags must be odd so `(encode(...) & 3) != 0`. That way encoded
/// payloads never look like REPR_A heap objects (`is_obj` requires low bits 0),
/// which previously let SyntaxError/OverflowError encodings segfault when
/// mistaken for exception instances.
#[repr(u8)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum MpRaiseKind {
    TypeError = 1,
    ValueError = 3,
    RuntimeError = 5,
    OverflowError = 7,
    ZeroDivisionError = 9,
    OSError = 11,
    RecursionDepth = 13,
    SyntaxError = 15,
    NameError = 17,
    AttributeError = 19,
    KeyError = 21,
    IndexError = 23,
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
    NameError(&'static str),
    KeyError(&'static str),
    IndexError(&'static str),
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
            MpRaise::AttributeError(_) => MpRaiseKind::AttributeError,
            MpRaise::SyntaxError(_) => MpRaiseKind::SyntaxError,
            MpRaise::NameError(_) => MpRaiseKind::NameError,
            MpRaise::KeyError(_) => MpRaiseKind::KeyError,
            MpRaise::IndexError(_) => MpRaiseKind::IndexError,
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
            | MpRaise::SyntaxError(m)
            | MpRaise::NameError(m)
            | MpRaise::KeyError(m)
            | MpRaise::IndexError(m) => Some(m),
            MpRaise::ZeroDivisionError => Some("divide by zero"),
            MpRaise::OSError(_) | MpRaise::RecursionDepth => None,
        }
    }
}

/// Pack a raise payload into the usize passed to `nlr::jump`.
pub fn encode(err: MpRaise) -> usize {
    let kind = err.clone().kind() as usize;
    match err {
        MpRaise::TypeError(msg)
        | MpRaise::ValueError(msg)
        | MpRaise::RuntimeError(msg)
        | MpRaise::OverflowError(msg)
        | MpRaise::AttributeError(msg)
        | MpRaise::SyntaxError(msg)
        | MpRaise::NameError(msg)
        | MpRaise::KeyError(msg)
        | MpRaise::IndexError(msg) => {
            ((msg.as_ptr() as usize) << 16) | ((msg.len().min(255)) << 8) | kind
        }
        MpRaise::ZeroDivisionError | MpRaise::RecursionDepth => kind,
        MpRaise::OSError(code) => ((code as usize) << 8) | kind,
    }
}

fn decode_str(value: usize) -> &'static str {
    let len = (value >> 8) & 0xff;
    let ptr = (value >> 16) as *const u8;
    if ptr.is_null() || len == 0 {
        return "";
    }
    unsafe { std::str::from_utf8_unchecked(std::slice::from_raw_parts(ptr, len)) }
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
        x if x == MpRaiseKind::NameError as u8 => MpRaise::NameError(decode_str(value)),
        x if x == MpRaiseKind::AttributeError as u8 => MpRaise::AttributeError(decode_str(value)),
        x if x == MpRaiseKind::KeyError as u8 => MpRaise::KeyError(decode_str(value)),
        x if x == MpRaiseKind::IndexError as u8 => MpRaise::IndexError(decode_str(value)),
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

#[cfg(test)]
mod tests {
    use super::*;

    fn roundtrip(err: MpRaise, check_msg: Option<&str>) {
        let encoded = encode(err.clone());
        let decoded = decode(encoded);
        match (err, decoded.clone()) {
            (
                MpRaise::TypeError(m)
                | MpRaise::ValueError(m)
                | MpRaise::RuntimeError(m)
                | MpRaise::OverflowError(m)
                | MpRaise::AttributeError(m)
                | MpRaise::SyntaxError(m)
                | MpRaise::NameError(m)
                | MpRaise::KeyError(m)
                | MpRaise::IndexError(m),
                MpRaise::TypeError(d)
                | MpRaise::ValueError(d)
                | MpRaise::RuntimeError(d)
                | MpRaise::OverflowError(d)
                | MpRaise::AttributeError(d)
                | MpRaise::SyntaxError(d)
                | MpRaise::NameError(d)
                | MpRaise::KeyError(d)
                | MpRaise::IndexError(d),
            ) => assert_eq!(m, d),
            (MpRaise::ZeroDivisionError, MpRaise::ZeroDivisionError) => {}
            (MpRaise::RecursionDepth, MpRaise::RecursionDepth) => {}
            (MpRaise::OSError(c), MpRaise::OSError(d)) => assert_eq!(c, d),
            _ => panic!("roundtrip kind mismatch"),
        }
        if let Some(msg) = check_msg {
            assert_eq!(decoded.message(), Some(msg));
        }
    }

    #[test]
    fn encode_decode_static_messages() {
        roundtrip(MpRaise::ValueError("1"), Some("1"));
        roundtrip(MpRaise::TypeError("bad type"), Some("bad type"));
        roundtrip(
            MpRaise::RuntimeError("name not defined"),
            Some("name not defined"),
        );
        roundtrip(MpRaise::ZeroDivisionError, Some("divide by zero"));
        roundtrip(MpRaise::OSError(28), None);
        roundtrip(MpRaise::RecursionDepth, None);
    }

    #[test]
    fn encode_kinds_never_look_like_heap_objects() {
        // REPR_A: is_obj when (val & 3) == 0. Encoded MpRaise must never match.
        for kind in [
            MpRaiseKind::TypeError,
            MpRaiseKind::ValueError,
            MpRaiseKind::RuntimeError,
            MpRaiseKind::OverflowError,
            MpRaiseKind::ZeroDivisionError,
            MpRaiseKind::OSError,
            MpRaiseKind::RecursionDepth,
            MpRaiseKind::SyntaxError,
            MpRaiseKind::NameError,
            MpRaiseKind::AttributeError,
            MpRaiseKind::KeyError,
            MpRaiseKind::IndexError,
        ] {
            let encoded = encode_kind(kind, 0);
            assert_ne!(
                encoded & 3,
                0,
                "{kind:?} encode looks like a heap object: {encoded:#x}"
            );
        }
    }

    fn encode_kind(kind: MpRaiseKind, msg_bits: usize) -> usize {
        (msg_bits << 8) | kind as usize
    }

    #[test]
    fn encode_decode_attribute_key_index_errors() {
        roundtrip(
            MpRaise::AttributeError("no such attribute"),
            Some("no such attribute"),
        );
        roundtrip(MpRaise::KeyError("missing"), Some("missing"));
        roundtrip(
            MpRaise::IndexError("index out of range"),
            Some("index out of range"),
        );
        roundtrip(
            MpRaise::NameError("name not defined"),
            Some("name not defined"),
        );
    }

    #[test]
    fn decode_hand_crafted_value_error() {
        let msg = "1";
        let encoded = ((msg.as_ptr() as usize) << 16)
            | ((msg.len().min(255)) << 8)
            | MpRaiseKind::ValueError as usize;
        assert!(matches!(decode(encoded), MpRaise::ValueError("1")));
    }
}
