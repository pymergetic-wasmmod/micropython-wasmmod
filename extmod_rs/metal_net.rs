//! Metal guest net FFI — binds MetalPython to metal's lwIP TCP/UDP faces.
//!
//! Host builds keep POSIX `modsocket` / stub `modlwip`. Enable with
//! `feature = "metal_net"` when linking against metal's
//! `pm_metal_net_ip_{tcp,udp}_*` providers. No NIC drivers live here;
//! virtio/bge L2 stays in metal.

#![allow(dead_code)]

/// Opaque metal TCP stream / listen handle (`uint32_t` in the C face).
pub type TcpHandle = u32;

/// Opaque metal UDP handle.
pub type UdpHandle = u32;

/// Invalid / closed handle sentinel used by metal faces (`0`).
pub const HANDLE_INVALID: u32 = 0;

#[cfg(feature = "metal_net")]
mod ffi {
    use super::TcpHandle;
    use core::ffi::c_char;

    #[repr(C)]
    pub struct TcpSslOpts {
        pub sni: *const c_char,
        pub insecure: i32,
        pub ca_pem: *const u8,
        pub ca_pem_len: u32,
    }

    /// Opaque lwIP `ip_addr_t` for FFI (layout owned by metal/lwIP).
    pub type IpAddr = [u8; 16];

    unsafe extern "C" {
        pub fn pm_metal_net_ip_tcp_connect(
            host: *const c_char,
            port: u16,
            ssl_opts: *const TcpSslOpts,
        ) -> TcpHandle;
        pub fn pm_metal_net_ip_tcp_listen(port: u16) -> TcpHandle;
        pub fn pm_metal_net_ip_tcp_accept(listen_h: TcpHandle, creds_h: u32) -> TcpHandle;
        pub fn pm_metal_net_ip_tcp_read(stream_h: TcpHandle, buf: *mut u8, cap: u32) -> u32;
        pub fn pm_metal_net_ip_tcp_write(stream_h: TcpHandle, buf: *const u8, len: u32) -> u32;
        pub fn pm_metal_net_ip_tcp_close(stream_h: TcpHandle);
        pub fn pm_metal_net_ip_tcp_try_read(stream_h: TcpHandle, buf: *mut u8, cap: u32) -> u32;
        pub fn pm_metal_net_ip_tcp_try_write(stream_h: TcpHandle, buf: *const u8, len: u32) -> u32;
        pub fn pm_metal_net_ip_tcp_listen_close(listen_h: TcpHandle);

        pub fn pm_metal_net_ip_udp_bind(port: u16) -> u32;
        pub fn pm_metal_net_ip_udp_close(sock_h: u32);
        pub fn pm_metal_net_ip_udp_sendto(
            sock_h: u32,
            buf: *const u8,
            len: u32,
            addr: *const IpAddr,
            port: u16,
        ) -> u32;
        pub fn pm_metal_net_ip_udp_recvfrom(
            sock_h: u32,
            buf: *mut u8,
            cap: u32,
            addr_out: *mut IpAddr,
            port_out: *mut u16,
        ) -> u32;

        pub fn pm_metal_net_ip_poll();
        pub fn pm_metal_net_ip_if_count() -> u32;
        pub fn pm_metal_net_ip_if_dhcp_ready(
            ifname: *const c_char,
            ip_out: *mut c_char,
            ip_cap: u32,
        ) -> i32;
        pub fn pm_metal_net_ip_if_status_index(index: u32, dest: *mut c_char, dest_cap: u32)
            -> i32;
    }
}

/// Whether this build links metal's net faces.
#[inline]
pub fn metal_net_enabled() -> bool {
    cfg!(feature = "metal_net")
}

#[cfg(feature = "metal_net")]
pub mod tcp {
    use super::ffi;
    use super::{TcpHandle, HANDLE_INVALID};
    use std::ffi::CString;

    pub fn connect(host: &str, port: u16) -> Option<TcpHandle> {
        let c = CString::new(host).ok()?;
        let h = unsafe { ffi::pm_metal_net_ip_tcp_connect(c.as_ptr(), port, core::ptr::null()) };
        if h == HANDLE_INVALID {
            None
        } else {
            Some(h)
        }
    }

    pub fn listen(port: u16) -> Option<TcpHandle> {
        let h = unsafe { ffi::pm_metal_net_ip_tcp_listen(port) };
        if h == HANDLE_INVALID {
            None
        } else {
            Some(h)
        }
    }

    pub fn accept(listen_h: TcpHandle) -> Option<TcpHandle> {
        let h = unsafe { ffi::pm_metal_net_ip_tcp_accept(listen_h, 0) };
        if h == HANDLE_INVALID {
            None
        } else {
            Some(h)
        }
    }

    pub fn read(stream_h: TcpHandle, buf: &mut [u8]) -> u32 {
        unsafe { ffi::pm_metal_net_ip_tcp_read(stream_h, buf.as_mut_ptr(), buf.len() as u32) }
    }

    pub fn write(stream_h: TcpHandle, buf: &[u8]) -> u32 {
        unsafe { ffi::pm_metal_net_ip_tcp_write(stream_h, buf.as_ptr(), buf.len() as u32) }
    }

    pub fn try_read(stream_h: TcpHandle, buf: &mut [u8]) -> u32 {
        unsafe { ffi::pm_metal_net_ip_tcp_try_read(stream_h, buf.as_mut_ptr(), buf.len() as u32) }
    }

    pub fn try_write(stream_h: TcpHandle, buf: &[u8]) -> u32 {
        unsafe { ffi::pm_metal_net_ip_tcp_try_write(stream_h, buf.as_ptr(), buf.len() as u32) }
    }

    pub fn close(stream_h: TcpHandle) {
        unsafe { ffi::pm_metal_net_ip_tcp_close(stream_h) }
    }

    pub fn listen_close(listen_h: TcpHandle) {
        unsafe { ffi::pm_metal_net_ip_tcp_listen_close(listen_h) }
    }
}

#[cfg(feature = "metal_net")]
pub mod status {
    use super::ffi;
    use std::ffi::CString;

    pub fn poll() {
        unsafe { ffi::pm_metal_net_ip_poll() }
    }

    pub fn if_count() -> u32 {
        unsafe { ffi::pm_metal_net_ip_if_count() }
    }

    /// Returns DHCP-assigned IPv4 string when ready (`r == 1`).
    pub fn dhcp_ready(ifname: &str) -> Option<String> {
        let c = CString::new(ifname).ok()?;
        let mut ip = [0i8; 16];
        let r = unsafe {
            ffi::pm_metal_net_ip_if_dhcp_ready(c.as_ptr(), ip.as_mut_ptr(), ip.len() as u32)
        };
        if r == 1 {
            let bytes = ip
                .iter()
                .map(|&b| b as u8)
                .take_while(|&b| b != 0)
                .collect::<Vec<_>>();
            String::from_utf8(bytes).ok()
        } else {
            None
        }
    }
}
