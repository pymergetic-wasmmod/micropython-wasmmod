//! rewrite of extmod/misc.h
// symmetry: done

use py_rs::mpconfig;
use py_rs::obj::Obj;

use crate::os_dupterm;

/// `mp_os_dupterm_is_builtin_stream`
pub fn os_dupterm_is_builtin_stream(_stream: Obj) -> bool {
    false
}

/// `mp_os_dupterm_stream_detached_attached`
pub fn os_dupterm_stream_detached_attached(_detached: Obj, _attached: Obj) {}

/// `mp_os_dupterm_poll`
pub fn os_dupterm_poll(poll_flags: usize) -> usize {
    if mpconfig::PY_OS_DUPTERM > 0 {
        os_dupterm::poll(poll_flags)
    } else {
        poll_flags
    }
}

/// `mp_os_dupterm_rx_chr`
pub fn os_dupterm_rx_chr() -> i32 {
    if mpconfig::PY_OS_DUPTERM > 0 {
        os_dupterm::rx_chr()
    } else {
        -1
    }
}

/// `mp_os_dupterm_tx_strn`
pub fn os_dupterm_tx_strn(s: &[u8], len: usize) -> i32 {
    if mpconfig::PY_OS_DUPTERM > 0 {
        os_dupterm::tx_strn(s, len)
    } else {
        -1
    }
}

/// `mp_os_deactivate`
pub fn os_dupterm_deactivate(idx: usize, msg: &str, exc: Obj) {
    if mpconfig::PY_OS_DUPTERM > 0 {
        os_dupterm::deactivate(idx, msg, exc);
    }
}
