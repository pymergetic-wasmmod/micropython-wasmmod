//! rewrite of extmod/os_dupterm.c
// symmetry: done

use py_rs::malloc;
use py_rs::mpconfig;
use py_rs::mpprint::{self, PLAT_PRINT};
use py_rs::mpstate;
use py_rs::nlr::{self, NlrBuf};
use py_rs::obj::{self, Obj, ObjBase, ObjType, TYPE_FLAG_BUILTIN_FUN};
use py_rs::raise::{self, MpRaise};
use py_rs::scheduler;
use py_rs::stream::{self, StreamP, STREAM_ERROR, STREAM_OP_IOCTL, STREAM_OP_READ, STREAM_OP_WRITE, STREAM_POLL, STREAM_RW_WRITE};
use shared_rs::runtime::interrupt_char;

use crate::misc;

fn dupterm_enabled() -> bool {
    mpconfig::PY_OS_DUPTERM > 0
}

/// `mp_os_deactivate`
pub fn deactivate(idx: usize, msg: &str, exc: Obj) {
    if !dupterm_enabled() {
        return;
    }
    let term = mpstate::with_vm(|vm| {
        let prev = vm.dupterm_objs[idx];
        vm.dupterm_objs[idx] = obj::OBJ_NULL;
        prev
    });
    mpprint::print_str(&PLAT_PRINT, msg);
    if exc != obj::OBJ_NULL {
        obj::print_exception(&PLAT_PRINT, exc);
    }
    if term == obj::OBJ_NULL {
        return;
    }
    let mut nlr_buf = NlrBuf::default();
    let _ = nlr::protect(&mut nlr_buf, || stream::stream_close(term));
}

/// `mp_os_dupterm_poll`
pub fn poll(poll_flags: usize) -> usize {
    if !dupterm_enabled() {
        return 0;
    }
    let mut poll_flags_out = 0usize;
    mpstate::with_vm(|vm| {
        for idx in 0..mpconfig::PY_OS_DUPTERM {
            let s = vm.dupterm_objs[idx];
            if s == obj::OBJ_NULL {
                continue;
            }
            let stream_p = stream::get_stream(s);
            let ret = if mpconfig::PY_OS_DUPTERM_BUILTIN_STREAM
                && misc::os_dupterm_is_builtin_stream(s)
            {
                stream_p
                    .ioctl
                    .map(|ioctl| ioctl(s, STREAM_POLL, poll_flags, std::ptr::null_mut()))
                    .unwrap_or(STREAM_ERROR)
            } else {
                let mut nlr_buf = NlrBuf::default();
                match nlr::protect(&mut nlr_buf, || {
                    stream_p
                        .ioctl
                        .map(|ioctl| ioctl(s, STREAM_POLL, poll_flags, std::ptr::null_mut()))
                        .unwrap_or(STREAM_ERROR)
                }) {
                    Ok(v) => v,
                    Err(_) => continue,
                }
            };
            if ret != STREAM_ERROR {
                poll_flags_out |= ret;
                if poll_flags_out == poll_flags {
                    break;
                }
            }
        }
    });
    poll_flags_out
}

/// `mp_os_dupterm_rx_chr`
pub fn rx_chr() -> i32 {
    if !dupterm_enabled() {
        return -1;
    }
    if mpconfig::PY_OS_DUPTERM_NOTIFY {
        scheduler::sched_lock();
    }
    let mut ret = -1i32;
    mpstate::with_vm(|vm| {
        for idx in 0..mpconfig::PY_OS_DUPTERM {
            if vm.dupterm_objs[idx] == obj::OBJ_NULL {
                continue;
            }
            let s = vm.dupterm_objs[idx];
            if mpconfig::PY_OS_DUPTERM_BUILTIN_STREAM && misc::os_dupterm_is_builtin_stream(s) {
                let stream_p = stream::get_stream(s);
                let mut buf = [0u8; 1];
                let mut errcode = 0i32;
                let out_sz = stream_p
                    .read
                    .map(|read| read(s, buf.as_mut_ptr(), 1, &mut errcode))
                    .unwrap_or(STREAM_ERROR);
                if errcode == 0 && out_sz != 0 {
                    ret = buf[0] as i32;
                    break;
                }
                continue;
            }
            let mut nlr_buf = NlrBuf::default();
            match nlr::protect(&mut nlr_buf, || {
                let stream_p = stream::get_stream(s);
                let mut buf = [0u8; 1];
                let mut errcode = 0i32;
                let out_sz = stream_p
                    .read
                    .map(|read| read(s, buf.as_mut_ptr(), 1, &mut errcode))
                    .unwrap_or(STREAM_ERROR);
                (out_sz, errcode, buf[0])
            }) {
                Ok((out_sz, errcode, byte)) if out_sz == 0 => {
                    deactivate(idx, "dupterm: EOF received, deactivating\n", obj::OBJ_NULL);
                }
                Ok((out_sz, errcode, _)) if out_sz == STREAM_ERROR => {
                    if stream::is_nonblocking_error(errcode) {
                        continue;
                    }
                    raise::raise(MpRaise::OSError(errcode));
                }
                Ok((_, _, byte)) => {
                    ret = byte as i32;
                    if ret == interrupt_char::interrupt_char() {
                        scheduler::sched_keyboard_interrupt();
                        ret = -2;
                    }
                    break;
                }
                Err(exc) => {
                    deactivate(
                        idx,
                        "dupterm: Exception in read() method, deactivating: ",
                        obj::from_ptr(exc as *const ()),
                    );
                }
            }
        }
    });
    if mpconfig::PY_OS_DUPTERM_NOTIFY {
        scheduler::sched_unlock();
    }
    ret
}

