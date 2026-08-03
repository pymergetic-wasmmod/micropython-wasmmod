//! rewrite of py/builtin.h
// symmetry: done

pub use crate::builtinimport::{builtin___import___default, import_stat, ImportStat};
pub use crate::modbuiltins::{init_builtins_module, PY_BUILTINS_HELP_TEXT};
pub use crate::opmethods::{op_contains_obj, op_delitem_obj, op_getitem_obj, op_setitem_obj};
pub use crate::stream::{
    stream___exit___obj, stream_close, stream_close_obj, stream_flush_obj, stream_ioctl_obj,
    stream_read1_obj, stream_read_obj, stream_readinto1_obj, stream_readinto_obj, stream_seek_obj,
    stream_tell_obj, stream_unbuffered_readline_obj, stream_unbuffered_readlines_obj,
    stream_write1_obj, stream_write_obj, StreamP, STREAM_ERROR, STREAM_OP_IOCTL, STREAM_OP_READ,
    STREAM_OP_WRITE,
};

use crate::obj::Obj;

/// Default `__import__` hook (ports may override via `mp_builtin___import__` macro in C).
#[inline]
pub fn builtin___import__(n_args: usize, args: &[Obj]) -> Obj {
    builtin___import___default(n_args, args)
}

/// Built-in `open` — delegated to VFS when enabled; host ports register via `set_builtin_open`.
pub type BuiltinOpenFn = fn(usize, &[Obj], Option<&mut crate::map::Map>) -> Obj;

static mut BUILTIN_OPEN: Option<BuiltinOpenFn> = None;

pub fn set_builtin_open(f: BuiltinOpenFn) {
    unsafe { BUILTIN_OPEN = Some(f) };
}

pub fn builtin_open(n_args: usize, args: &[Obj], kwargs: Option<&mut crate::map::Map>) -> Obj {
    unsafe {
        if let Some(f) = BUILTIN_OPEN {
            return f(n_args, args, kwargs);
        }
    }
    crate::raise::raise(crate::raise::MpRaise::RuntimeError("open not available"));
}
