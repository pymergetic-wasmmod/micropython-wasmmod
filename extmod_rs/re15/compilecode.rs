// symmetry: done
//! Port of `lib/re1.5/compilecode.c`.

use py_rs::cstack;

use super::types::{
    ByteProg, ANY, BOL, CHAR, CLASS, CLASS_NOT, EOL, JMP, MATCH, NAMED_CLASS, NON_ANCHORED_PREFIX,
    RE15_CLASS_NAMED_CLASS_INDICATOR, RSPLIT, SAVE, SPLIT,
};

fn match_named_class_char(c: u8) -> bool {
    (c | 0x20) == b'd' || (c | 0x24) == b'w'
}

fn emit_byte(prog: &mut ByteProg, at: usize, val: u8, sizecode: bool) {
    if !sizecode {
        prog.insts[at] = val;
    }
}

fn emit_checked(prog: &mut ByteProg, at: usize, val: i32, err: &mut bool, sizecode: bool) {
    *err |= val != val as i8 as i32;
    emit_byte(prog, at, val as u8, sizecode);
}

fn insert_code(prog: &mut ByteProg, at: usize, num: usize, sizecode: bool) {
    let pc = prog.bytelen as usize;
    if !sizecode {
        prog.insts.copy_within(at..pc, at + num);
    }
    prog.bytelen = (pc + num) as i32;
}

fn rel(at: usize, to: usize) -> i32 {
    (to as i32) - (at as i32) - 2
}

