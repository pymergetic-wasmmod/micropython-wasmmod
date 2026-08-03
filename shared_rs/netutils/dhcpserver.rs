//! rewrite of shared/netutils/dhcpserver.c + shared/netutils/dhcpserver.h
// symmetry: done

use py_rs::mpconfig;

pub const BASE_IP: u8 = 16;
pub const MAX_IP: usize = 8;

#[derive(Copy, Clone, Debug, Default)]
pub struct DhcpServerLease {
    pub mac: [u8; 6],
    pub expiry: u16,
}

#[repr(C)]
pub struct DhcpServer {
    pub ip: [u8; 4],
    pub nm: [u8; 4],
    pub lease: [DhcpServerLease; MAX_IP],
    pub udp: *mut (),
    pub send_router: bool,
}

impl Default for DhcpServer {
    fn default() -> Self {
        Self {
            ip: [0; 4],
            nm: [0; 4],
            lease: [DhcpServerLease::default(); MAX_IP],
            udp: core::ptr::null_mut(),
            send_router: true,
        }
    }
}

/// `dhcp_server_init`.
pub fn init(d: &mut DhcpServer, ip: [u8; 4], nm: [u8; 4]) {
    if !mpconfig::PY_LWIP {
        return;
    }
    d.ip = ip;
    d.nm = nm;
    d.lease = [DhcpServerLease::default(); MAX_IP];
    d.send_router = true;
    d.udp = core::ptr::null_mut();
    lwip_init_server(d);
}

/// `dhcp_server_deinit`.
pub fn deinit(d: &mut DhcpServer) {
    if !mpconfig::PY_LWIP {
        return;
    }
    lwip_deinit_server(d);
}

#[cfg(feature = "lwip")]
fn lwip_init_server(_d: &mut DhcpServer) {}

#[cfg(feature = "lwip")]
fn lwip_deinit_server(_d: &mut DhcpServer) {}

#[cfg(not(feature = "lwip"))]
fn lwip_init_server(_d: &mut DhcpServer) {}

#[cfg(not(feature = "lwip"))]
fn lwip_deinit_server(d: &mut DhcpServer) {
    d.udp = core::ptr::null_mut();
}
