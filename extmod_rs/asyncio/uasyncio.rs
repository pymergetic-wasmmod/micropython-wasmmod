//! rewrite of extmod/asyncio/uasyncio.py
// symmetry: done
//! Legacy uasyncio compatibility shims — lazy-load from `asyncio`.

pub use super::__init__::*;

/// Re-export attribute from asyncio (mirrors Python `uasyncio.__getattr__`).
pub fn getattr(attr: &str) -> Option<&'static str> {
    super::__init__::getattr_module(attr).or_else(|| match attr {
        "Task" => Some("task"),
        "TaskQueue" => Some("task"),
        "sleep" | "sleep_ms" | "create_task" | "run" => Some("core"),
        _ => None,
    })
}
