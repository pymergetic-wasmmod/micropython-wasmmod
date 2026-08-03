//! rewrite of shared/memzip/import.c
// symmetry: done

use py_rs::builtinimport::{self, ImportStat};

use super::memzip::{self, MemzipResult};

/// `mp_import_stat` hook for memzip-backed imports.
pub fn import_stat(path: &str) -> ImportStat {
    match memzip::stat(path) {
        Ok(info) if info.is_dir != 0 => ImportStat::Dir,
        Ok(_) => ImportStat::File,
        Err(MemzipResult::NoFile) => ImportStat::NoExist,
        Err(_) => ImportStat::NoExist,
    }
}

/// Register the memzip import stat hook with the runtime.
pub fn register_import_hook() {
    builtinimport::set_import_stat_hook(import_stat);
}
