// symmetry: done
//! Port of `lib/re1.5/recursiveloop.c`.

use py_rs::cstack;

use super::charclass;
use super::types::{
    inst_is_consumer, ANY, BOL, CHAR, CLASS, CLASS_NOT, EOL, JMP, MATCH, NAMED_CLASS, RSPLIT, SAVE,
    SPLIT, Subject,
};
use super::types::ByteProg;
use super::types::handle_anchored;

fn recursiveloop(
    pc: &mut usize,
    insts: &[u8],
    mut sp: *const u8,
    input: &Subject,
    subp: &mut [*const u8],
    nsubp: i32,
) -> i32 {
    cstack::check();

    loop {
        if inst_is_consumer(insts[*pc]) && sp >= input.end {
            return 0;
        }
        match insts[*pc] {
            CHAR => {
                *pc += 1;
                if unsafe { *sp } != insts[*pc] {
                    return 0;
                }
                *pc += 1;
                sp = unsafe { sp.add(1) };
            }
            ANY => {
                *pc += 1;
                sp = unsafe { sp.add(1) };
            }
            CLASS | CLASS_NOT => {
                let class_start = *pc;
                *pc += 1;
                if !charclass::classmatch(&insts[class_start..], unsafe { *sp }) {
                    return 0;
                }
                *pc += insts[*pc] as usize * 2 + 1;
                sp = unsafe { sp.add(1) };
            }
            NAMED_CLASS => {
                *pc += 1;
                if !charclass::namedclassmatch(insts[*pc], unsafe { *sp }) {
                    return 0;
                }
                *pc += 1;
                sp = unsafe { sp.add(1) };
            }
            MATCH => return 1,
            JMP => {
                let off = insts[*pc + 1] as i8;
                *pc = (*pc as i32 + 2 + off as i32) as usize;
            }
            SPLIT => {
                let base = *pc;
                let off = insts[base + 1] as i8;
                let mut save_pc = base + 2;
                if recursiveloop(&mut save_pc, insts, sp, input, subp, nsubp) != 0 {
                    return 1;
                }
                *pc = (base as i32 + 2 + off as i32) as usize;
            }
            RSPLIT => {
                let base = *pc;
                let off = insts[base + 1] as i8;
                let mut branch_pc = (base as i32 + 2 + off as i32) as usize;
                if recursiveloop(&mut branch_pc, insts, sp, input, subp, nsubp) != 0 {
                    return 1;
                }
                *pc = base + 2;
            }
            SAVE => {
                *pc += 1;
                let off = insts[*pc] as usize;
                *pc += 1;
                if (off as i32) >= nsubp {
                    continue;
                }
                let old = subp[off];
                subp[off] = sp;
                if recursiveloop(pc, insts, sp, input, subp, nsubp) != 0 {
                    return 1;
                }
                subp[off] = old;
                return 0;
            }
            BOL => {
                *pc += 1;
                if sp != input.begin_line {
                    return 0;
                }
            }
            EOL => {
                *pc += 1;
                if sp != input.end {
                    return 0;
                }
            }
            _ => debug_assert!(false, "recursiveloop: bad opcode"),
        }
    }
}

/// `re1_5_recursiveloopprog`
pub fn recursiveloopprog(
    prog: &ByteProg,
    input: &Subject,
    subp: &mut [*const u8],
    nsubp: i32,
    is_anchored: bool,
) -> i32 {
    let insts = handle_anchored(&prog.insts, is_anchored);
    let mut pc = 0usize;
    recursiveloop(&mut pc, insts, input.begin, input, subp, nsubp)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::re15::compilecode::compilecode;

    #[test]
    fn search_finds_substring() {
        let mut prog = super::super::types::ByteProg::default();
        assert_eq!(compilecode(&mut prog, "bar"), 0);
        let hay = b"foobarbaz";
        let subj = Subject {
            begin_line: hay.as_ptr(),
            begin: hay.as_ptr(),
            end: unsafe { hay.as_ptr().add(hay.len()) },
        };
        let mut caps = [std::ptr::null(); 4];
        assert_eq!(recursiveloopprog(&prog, &subj, &mut caps, 4, false), 1);
        assert!(!caps[0].is_null());
    }
}
