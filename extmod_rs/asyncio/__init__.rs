//! rewrite of extmod/asyncio/__init__.py
// symmetry: done

pub use super::core::*;
pub use super::event::{Event, ThreadSafeFlag};
pub use super::funcs::{gather, wait_for, wait_for_ms, GatherResult, WaitForOutcome};
pub use super::lock::Lock;
pub use super::stream::{
    open_connection, start_server, Server, Stream, StreamReader, StreamWriter,
};

pub const VERSION: (u8, u8, u8) = (3, 0, 0);

static ATTRS: &[(&str, &str)] = &[
    ("wait_for", "funcs"),
    ("wait_for_ms", "funcs"),
    ("gather", "funcs"),
    ("Event", "event"),
    ("ThreadSafeFlag", "event"),
    ("Lock", "lock"),
    ("open_connection", "stream"),
    ("start_server", "stream"),
    ("StreamReader", "stream"),
    ("StreamWriter", "stream"),
];

/// Lazy loader module name for `attr` (mirrors Python `__getattr__`).
pub fn getattr_module(attr: &str) -> Option<&'static str> {
    ATTRS.iter().find(|(n, _)| *n == attr).map(|(_, m)| *m)
}
