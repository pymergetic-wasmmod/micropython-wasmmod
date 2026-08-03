//! rewrite of extmod/mpbthci.c + extmod/mpbthci.h
//! Upstream `mpbthci.c` is empty; HCI UART/controller bindings live in port `mpbthciport` (unix: `ports_rs/unix/mpbthciport.rs`).
// symmetry: done

pub const HCI_READ_MODE_BYTE: u32 = 0;
pub const HCI_READ_MODE_PACKET: u32 = 1;
