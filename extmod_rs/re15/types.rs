// symmetry: done
//! Types and opcodes from `lib/re1.5/re1.5.h`.

pub const NON_ANCHORED_PREFIX: usize = 5;
pub const RE15_CLASS_NAMED_CLASS_INDICATOR: u8 = 0;

pub const CONSUMERS: u8 = 1;
pub const CHAR: u8 = CONSUMERS;
pub const ANY: u8 = 2;
pub const CLASS: u8 = 3;
pub const CLASS_NOT: u8 = 4;
pub const NAMED_CLASS: u8 = 5;

pub const ASSERTS: u8 = 0x50;
pub const BOL: u8 = ASSERTS;
pub const EOL: u8 = 0x51;

pub const JUMPS: u8 = 0x60;
pub const JMP: u8 = JUMPS;
pub const SPLIT: u8 = 0x61;
pub const RSPLIT: u8 = 0x62;

pub const SAVE: u8 = 0x7e;
pub const MATCH: u8 = 0x7f;

#[inline]
pub fn inst_is_consumer(inst: u8) -> bool {
    inst < ASSERTS
}

#[inline]
pub fn inst_is_jump(inst: u8) -> bool {
    inst & 0x70 == JUMPS
}

#[inline]
pub fn handle_anchored(bytecode: &[u8], is_anchored: bool) -> &[u8] {
    if is_anchored {
        &bytecode[NON_ANCHORED_PREFIX..]
    } else {
        bytecode
    }
}

/// Compiled regex bytecode (`ByteProg`).
#[derive(Clone, Debug, Default)]
pub struct ByteProg {
    pub bytelen: i32,
    pub len: i32,
    pub sub: i32,
    pub insts: Vec<u8>,
}

/// Match subject slice (`Subject`).
#[derive(Copy, Clone, Debug)]
pub struct Subject {
    pub begin_line: *const u8,
    pub begin: *const u8,
    pub end: *const u8,
}