fn compile_code(bytes: &[u8], prog: &mut ByteProg, sizecode: bool) -> Option<usize> {
    let mut err = false;
    let start = prog.bytelen as usize;
    let mut term = prog.bytelen as usize;
    let mut alt_label = 0usize;
    let mut i = 0usize;

    cstack::check();

    while i < bytes.len() && bytes[i] != b')' {
        let ch = bytes[i];
        i += 1;
        match ch {
            b'\\' => {
                if i >= bytes.len() {
                    return None;
                }
                let esc = bytes[i];
                i += 1;
                if match_named_class_char(esc) {
                    term = prog.bytelen as usize;
                    let pc = prog.bytelen as usize;
                    prog.bytelen += 1;
                    emit_byte(prog, pc, NAMED_CLASS, sizecode);
                    prog.bytelen += 1;
                    emit_byte(prog, pc + 1, esc, sizecode);
                    prog.len += 1;
                } else {
                    term = prog.bytelen as usize;
                    let pc = prog.bytelen as usize;
                    prog.bytelen += 1;
                    emit_byte(prog, pc, CHAR, sizecode);
                    prog.bytelen += 1;
                    emit_byte(prog, pc + 1, esc, sizecode);
                    prog.len += 1;
                }
            }
            b'.' => {
                term = prog.bytelen as usize;
                let pc = prog.bytelen as usize;
                prog.bytelen += 1;
                emit_byte(prog, pc, ANY, sizecode);
                prog.len += 1;
            }
            b'[' => {
                term = prog.bytelen as usize;
                let class_pc = prog.bytelen as usize;
                prog.bytelen += 1;
                if i < bytes.len() && bytes[i] == b'^' {
                    emit_byte(prog, class_pc, CLASS_NOT, sizecode);
                    i += 1;
                } else {
                    emit_byte(prog, class_pc, CLASS, sizecode);
                }
                prog.bytelen += 1;
                prog.len += 1;
                let mut cnt = 0i32;
                while i < bytes.len() && bytes[i] != b']' {
                    let mut c = bytes[i];
                    i += 1;
                    if c == b'\\' {
                        if i >= bytes.len() {
                            return None;
                        }
                        c = bytes[i];
                        i += 1;
                        if match_named_class_char(c) {
                            let pc = prog.bytelen as usize;
                            prog.bytelen += 1;
                            emit_byte(prog, pc, RE15_CLASS_NAMED_CLASS_INDICATOR, sizecode);
                            prog.bytelen += 1;
                            emit_byte(prog, pc + 1, c, sizecode);
                            cnt += 1;
                            continue;
                        }
                    }
                    if i + 1 < bytes.len() && bytes[i] == b'-' && bytes[i + 1] != b']' {
                        i += 2;
                    }
                    let pc = prog.bytelen as usize;
                    prog.bytelen += 1;
                    emit_byte(prog, pc, c, sizecode);
                    prog.bytelen += 1;
                    emit_byte(prog, pc + 1, bytes[i - 1], sizecode);
                    cnt += 1;
                }
                if i >= bytes.len() {
                    return None;
                }
                i += 1;
                emit_checked(prog, term + 1, cnt, &mut err, sizecode);
            }
            b'(' => {
                term = prog.bytelen as usize;
                let mut sub = 0i32;
                let capture = !(i + 1 < bytes.len() && bytes[i] == b'?' && bytes[i + 1] == b':');
                if capture {
                    prog.sub += 1;
                    sub = prog.sub;
                    let pc = prog.bytelen as usize;
                    prog.bytelen += 1;
                    emit_byte(prog, pc, SAVE, sizecode);
                    prog.bytelen += 1;
                    emit_checked(prog, pc + 1, 2 * sub, &mut err, sizecode);
                    prog.len += 1;
                } else {
                    i += 2;
                }
                let inner_end = compile_code(&bytes[i..], prog, sizecode)?;
                i += inner_end;
                if i >= bytes.len() || bytes[i] != b')' {
                    return None;
                }
                i += 1;
                if capture {
                    let pc = prog.bytelen as usize;
                    prog.bytelen += 1;
                    emit_byte(prog, pc, SAVE, sizecode);
                    prog.bytelen += 1;
                    emit_checked(prog, pc + 1, 2 * sub + 1, &mut err, sizecode);
                    prog.len += 1;
                }
            }
            b'?' => {
                if prog.bytelen as usize == term {
                    return None;
                }
                insert_code(prog, term, 2, sizecode);
                if i < bytes.len() && bytes[i] == b'?' {
                    emit_byte(prog, term, RSPLIT, sizecode);
                    i += 1;
                } else {
                    emit_byte(prog, term, SPLIT, sizecode);
                }
                emit_checked(prog, term + 1, rel(term, prog.bytelen as usize), &mut err, sizecode);
                prog.len += 1;
                term = prog.bytelen as usize;
            }
            b'*' => {
                if prog.bytelen as usize == term {
                    return None;
                }
                insert_code(prog, term, 2, sizecode);
                let jmp_pc = prog.bytelen as usize;
                emit_byte(prog, jmp_pc, JMP, sizecode);
                emit_checked(prog, jmp_pc + 1, rel(jmp_pc, term), &mut err, sizecode);
                prog.bytelen += 2;
                if i < bytes.len() && bytes[i] == b'?' {
                    emit_byte(prog, term, RSPLIT, sizecode);
                    i += 1;
                } else {
                    emit_byte(prog, term, SPLIT, sizecode);
                }
                emit_checked(prog, term + 1, rel(term, prog.bytelen as usize), &mut err, sizecode);
                prog.len += 2;
                term = prog.bytelen as usize;
            }
            b'+' => {
                if prog.bytelen as usize == term {
                    return None;
                }
                let pc = prog.bytelen as usize;
                if i < bytes.len() && bytes[i] == b'?' {
                    emit_byte(prog, pc, SPLIT, sizecode);
                    i += 1;
                } else {
                    emit_byte(prog, pc, RSPLIT, sizecode);
                }
                emit_checked(prog, pc + 1, rel(pc, term), &mut err, sizecode);
                prog.bytelen = (pc + 2) as i32;
                prog.len += 1;
                term = prog.bytelen as usize;
            }
            b'|' => {
                if alt_label != 0 {
                    emit_checked(prog, alt_label, rel(alt_label, prog.bytelen as usize) + 1, &mut err, sizecode);
                }
                insert_code(prog, start, 2, sizecode);
                let jmp_pc = prog.bytelen as usize;
                emit_byte(prog, jmp_pc, JMP, sizecode);
                alt_label = jmp_pc + 1;
                prog.bytelen += 1;
                emit_byte(prog, start, SPLIT, sizecode);
                emit_checked(prog, start + 1, rel(start, prog.bytelen as usize), &mut err, sizecode);
                prog.len += 2;
                term = prog.bytelen as usize;
            }
            b'^' => {
                let pc = prog.bytelen as usize;
                prog.bytelen += 1;
                emit_byte(prog, pc, BOL, sizecode);
                prog.len += 1;
                term = prog.bytelen as usize;
            }
            b'$' => {
                let pc = prog.bytelen as usize;
                prog.bytelen += 1;
                emit_byte(prog, pc, EOL, sizecode);
                prog.len += 1;
                term = prog.bytelen as usize;
            }
            _ => {
                term = prog.bytelen as usize;
                let pc = prog.bytelen as usize;
                prog.bytelen += 1;
                emit_byte(prog, pc, CHAR, sizecode);
                prog.bytelen += 1;
                emit_byte(prog, pc + 1, ch, sizecode);
                prog.len += 1;
            }
        }
    }

    if alt_label != 0 {
        emit_checked(prog, alt_label, rel(alt_label, prog.bytelen as usize) + 1, &mut err, sizecode);
    }

    if err {
        return None;
    }
    Some(i)
}

/// `re1_5_sizecode`
pub fn sizecode(re: &str) -> i32 {
    let mut dummy = ByteProg {
        bytelen: (5 + NON_ANCHORED_PREFIX) as i32,
        len: 0,
        sub: 0,
        insts: Vec::new(),
    };
    if compile_code(re.as_bytes(), &mut dummy, true).is_none() {
        return -1;
    }
    dummy.bytelen
}

