//! C-ABI `pm_mpy_*` accessors for extmod built-in modules.
// symmetry: done

mod os;
mod os_wired;

mod time;
mod time_wired;

mod json;
mod json_wired;

mod select;
mod select_wired;

mod platform;
mod platform_wired;

mod hashlib;
mod hashlib_wired;

mod vfs;
mod vfs_wired;

mod random;
mod random_wired;

mod re;
mod re_wired;

mod heapq;
mod heapq_wired;

mod binascii;
mod binascii_wired;

mod uctypes;
mod uctypes_wired;

mod marshal;
mod marshal_wired;

mod deflate;
mod deflate_wired;

mod socket;
mod socket_wired;

mod framebuf;
mod framebuf_wired;

mod cryptolib;
mod cryptolib_wired;

mod machine;
mod machine_wired;

mod websocket;
mod websocket_wired;

mod onewire;
mod onewire_wired;

mod webrepl;
mod webrepl_wired;

mod asyncio;
mod asyncio_wired;

mod tls;
mod tls_wired;

mod btree;
mod btree_wired;

mod network;
mod network_wired;
