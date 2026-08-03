//! rewrite of shared/memzip/lexermemzip.c
// symmetry: done

use py_rs::lexer::Lexer;
use py_rs::mperrno::ENOENT;
use py_rs::qstr::Qstr;
use py_rs::raise::{self, MpRaise};
use py_rs::reader::READER_IS_ROM;

use super::memzip::{self, MemzipResult};

/// `mp_lexer_new_from_file` — open a script from the embedded memzip archive.
pub fn lexer_new_from_file(filename: Qstr) -> Lexer {
    let path = py_rs::qstr::str_from_qstr(filename).unwrap_or_default();
    match memzip::locate(&path) {
        Ok((data, _len)) => Lexer::new_from_str_len(filename, data, READER_IS_ROM),
        Err(MemzipResult::NoFile) => raise::raise(MpRaise::OSError(ENOENT)),
        Err(_) => raise::raise(MpRaise::OSError(ENOENT)),
    }
}
