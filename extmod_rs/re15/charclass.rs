// symmetry: done
//! Port of `lib/re1.5/charclass.c`.

use super::types::{CLASS, RE15_CLASS_NAMED_CLASS_INDICATOR};

pub fn classmatch(pc: &[u8], sp: u8) -> bool {
    let is_positive = pc[0] == CLASS;
    let mut pc = &pc[1..];
    let mut cnt = pc[0] as usize;
    pc = &pc[1..];
    while cnt > 0 {
        if pc[0] == RE15_CLASS_NAMED_CLASS_INDICATOR {
            if namedclassmatch(pc[1], sp) == is_positive {
                return is_positive;
            }
        } else if sp >= pc[0] && sp <= pc[1] {
            return is_positive;
        }
        pc = &pc[2..];
        cnt -= 1;
    }
    !is_positive
}

pub fn namedclassmatch(pc: u8, sp: u8) -> bool {
    let mut off = ((pc >> 5) & 1) != 0;
    let cls = pc | 0x20;
    if cls == b'd' {
        if !(sp >= b'0' && sp <= b'9') {
            off = !off;
        }
    } else if cls == b's' {
        if !(sp == b' ' || (sp >= b'\t' && sp <= b'\r')) {
            off = !off;
        }
    } else if !(sp >= b'A' && sp <= b'Z')
        && !(sp >= b'a' && sp <= b'z')
        && !(sp >= b'0' && sp <= b'9')
        && sp != b'_'
    {
        off = !off;
    }
    off
}
