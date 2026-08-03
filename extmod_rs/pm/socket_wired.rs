//! Wired `pm_mpy_socket_*` accessors.
// symmetry: done

use super::socket::socket_export;
use py_rs::pm::mpy::pm_mpy_obj_t;

/// `pm_mpy_socket_socket` — return the `socket` export from `socket`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_socket_socket() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(socket_export("socket"))
}

/// `pm_mpy_socket_getaddrinfo` — return the `getaddrinfo` export from `socket`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_socket_getaddrinfo() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(socket_export("getaddrinfo"))
}

/// `pm_mpy_socket_inet_pton` — return the `inet_pton` export from `socket`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_socket_inet_pton() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(socket_export("inet_pton"))
}

/// `pm_mpy_socket_inet_ntop` — return the `inet_ntop` export from `socket`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_socket_inet_ntop() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(socket_export("inet_ntop"))
}

/// `pm_mpy_socket_sockaddr` — return the `sockaddr` export from `socket`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_socket_sockaddr() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(socket_export("sockaddr"))
}

/// `pm_mpy_socket_AF_UNIX` — return the `AF_UNIX` export from `socket`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_socket_AF_UNIX() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(socket_export("AF_UNIX"))
}

/// `pm_mpy_socket_AF_INET` — return the `AF_INET` export from `socket`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_socket_AF_INET() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(socket_export("AF_INET"))
}

/// `pm_mpy_socket_AF_INET6` — return the `AF_INET6` export from `socket`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_socket_AF_INET6() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(socket_export("AF_INET6"))
}

/// `pm_mpy_socket_SOCK_STREAM` — return the `SOCK_STREAM` export from `socket`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_socket_SOCK_STREAM() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(socket_export("SOCK_STREAM"))
}

/// `pm_mpy_socket_SOCK_DGRAM` — return the `SOCK_DGRAM` export from `socket`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_socket_SOCK_DGRAM() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(socket_export("SOCK_DGRAM"))
}

/// `pm_mpy_socket_SOCK_RAW` — return the `SOCK_RAW` export from `socket`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_socket_SOCK_RAW() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(socket_export("SOCK_RAW"))
}

/// `pm_mpy_socket_MSG_DONTROUTE` — return the `MSG_DONTROUTE` export from `socket`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_socket_MSG_DONTROUTE() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(socket_export("MSG_DONTROUTE"))
}

/// `pm_mpy_socket_MSG_DONTWAIT` — return the `MSG_DONTWAIT` export from `socket`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_socket_MSG_DONTWAIT() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(socket_export("MSG_DONTWAIT"))
}

/// `pm_mpy_socket_MSG_PEEK` — return the `MSG_PEEK` export from `socket`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_socket_MSG_PEEK() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(socket_export("MSG_PEEK"))
}

/// `pm_mpy_socket_SOL_SOCKET` — return the `SOL_SOCKET` export from `socket`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_socket_SOL_SOCKET() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(socket_export("SOL_SOCKET"))
}

/// `pm_mpy_socket_SO_BROADCAST` — return the `SO_BROADCAST` export from `socket`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_socket_SO_BROADCAST() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(socket_export("SO_BROADCAST"))
}

/// `pm_mpy_socket_SO_ERROR` — return the `SO_ERROR` export from `socket`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_socket_SO_ERROR() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(socket_export("SO_ERROR"))
}

/// `pm_mpy_socket_SO_KEEPALIVE` — return the `SO_KEEPALIVE` export from `socket`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_socket_SO_KEEPALIVE() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(socket_export("SO_KEEPALIVE"))
}

/// `pm_mpy_socket_SO_LINGER` — return the `SO_LINGER` export from `socket`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_socket_SO_LINGER() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(socket_export("SO_LINGER"))
}

/// `pm_mpy_socket_SO_REUSEADDR` — return the `SO_REUSEADDR` export from `socket`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_socket_SO_REUSEADDR() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(socket_export("SO_REUSEADDR"))
}

/// `pm_mpy_socket_IP_ADD_MEMBERSHIP` — return the `IP_ADD_MEMBERSHIP` export from `socket`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_socket_IP_ADD_MEMBERSHIP() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(socket_export("IP_ADD_MEMBERSHIP"))
}

/// `pm_mpy_socket_IP_DROP_MEMBERSHIP` — return the `IP_DROP_MEMBERSHIP` export from `socket`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_socket_IP_DROP_MEMBERSHIP() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(socket_export("IP_DROP_MEMBERSHIP"))
}

/// `pm_mpy_socket_SO_SNDTIMEO` — return the `SO_SNDTIMEO` export from `socket`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_socket_SO_SNDTIMEO() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(socket_export("SO_SNDTIMEO"))
}

/// `pm_mpy_socket_SO_RCVTIMEO` — return the `SO_RCVTIMEO` export from `socket`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_socket_SO_RCVTIMEO() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(socket_export("SO_RCVTIMEO"))
}

/// `pm_mpy_socket_IPPROTO_IP` — return the `IPPROTO_IP` export from `socket`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_socket_IPPROTO_IP() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(socket_export("IPPROTO_IP"))
}

/// `pm_mpy_socket_IPPROTO_ICMP` — return the `IPPROTO_ICMP` export from `socket`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_socket_IPPROTO_ICMP() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(socket_export("IPPROTO_ICMP"))
}

/// `pm_mpy_socket_IPPROTO_IPV4` — return the `IPPROTO_IPV4` export from `socket`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_socket_IPPROTO_IPV4() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(socket_export("IPPROTO_IPV4"))
}

/// `pm_mpy_socket_IPPROTO_TCP` — return the `IPPROTO_TCP` export from `socket`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_socket_IPPROTO_TCP() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(socket_export("IPPROTO_TCP"))
}

/// `pm_mpy_socket_IPPROTO_UDP` — return the `IPPROTO_UDP` export from `socket`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_socket_IPPROTO_UDP() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(socket_export("IPPROTO_UDP"))
}

/// `pm_mpy_socket_IPPROTO_IPV6` — return the `IPPROTO_IPV6` export from `socket`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_socket_IPPROTO_IPV6() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(socket_export("IPPROTO_IPV6"))
}

/// `pm_mpy_socket_IPPROTO_RAW` — return the `IPPROTO_RAW` export from `socket`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_socket_IPPROTO_RAW() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(socket_export("IPPROTO_RAW"))
}
