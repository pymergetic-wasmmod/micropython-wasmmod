//! rewrite of py/argcheck.c
// symmetry: done

use crate::map::{self, LookupKind, Map};
use crate::mpconfig;
use crate::obj::{self, Int, Obj};
use crate::qstr::Qstr;
use crate::raise::{self, MpRaise};

/// `mp_arg_flag_t` from py/runtime.h.
#[repr(u16)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ArgFlag {
    Bool = 0x001,
    Int = 0x002,
    Obj = 0x003,
    KindMask = 0x0ff,
    Required = 0x100,
    KwOnly = 0x200,
}

/// `mp_arg_val_t` from py/runtime.h.
#[derive(Copy, Clone, Debug)]
pub enum ArgVal {
    Bool(bool),
    Int(Int),
    Obj(Obj),
}

impl Default for ArgVal {
    fn default() -> Self {
        ArgVal::Obj(obj::OBJ_NULL)
    }
}

/// `mp_arg_t` from py/runtime.h.
#[derive(Copy, Clone, Debug)]
pub struct Arg {
    pub qst: Qstr,
    pub flags: u16,
    pub defval: ArgVal,
}

/// Build a function signature word (`MP_OBJ_FUN_MAKE_SIG`).
#[inline]
pub const fn make_sig(n_args_min: usize, n_args_max: usize, takes_kw: bool) -> u32 {
    let min = (n_args_min & 0xffff) as u32;
    let max = (n_args_max & 0xffff) as u32;
    (min << 17) | (max << 1) | if takes_kw { 1 } else { 0 }
}

/// `mp_arg_error_terse_mismatch`.
pub fn error_terse_mismatch() -> ! {
    raise::raise(MpRaise::TypeError("argument num/types mismatch"));
}

/// `mp_arg_error_unimpl_kw` (CPython compat).
#[cfg(any())]
pub fn error_unimpl_kw() -> ! {
    raise::raise(MpRaise::RuntimeError(
        "keyword argument(s) not implemented - use normal args instead",
    ));
}

/// Positional arg count check (`mp_arg_check_num`).
pub fn check_num(n_args: usize, n_kw: usize, n_args_min: usize, n_args_max: usize, takes_kw: bool) {
    check_num_sig(n_args, n_kw, make_sig(n_args_min, n_args_max, takes_kw));
}

/// Signature-based arg count check (`mp_arg_check_num_sig`).
pub fn check_num_sig(n_args: usize, n_kw: usize, sig: u32) {
    let takes_kw = (sig & 1) != 0;
    let n_args_min = (sig >> 17) as usize;
    let n_args_max = ((sig >> 1) & 0xffff) as usize;

    if n_kw != 0 && !takes_kw {
        if mpconfig::ERROR_REPORTING <= mpconfig::ERROR_REPORTING_TERSE as u8 {
            error_terse_mismatch();
        }
        raise::raise(MpRaise::TypeError(
            "function doesn't take keyword arguments",
        ));
    }

    if n_args_min == n_args_max {
        if n_args != n_args_min {
            if mpconfig::ERROR_REPORTING <= mpconfig::ERROR_REPORTING_TERSE as u8 {
                error_terse_mismatch();
            }
            raise::raise(MpRaise::TypeError("argument num/types mismatch"));
        }
    } else if n_args < n_args_min {
        if mpconfig::ERROR_REPORTING <= mpconfig::ERROR_REPORTING_TERSE as u8 {
            error_terse_mismatch();
        }
        raise::raise(MpRaise::TypeError("argument num/types mismatch"));
    } else if n_args > n_args_max {
        if mpconfig::ERROR_REPORTING <= mpconfig::ERROR_REPORTING_TERSE as u8 {
            error_terse_mismatch();
        }
        raise::raise(MpRaise::TypeError("argument num/types mismatch"));
    }
}

fn kind(flags: u16) -> u16 {
    flags & ArgFlag::KindMask as u16
}

/// Parse positional and keyword args (`mp_arg_parse_all`).
pub fn parse_all(
    n_pos: usize,
    pos: &[Obj],
    kws: &mut Map,
    n_allowed: usize,
    allowed: &[Arg],
    out_vals: &mut [ArgVal],
) {
    let mut pos_found = 0usize;
    let mut kws_found = 0usize;

    for i in 0..n_allowed {
        let given_arg = if i < n_pos {
            if allowed[i].flags & ArgFlag::KwOnly as u16 != 0 {
                if mpconfig::ERROR_REPORTING <= mpconfig::ERROR_REPORTING_TERSE as u8 {
                    error_terse_mismatch();
                }
                raise::raise(MpRaise::TypeError("extra positional arguments given"));
            }
            pos_found += 1;
            pos[i]
        } else if let Some(elem) =
            map::lookup(kws, obj::new_qstr(allowed[i].qst), LookupKind::Lookup)
        {
            kws_found += 1;
            elem.value
        } else {
            if allowed[i].flags & ArgFlag::Required as u16 != 0 {
                if mpconfig::ERROR_REPORTING <= mpconfig::ERROR_REPORTING_TERSE as u8 {
                    error_terse_mismatch();
                }
                raise::raise(MpRaise::TypeError("argument required"));
            }
            out_vals[i] = allowed[i].defval;
            continue;
        };

        out_vals[i] = match kind(allowed[i].flags) {
            x if x == ArgFlag::Bool as u16 => ArgVal::Bool(obj::is_true(given_arg)),
            x if x == ArgFlag::Int as u16 => ArgVal::Int(obj::get_int(given_arg)),
            _ => ArgVal::Obj(given_arg),
        };
    }

    if pos_found < n_pos {
        if mpconfig::ERROR_REPORTING <= mpconfig::ERROR_REPORTING_TERSE as u8 {
            error_terse_mismatch();
        }
        raise::raise(MpRaise::TypeError("extra positional arguments given"));
    }
    if kws_found < kws.used {
        if mpconfig::ERROR_REPORTING <= mpconfig::ERROR_REPORTING_TERSE as u8 {
            error_terse_mismatch();
        }
        raise::raise(MpRaise::TypeError("extra keyword arguments given"));
    }
}

/// Flat-array kwarg parse helper (`mp_arg_parse_all_kw_array`).
pub fn parse_all_kw_array(
    n_pos: usize,
    n_kw: usize,
    args: &[Obj],
    n_allowed: usize,
    allowed: &[Arg],
    out_vals: &mut [ArgVal],
) {
    let mut kw_args = Map::default();
    if n_kw != 0 {
        let start = n_pos;
        let end = start + n_kw * 2;
        let table: Vec<_> = args[start..end]
            .chunks(2)
            .map(|pair| map::MapElem {
                key: pair[0],
                value: pair[1],
            })
            .collect();
        map::init_fixed_table(&mut kw_args, table);
    }
    parse_all(n_pos, args, &mut kw_args, n_allowed, allowed, out_vals);
}
