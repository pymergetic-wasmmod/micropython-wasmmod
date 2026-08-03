// symmetry: done
//! Port of `lib/re1.5/dumpcode.c` (debug builds only).

use py_rs::mpprint::{self, Print, VaArg};

use super::types::{
    ANY, BOL, CHAR, CLASS, CLASS_NOT, EOL, JMP, MATCH, NAMED_CLASS, RSPLIT, SAVE, SPLIT, ByteProg,
};

/// `re1_5_dumpcode`
pub fn dumpcode(prog: &ByteProg) {
    let print = &mpprint::PLAT_PRINT;
    let code = &prog.insts;
    let mut pc = 0usize;
    while pc < prog.bytelen as usize {
        let _ = mpprint::printf(print, "%2d: ", [VaArg::Int(pc as i32)]);
        match code[pc] {
            SPLIT => {
                pc += 1;
                let off = code[pc] as i8;
                let _ = mpprint::printf(
                    print,
                    "split %d (%d)\n",
                    [
                        VaArg::Int(pc as i32 + off as i32 + 1),
                        VaArg::Int(off as i32),
                    ],
                );
                pc += 1;
            }
            RSPLIT => {
                pc += 1;
                let off = code[pc] as i8;
                let _ = mpprint::printf(
                    print,
                    "rsplit %d (%d)\n",
                    [
                        VaArg::Int(pc as i32 + off as i32 + 1),
                        VaArg::Int(off as i32),
                    ],
                );
                pc += 1;
            }
            JMP => {
                pc += 1;
                let off = code[pc] as i8;
                let _ = mpprint::printf(
                    print,
                    "jmp %d (%d)\n",
                    [
                        VaArg::Int(pc as i32 + off as i32 + 1),
                        VaArg::Int(off as i32),
                    ],
                );
                pc += 1;
            }
            CHAR => {
                pc += 1;
                let _ = mpprint::printf(print, "char %c\n", [VaArg::Int(code[pc] as i32)]);
                pc += 1;
            }
            ANY => {
                let _ = mpprint::print_str(print, "any\n");
                pc += 1;
            }
            CLASS | CLASS_NOT => {
                let num = code[pc] as i32;
                let _ = mpprint::printf(
                    print,
                    "class%s %d",
                    [
                        VaArg::Str(if code[pc - 1] == CLASS_NOT {
                            "not"
                        } else {
                            ""
                        }),
                        VaArg::Int(num),
                    ],
                );
                pc += 1;
                let mut n = num;
                while n > 0 {
                    let _ = mpprint::printf(
                        print,
                        " 0x%02x-0x%02x",
                        [VaArg::Int(code[pc] as i32), VaArg::Int(code[pc + 1] as i32)],
                    );
                    pc += 2;
                    n -= 1;
                }
                let _ = mpprint::print_str(print, "\n");
            }
            NAMED_CLASS => {
                pc += 1;
                let _ = mpprint::printf(print, "namedclass %c\n", [VaArg::Int(code[pc] as i32)]);
                pc += 1;
            }
            MATCH => {
                let _ = mpprint::print_str(print, "match\n");
                pc += 1;
            }
            SAVE => {
                pc += 1;
                let _ = mpprint::printf(print, "save %d\n", [VaArg::Int(code[pc] as i32)]);
                pc += 1;
            }
            BOL => {
                let _ = mpprint::print_str(print, "assert bol\n");
                pc += 1;
            }
            EOL => {
                let _ = mpprint::print_str(print, "assert eol\n");
                pc += 1;
            }
            _ => debug_assert!(false, "dumpcode: bad opcode"),
        }
    }
    let _ = mpprint::printf(
        print,
        "Bytes: %d, insts: %d\n",
        [VaArg::Int(prog.bytelen), VaArg::Int(prog.len)],
    );
}