/// `re1_5_compilecode`
pub fn compilecode(prog: &mut ByteProg, re: &str) -> i32 {
    prog.len = 0;
    prog.bytelen = 0;
    prog.sub = 0;
    prog.insts.clear();

    let size = sizecode(re);
    if size < 0 {
        return 1;
    }
    prog.insts.resize(size as usize, 0);

    prog.insts[prog.bytelen as usize] = RSPLIT;
    prog.bytelen += 1;
    prog.insts[prog.bytelen as usize] = 3;
    prog.bytelen += 1;
    prog.insts[prog.bytelen as usize] = ANY;
    prog.bytelen += 1;
    prog.insts[prog.bytelen as usize] = JMP;
    prog.bytelen += 1;
    prog.insts[prog.bytelen as usize] = (-5i8) as u8;
    prog.bytelen += 1;
    prog.len += 3;

    prog.insts[prog.bytelen as usize] = SAVE;
    prog.bytelen += 1;
    prog.insts[prog.bytelen as usize] = 0;
    prog.bytelen += 1;
    prog.len += 1;

    let end = match compile_code(re.as_bytes(), prog, false) {
        Some(i) if i == re.len() => i,
        _ => return 1,
    };
    let _ = end;

    prog.insts[prog.bytelen as usize] = SAVE;
    prog.bytelen += 1;
    prog.insts[prog.bytelen as usize] = 1;
    prog.bytelen += 1;
    prog.len += 1;

    prog.insts[prog.bytelen as usize] = MATCH;
    prog.bytelen += 1;
    prog.len += 1;

    0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::re15::recursiveloop::recursiveloopprog;
    use crate::re15::types::Subject;

    fn compile_pat(re: &str) -> ByteProg {
        let mut prog = ByteProg::default();
        assert_eq!(compilecode(&mut prog, re), 0);
        prog
    }

    #[test]
    fn sizecode_rejects_bad_paren() {
        assert_eq!(sizecode("(abc"), -1);
    }

    #[test]
    fn match_literal() {
        let prog = compile_pat("foo");
        let hay = b"xxfooyy";
        let subj = Subject {
            begin_line: hay.as_ptr(),
            begin: hay.as_ptr(),
            end: unsafe { hay.as_ptr().add(hay.len()) },
        };
        let mut caps = [std::ptr::null(); 4];
        assert_eq!(recursiveloopprog(&prog, &subj, &mut caps, 4, false), 1);
        assert_eq!(unsafe { caps[1].offset_from(caps[0]) }, 3);
    }

    #[test]
    fn concat_plus() {
        let prog = compile_pat("ab+y");
        let hay = b"xxabbbyy";
        let subj = Subject {
            begin_line: hay.as_ptr(),
            begin: hay.as_ptr(),
            end: unsafe { hay.as_ptr().add(hay.len()) },
        };
        let mut caps = [std::ptr::null(); 4];
        assert_eq!(recursiveloopprog(&prog, &subj, &mut caps, 4, false), 1);
    }

    #[test]
    fn non_capturing_group() {
        let prog = compile_pat("a(?:b+)y");
        let hay = b"xxabbbyy";
        let subj = Subject {
            begin_line: hay.as_ptr(),
            begin: hay.as_ptr(),
            end: unsafe { hay.as_ptr().add(hay.len()) },
        };
        let mut caps = [std::ptr::null(); 4];
        assert_eq!(recursiveloopprog(&prog, &subj, &mut caps, 4, false), 1);
    }

    #[test]
    fn group_and_plus() {
        let prog = compile_pat("a(b+)y");
        let hay = b"xxabbbyy";
        let subj = Subject {
            begin_line: hay.as_ptr(),
            begin: hay.as_ptr(),
            end: unsafe { hay.as_ptr().add(hay.len()) },
        };
        let mut caps = [std::ptr::null(); 6];
        assert_eq!(recursiveloopprog(&prog, &subj, &mut caps, 6, false), 1);
    }

    #[test]
    fn plus_subpattern() {
        let prog = compile_pat("b+");
        let hay = b"xxabbbyy";
        let subj = Subject {
            begin_line: hay.as_ptr(),
            begin: hay.as_ptr(),
            end: unsafe { hay.as_ptr().add(hay.len()) },
        };
        let mut caps = [std::ptr::null(); 4];
        assert_eq!(recursiveloopprog(&prog, &subj, &mut caps, 4, false), 1);
    }

    #[test]
    fn anchored_match_requires_start() {
        let prog = compile_pat("^foo");
        let hay = b"xxfooyy";
        let subj = Subject {
            begin_line: hay.as_ptr(),
            begin: unsafe { hay.as_ptr().add(2) },
            end: unsafe { hay.as_ptr().add(hay.len()) },
        };
        let mut caps = [std::ptr::null(); 4];
        assert_eq!(recursiveloopprog(&prog, &subj, &mut caps, 4, true), 0);
        let hay2 = b"foox";
        let subj2 = Subject {
            begin_line: hay2.as_ptr(),
            begin: hay2.as_ptr(),
            end: unsafe { hay2.as_ptr().add(hay2.len()) },
        };
        assert_eq!(recursiveloopprog(&prog, &subj2, &mut caps, 4, true), 1);
    }
}
