// symmetry: done
//! Faithful Rust port of MicroPython's `lib/re1.5` regex engine.

mod charclass;
mod compilecode;
mod dumpcode;
mod recursiveloop;

pub use charclass::{classmatch, namedclassmatch};
pub use compilecode::{compilecode, sizecode};
pub use dumpcode::dumpcode;
pub use recursiveloop::recursiveloopprog;
pub use types::{ByteProg, Subject, NON_ANCHORED_PREFIX};

pub mod types;
