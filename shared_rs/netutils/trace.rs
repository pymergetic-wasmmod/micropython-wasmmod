//! rewrite of shared/netutils/trace.c
// symmetry: done

use py_rs::mphal;
use py_rs::mpprint::{self, Print, VaArg};

use super::netutils::{TRACE_IS_TX, TRACE_NEWLINE, TRACE_PAYLOAD};

fn get_be16(buf: &[u8]) -> u32 {
    u32::from(buf[0]) << 8 | u32::from(buf[1])
}

fn get_be32(buf: &[u8]) -> u32 {
    u32::from(buf[0]) << 24
        | u32::from(buf[1]) << 16
        | u32::from(buf[2]) << 8
        | u32::from(buf[3])
}

fn dump_hex_bytes(print: &Print, len: usize, buf: &[u8]) {
    for i in 0..len {
        mpprint::vprintf(print, " %02x", [VaArg::UInt(buf[i] as u32)]);
    }
}

fn ethertype_str(ty: u16) -> Option<&'static str> {
    match ty {
        0x0800 => Some("IPv4"),
        0x0806 => Some("ARP"),
        0x86dd => Some("IPv6"),
        _ => None,
    }
}

pub fn ethernet_trace(print: &Print, len: usize, buf: &[u8], flags: u32) {
    if buf.len() < 14 {
        return;
    }
    mpprint::vprintf(
        print,
        "[%8u] ETH%s len=%u",
        [
            VaArg::UInt(mphal::ticks_ms() as u32),
            VaArg::Str(if flags & TRACE_IS_TX != 0 { "T" } else { "R" }),
            VaArg::UInt(len as u32),
        ],
    );
    mpprint::vprintf(
        print,
        " dst=%02x:%02x:%02x:%02x:%02x:%02x",
        [
            VaArg::UInt(buf[0] as u32),
            VaArg::UInt(buf[1] as u32),
            VaArg::UInt(buf[2] as u32),
            VaArg::UInt(buf[3] as u32),
            VaArg::UInt(buf[4] as u32),
            VaArg::UInt(buf[5] as u32),
        ],
    );
    mpprint::vprintf(
        print,
        " src=%02x:%02x:%02x:%02x:%02x:%02x",
        [
            VaArg::UInt(buf[6] as u32),
            VaArg::UInt(buf[7] as u32),
            VaArg::UInt(buf[8] as u32),
            VaArg::UInt(buf[9] as u32),
            VaArg::UInt(buf[10] as u32),
            VaArg::UInt(buf[11] as u32),
        ],
    );

    let ethertype = (buf[12] as u16) << 8 | buf[13] as u16;
    if let Some(name) = ethertype_str(ethertype) {
        mpprint::vprintf(print, " type=%s", [VaArg::Str(name)]);
    } else {
        mpprint::vprintf(print, " type=0x%04x", [VaArg::UInt(ethertype as u32)]);
    }

    let mut len = len.saturating_sub(14);
    let mut payload = &buf[14..];
    if len > 0 && payload.len() >= 20 && buf[12] == 0x08 && buf[13] == 0x00 && payload[0] == 0x45 {
        len = get_be16(&payload[2..4]) as usize;
        mpprint::vprintf(
            print,
            " srcip=%u.%u.%u.%u dstip=%u.%u.%u.%u",
            [
                VaArg::UInt(payload[12] as u32),
                VaArg::UInt(payload[13] as u32),
                VaArg::UInt(payload[14] as u32),
                VaArg::UInt(payload[15] as u32),
                VaArg::UInt(payload[16] as u32),
                VaArg::UInt(payload[17] as u32),
                VaArg::UInt(payload[18] as u32),
                VaArg::UInt(payload[19] as u32),
            ],
        );
        let prot = payload[9];
        payload = &payload[20..];
        len = len.saturating_sub(20);
        if prot == 6 && payload.len() >= 20 {
            let srcport = get_be16(&payload[0..2]);
            let dstport = get_be16(&payload[2..4]);
            let seqnum = get_be32(&payload[4..8]);
            let acknum = get_be32(&payload[8..12]);
            let dataoff_flags = get_be16(&payload[12..14]);
            let winsz = get_be16(&payload[14..16]);
            mpprint::vprintf(
                print,
                " TCP srcport=%u dstport=%u seqnum=%u acknum=%u dataoff=%u flags=%x winsz=%u",
                [
                    VaArg::UInt(srcport),
                    VaArg::UInt(dstport),
                    VaArg::UInt(seqnum),
                    VaArg::UInt(acknum),
                    VaArg::UInt(dataoff_flags >> 12),
                    VaArg::UInt(dataoff_flags & 0x1ff),
                    VaArg::UInt(winsz),
                ],
            );
        } else if prot == 17 && payload.len() >= 8 {
            let srcport = get_be16(&payload[0..2]);
            let dstport = get_be16(&payload[2..4]);
            mpprint::vprintf(
                print,
                " UDP srcport=%u dstport=%u",
                [VaArg::UInt(srcport), VaArg::UInt(dstport)],
            );
        } else {
            mpprint::vprintf(print, " prot=%u", [VaArg::UInt(prot as u32)]);
        }
    }
    if flags & TRACE_PAYLOAD != 0 && !payload.is_empty() {
        mpprint::print_str(print, " data=");
        dump_hex_bytes(print, len.min(payload.len()), payload);
    }
    if flags & TRACE_NEWLINE != 0 {
        mpprint::print_str(print, "\n");
    }
}
