//! rewrite of shared/netutils/netutils.c + shared/netutils/netutils.h
// symmetry: done

use py_rs::obj::{self, Obj, Uint};
use py_rs::objint;
use py_rs::objstr;
use py_rs::objtuple;
use py_rs::raise::{self, MpRaise};

pub const IPV4ADDR_BUFSIZE: usize = 4;

pub const TRACE_IS_TX: u32 = 0x0001;
pub const TRACE_PAYLOAD: u32 = 0x0002;
pub const TRACE_NEWLINE: u32 = 0x0004;

#[repr(u8)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Endian {
    Little = 0,
    Big = 1,
}

pub fn format_ipv4_addr(ip: &[u8; IPV4ADDR_BUFSIZE], endian: Endian) -> Obj {
    let s = match endian {
        Endian::Little => format!("{}.{}.{}.{}", ip[3], ip[2], ip[1], ip[0]),
        Endian::Big => format!("{}.{}.{}.{}", ip[0], ip[1], ip[2], ip[3]),
    };
    objstr::new_str(s.as_bytes())
}

pub fn format_inet_addr(ip: &[u8; IPV4ADDR_BUFSIZE], port: Uint, endian: Endian) -> Obj {
    let items = [format_ipv4_addr(ip, endian), obj::new_small_int(port as isize)];
    objtuple::new_tuple(2, Some(&items))
}

pub fn parse_ipv4_addr(addr: Obj, out_ip: &mut [u8; IPV4ADDR_BUFSIZE], endian: Endian) {
    let (addr_str, _) = objstr::str_get_data(addr);
    if addr_str.is_empty() {
        out_ip.fill(0);
        return;
    }

    let s_top = addr_str
        .iter()
        .position(|&c| c != b'.' && !c.is_ascii_digit())
        .unwrap_or(addr_str.len());
    let mut s = 0usize;
    for i in (0..4).rev() {
        let mut val = 0u8;
        while s < s_top && addr_str[s] != b'.' {
            val = val.wrapping_mul(10).wrapping_add(addr_str[s] - b'0');
            s += 1;
        }
        match endian {
            Endian::Little => out_ip[i] = val,
            Endian::Big => out_ip[IPV4ADDR_BUFSIZE - 1 - i] = val,
        }
        if i == 0 {
            if s == s_top {
                return;
            }
            raise::raise(MpRaise::ValueError("invalid arguments"));
        }
        if s < s_top && addr_str[s] == b'.' {
            s += 1;
        } else {
            raise::raise(MpRaise::ValueError("invalid arguments"));
        }
    }
}

pub fn parse_inet_addr(addr: Obj, out_ip: &mut [u8; IPV4ADDR_BUFSIZE], endian: Endian) -> Uint {
    let (count, items) = obj::get_array(addr);
    if count != 2 {
        raise::raise(MpRaise::ValueError("invalid arguments"));
    }
    parse_ipv4_addr(items[0], out_ip, endian);
    objint::int_get_truncated(items[1]) as Uint
}
