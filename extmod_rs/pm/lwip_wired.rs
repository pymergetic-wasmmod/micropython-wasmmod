//! Wired `pm_mpy_lwip_*` accessors.
// symmetry: done

use super::lwip::lwip_export;
use py_rs::pm::mpy::pm_mpy_obj_t;

/// `pm_mpy_lwip_reset` — return the `reset` export from `lwip`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_lwip_reset() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(lwip_export("reset"))
}

/// `pm_mpy_lwip_callback` — return the `callback` export from `lwip`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_lwip_callback() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(lwip_export("callback"))
}

/// `pm_mpy_lwip_getaddrinfo` — return the `getaddrinfo` export from `lwip`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_lwip_getaddrinfo() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(lwip_export("getaddrinfo"))
}

/// `pm_mpy_lwip_print_pcbs` — return the `print_pcbs` export from `lwip`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_lwip_print_pcbs() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(lwip_export("print_pcbs"))
}

/// `pm_mpy_lwip_socket` — return the `socket` export from `lwip`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_lwip_socket() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(lwip_export("socket"))
}

/// `pm_mpy_lwip_slip` — return the `slip` export from `lwip`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_lwip_slip() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(lwip_export("slip"))
}

/// `pm_mpy_lwip_AF_INET` — return the `AF_INET` export from `lwip`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_lwip_AF_INET() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(lwip_export("AF_INET"))
}

/// `pm_mpy_lwip_AF_INET6` — return the `AF_INET6` export from `lwip`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_lwip_AF_INET6() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(lwip_export("AF_INET6"))
}

/// `pm_mpy_lwip_SOCK_STREAM` — return the `SOCK_STREAM` export from `lwip`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_lwip_SOCK_STREAM() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(lwip_export("SOCK_STREAM"))
}

/// `pm_mpy_lwip_SOCK_DGRAM` — return the `SOCK_DGRAM` export from `lwip`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_lwip_SOCK_DGRAM() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(lwip_export("SOCK_DGRAM"))
}

/// `pm_mpy_lwip_SOCK_RAW` — return the `SOCK_RAW` export from `lwip`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_lwip_SOCK_RAW() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(lwip_export("SOCK_RAW"))
}

/// `pm_mpy_lwip_SOL_SOCKET` — return the `SOL_SOCKET` export from `lwip`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_lwip_SOL_SOCKET() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(lwip_export("SOL_SOCKET"))
}

/// `pm_mpy_lwip_SO_REUSEADDR` — return the `SO_REUSEADDR` export from `lwip`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_lwip_SO_REUSEADDR() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(lwip_export("SO_REUSEADDR"))
}

/// `pm_mpy_lwip_SO_BROADCAST` — return the `SO_BROADCAST` export from `lwip`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_lwip_SO_BROADCAST() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(lwip_export("SO_BROADCAST"))
}

/// `pm_mpy_lwip_IPPROTO_IP` — return the `IPPROTO_IP` export from `lwip`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_lwip_IPPROTO_IP() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(lwip_export("IPPROTO_IP"))
}

/// `pm_mpy_lwip_IP_ADD_MEMBERSHIP` — return the `IP_ADD_MEMBERSHIP` export from `lwip`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_lwip_IP_ADD_MEMBERSHIP() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(lwip_export("IP_ADD_MEMBERSHIP"))
}

/// `pm_mpy_lwip_IP_DROP_MEMBERSHIP` — return the `IP_DROP_MEMBERSHIP` export from `lwip`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_lwip_IP_DROP_MEMBERSHIP() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(lwip_export("IP_DROP_MEMBERSHIP"))
}

/// `pm_mpy_lwip_IPPROTO_TCP` — return the `IPPROTO_TCP` export from `lwip`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_lwip_IPPROTO_TCP() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(lwip_export("IPPROTO_TCP"))
}

/// `pm_mpy_lwip_TCP_NODELAY` — return the `TCP_NODELAY` export from `lwip`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_lwip_TCP_NODELAY() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(lwip_export("TCP_NODELAY"))
}

/// `pm_mpy_lwip_MSG_PEEK` — return the `MSG_PEEK` export from `lwip`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_lwip_MSG_PEEK() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(lwip_export("MSG_PEEK"))
}

/// `pm_mpy_lwip_MSG_DONTWAIT` — return the `MSG_DONTWAIT` export from `lwip`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_lwip_MSG_DONTWAIT() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(lwip_export("MSG_DONTWAIT"))
}