/// `mp_os_dupterm_tx_strn`
pub fn tx_strn(s: &[u8], len: usize) -> i32 {
    if !dupterm_enabled() {
        return -1;
    }
    let mut ret = len as i32;
    let mut did_write = false;
    mpstate::with_vm(|vm| {
        for idx in 0..mpconfig::PY_OS_DUPTERM {
            if vm.dupterm_objs[idx] == obj::OBJ_NULL {
                continue;
            }
            did_write = true;
            let stream = vm.dupterm_objs[idx];
            if mpconfig::PY_OS_DUPTERM_BUILTIN_STREAM && misc::os_dupterm_is_builtin_stream(stream)
            {
                let stream_p = stream::get_stream(stream);
                let mut errcode = 0i32;
                let written = stream_p
                    .write
                    .map(|write| write(stream, s.as_ptr(), len, &mut errcode))
                    .unwrap_or(STREAM_ERROR);
                let write_res = (written as i32).max(0);
                ret = ret.min(write_res);
                continue;
            }
            let mut nlr_buf = NlrBuf::default();
            match nlr::protect(&mut nlr_buf, || {
                stream::stream_write(stream, &s[..len], STREAM_RW_WRITE)
            }) {
                Ok(written) => {
                    if written == obj::CONST_NONE {
                        ret = 0;
                    } else if obj::is_small_int(written) {
                        let written_int = obj::small_int_value(written).max(0);
                        ret = ret.min(written_int as i32);
                    }
                }
                Err(exc) => {
                    deactivate(
                        idx,
                        "dupterm: Exception in write() method, deactivating: ",
                        obj::from_ptr(exc as *const ()),
                    );
                    ret = 0;
                }
            }
        }
    });
    if did_write { ret } else { -1 }
}

type BuiltinFnVar = fn(usize, &[Obj]) -> Obj;

#[repr(C)]
struct ObjFunBuiltinVar {
    base: ObjBase,
    min_args: u8,
    max_args: u8,
    fun: BuiltinFnVar,
}

static mut FV: [*const (); 1] = [callv as *const ()];
static TV: ObjType = ObjType {
    base: ObjBase {
        type_: core::ptr::null(),
    },
    flags: TYPE_FLAG_BUILTIN_FUN,
    name: 0,
    slot_index_make_new: 0,
    slot_index_print: 0,
    slot_index_call: 1,
    slot_index_unary_op: 0,
    slot_index_binary_op: 0,
    slot_index_attr: 0,
    slot_index_subscr: 0,
    slot_index_iter: 0,
    slot_index_buffer: 0,
    slot_index_protocol: 0,
    slot_index_parent: 0,
    slot_index_locals_dict: 0,
    slots: unsafe { FV.as_ptr() },
};

fn callv(s: Obj, n: usize, k: usize, a: &[Obj]) -> Obj {
    let self_ = unsafe { &*(obj::as_ptr(s) as *const ObjFunBuiltinVar) };
    py_rs::argcheck::check_num(n, k, self_.min_args as usize, self_.max_args as usize, false);
    (self_.fun)(n, a)
}

fn mkv(min: u8, max: u8, f: BuiltinFnVar) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltinVar>().expect("dupterm fn");
    unsafe {
        (*o).base.type_ = &TV;
        (*o).min_args = min;
        (*o).max_args = max;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltinVar as *const ())
    }
}

fn os_dupterm(n: usize, args: &[Obj]) -> Obj {
    let mut idx = 0usize;
    if n == 2 {
        idx = obj::get_int(args[1]) as usize;
    }
    if idx >= mpconfig::PY_OS_DUPTERM {
        raise::raise(MpRaise::ValueError("invalid dupterm index"));
    }
    let previous_obj = mpstate::with_vm(|vm| {
        let prev = vm.dupterm_objs[idx];
        if args[0] == obj::CONST_NONE {
            vm.dupterm_objs[idx] = obj::OBJ_NULL;
        } else {
            let _ = stream::get_stream_raise(args[0], STREAM_OP_READ | STREAM_OP_WRITE | STREAM_OP_IOCTL);
            vm.dupterm_objs[idx] = args[0];
        }
        prev
    });
    let previous = if previous_obj == obj::OBJ_NULL {
        obj::CONST_NONE
    } else {
        previous_obj
    };
    misc::os_dupterm_stream_detached_attached(previous, args[0]);
    previous
}

static DUPTERM_INIT: std::sync::OnceLock<Obj> = std::sync::OnceLock::new();

/// `mp_os_dupterm_obj`
pub fn dupterm_obj() -> Obj {
    *DUPTERM_INIT.get_or_init(|| mkv(1, 2, os_dupterm))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_when_zero_slots() {
        assert!(!dupterm_enabled() || mpconfig::PY_OS_DUPTERM > 0);
    }
}
