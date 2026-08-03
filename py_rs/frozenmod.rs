//! rewrite of py/frozenmod.c + py/frozenmod.h
// symmetry: done

use crate::builtinimport::ImportStat;
use crate::lexer::Lexer;
use crate::mpconfig;
use crate::qstr::{self, Qstr};
use crate::reader;

/// Frozen module kind (`MP_FROZEN_*`).
#[repr(u8)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum FrozenKind {
    None = 0,
    Str = 1,
    Mpy = 2,
}

/// Host-provided frozen module table (populated by the port / code generator).
static FROZEN_NAMES: std::sync::OnceLock<Vec<u8>> = std::sync::OnceLock::new();
static FROZEN_STR_SIZES: std::sync::OnceLock<Vec<u32>> = std::sync::OnceLock::new();
static FROZEN_STR_CONTENT: std::sync::OnceLock<Vec<u8>> = std::sync::OnceLock::new();

/// Register frozen module metadata from generated `frozen_content.c` equivalents.
pub fn register_frozen_modules(names: Vec<u8>, str_sizes: Vec<u32>, str_content: Vec<u8>) {
    let _ = FROZEN_NAMES.set(names);
    let _ = FROZEN_STR_SIZES.set(str_sizes);
    let _ = FROZEN_STR_CONTENT.set(str_content);
}

fn frozen_names() -> Option<&'static [u8]> {
    FROZEN_NAMES.get().map(|v| v.as_slice())
}

fn frozen_str_sizes() -> Option<&'static [u32]> {
    FROZEN_STR_SIZES.get().map(|v| v.as_slice())
}

fn frozen_str_content() -> Option<&'static [u8]> {
    FROZEN_STR_CONTENT.get().map(|v| v.as_slice())
}

/// `mp_find_frozen_module` — search `str` in the frozen name list.
pub fn find_frozen_module(
    module: &str,
    mut frozen_type: Option<&mut FrozenKind>,
    mut data: Option<&mut *mut ()>,
) -> ImportStat {
    if let Some(ft) = frozen_type.as_deref_mut() {
        *ft = FrozenKind::None;
    }
    if let Some(d) = data.as_deref_mut() {
        *d = core::ptr::null_mut();
    }

    if !mpconfig::MODULE_FROZEN {
        return ImportStat::NoExist;
    }

    let Some(names) = frozen_names() else {
        return ImportStat::NoExist;
    };

    let mut num_str = 0usize;
    if mpconfig::MODULE_FROZEN_STR && mpconfig::MODULE_FROZEN_MPY {
        if let Some(sizes) = frozen_str_sizes() {
            num_str = sizes.iter().filter(|&&s| s != 0).count();
        }
    }

    let query = module.as_bytes();
    let mut i = 0usize;
    let mut name_start = 0usize;
    while name_start < names.len() {
        let mut name_end = name_start;
        while name_end < names.len() && names[name_end] != 0 {
            name_end += 1;
        }
        if name_end == name_start {
            break;
        }
        let entry = &names[name_start..name_end];
        if query.len() <= entry.len() && &entry[..query.len()] == query {
            if query.len() == entry.len() {
                if let Some(ft) = frozen_type {
                    if mpconfig::MODULE_FROZEN_STR && i < num_str {
                        *ft = FrozenKind::Str;
                        if let (Some(data_out), Some(sizes), Some(content)) =
                            (data, frozen_str_sizes(), frozen_str_content())
                        {
                            let mut offset = 0usize;
                            for j in 0..i {
                                offset += sizes[j] as usize + 1;
                            }
                            let content_len = sizes[i] as usize;
                            let source = qstr::from_strn(module.as_bytes());
                            let lex = Lexer::new_from_str_len(
                                source,
                                &content[offset..offset + content_len],
                                reader::READER_IS_ROM,
                            );
                            *data_out = Box::into_raw(Box::new(lex)) as *mut ();
                        }
                    } else if mpconfig::MODULE_FROZEN_MPY && i >= num_str {
                        *ft = FrozenKind::Mpy;
                    }
                }
                return ImportStat::File;
            } else if entry.get(query.len()) == Some(&b'/') {
                return ImportStat::Dir;
            }
        }
        i += 1;
        name_start = name_end + 1;
    }

    ImportStat::NoExist
}

/// Release lexer allocated by `find_frozen_module` for string frozen modules.
pub fn release_frozen_str_data(data: *mut ()) {
    if !data.is_null() {
        unsafe {
            drop(Box::from_raw(data as *mut Lexer));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_without_module_frozen() {
        assert_eq!(
            find_frozen_module("foo", None, None),
            ImportStat::NoExist
        );
    }
}
