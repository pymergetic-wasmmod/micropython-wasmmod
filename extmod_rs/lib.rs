//! MetalPython rewrite of MicroPython `extmod/`.
//! Shadow tree: `extmod_rs/`.
#![allow(
    dead_code,
    unused_imports,
    unused_variables,
    unused_mut,
    unused_assignments,
    unused_unsafe,
    non_snake_case,
    non_upper_case_globals,
    static_mut_refs,
    private_interfaces,
    unexpected_cfgs,
    clippy::all
)]

pub mod asyncio;
pub mod cyw43_config_common;
pub mod font_petme128_8x8;
pub mod hal_pin;
pub mod machine_adc;
pub mod machine_adc_block;
pub mod machine_bitstream;
pub mod machine_can;
pub mod machine_can_port;
pub mod machine_i2c;
pub mod machine_i2c_target;
pub mod machine_i2s;
pub mod machine_mem;
pub mod machine_pinbase;
pub mod machine_pulse;
pub mod machine_pwm;
pub mod machine_signal;
pub mod machine_spi;
pub mod machine_timer;
pub mod machine_uart;
pub mod machine_usb_device;
pub mod machine_wdt;
pub mod misc;
pub mod modasyncio;
pub mod modbinascii;
pub mod modbluetooth;
pub mod modbtree;
pub mod modcryptolib;
pub mod moddeflate;
pub mod modframebuf;
pub mod modhashlib;
pub mod modheapq;
pub mod modjson;
pub mod modlwip;
pub mod modmachine;
pub mod modmarshal;
pub mod modnetwork;
pub mod modonewire;
pub mod modopenamp;
pub mod modopenamp_remoteproc;
pub mod modopenamp_remoteproc_store;
pub mod modos;
pub mod modplatform;
pub mod modrandom;
pub mod modre;
pub mod modselect;
pub mod modsocket;
pub mod modtime;
pub mod modtls_axtls;
pub mod modtls_mbedtls;
pub mod moductypes;
pub mod modvfs;
pub mod modwebrepl;
pub mod modwebsocket;
pub mod mpbthci;
pub mod network_cyw43;
pub mod network_esp_hosted;
pub mod network_lwip;
pub mod network_ninaw10;
pub mod network_ppp_lwip;
pub mod network_wiznet5k;
pub mod os_dupterm;
pub mod pm;
pub mod re15;
pub mod vfs;
pub mod vfs_blockdev;
pub mod vfs_fat;
pub mod vfs_fat_diskio;
pub mod vfs_fat_file;
pub mod vfs_lfs;
pub mod vfs_lfs_diskio;
pub mod vfs_lfsx;
pub mod vfs_lfsx_file;
pub mod vfs_posix;
pub mod vfs_posix_file;
pub mod vfs_reader;
pub mod vfs_rom;
pub mod vfs_rom_file;
pub mod virtpin;
pub mod wasmmod;

/// Initialise extmod host services (VFS mount, import hooks, built-in modules).
pub fn init_host() {
    if py_rs::mpconfig::PY_VFS {
        vfs::init_host();
    }
    if py_rs::mpconfig::PY_SYS_STDFILES && py_rs::mpconfig::VFS_POSIX {
        vfs_posix_file::install_sys_stdfiles();
    }
    if py_rs::mpconfig::PY_OS {
        let _ = modos::init_module();
    }
    if py_rs::mpconfig::PY_TIME {
        let _ = modtime::init_module();
    }
    if py_rs::mpconfig::PY_PLATFORM {
        let _ = modplatform::init_module();
    }
    if py_rs::mpconfig::PY_JSON {
        let _ = modjson::init_module();
    }
    if py_rs::mpconfig::PY_HASHLIB {
        let _ = modhashlib::init_module();
    }
    if py_rs::mpconfig::PY_BINASCII {
        let _ = modbinascii::init_module();
    }
    if py_rs::mpconfig::PY_HEAPQ {
        let _ = modheapq::init_module();
    }
    if py_rs::mpconfig::PY_RANDOM {
        let _ = modrandom::init_module();
    }
    if py_rs::mpconfig::PY_SELECT {
        let _ = modselect::init_module();
    }
    if py_rs::mpconfig::PY_ASYNCIO {
        let _ = modasyncio::init_module();
    }
    if py_rs::mpconfig::PY_RE {
        let _ = modre::init_module();
    }
    if py_rs::mpconfig::PY_VFS {
        let _ = modvfs::init_module();
    }
    if py_rs::mpconfig::PY_MACHINE {
        let _ = modmachine::init_module();
    }
    if py_rs::mpconfig::PY_MACHINE_TIMER {
        machine_timer::init_host_service();
    }
    if py_rs::mpconfig::PY_UCTYPES {
        let _ = moductypes::init_module();
    }
    if py_rs::mpconfig::PY_MARSHAL {
        let _ = modmarshal::init_module();
    }
    if py_rs::mpconfig::PY_FRAMEBUF {
        let _ = modframebuf::init_module();
    }
    if py_rs::mpconfig::PY_CRYPTOLIB {
        let _ = modcryptolib::init_module();
    }
    if py_rs::mpconfig::PY_DEFLATE {
        let _ = moddeflate::init_module();
    }
    if py_rs::mpconfig::PY_SOCKET {
        let _ = modsocket::init_module();
    }
    if py_rs::mpconfig::PY_NETWORK {
        let _ = modnetwork::init_module();
    }
    if py_rs::mpconfig::PY_SSL && py_rs::mpconfig::SSL_MBEDTLS {
        let _ = modtls_mbedtls::init_module();
    }
    if py_rs::mpconfig::PY_WEBSOCKET {
        let _ = modwebsocket::init_module();
        let _ = modwebrepl::init_module();
    }
    if py_rs::mpconfig::PY_ONEWIRE {
        let _ = modonewire::init_module();
    }
    if py_rs::mpconfig::PY_BTREE {
        let _ = modbtree::init_module();
    }
    if py_rs::mpconfig::PY_WASM {
        let _ = wasmmod::wasmmod::init_module();
    }
    if py_rs::mpconfig::PY_BLUETOOTH {
        let _ = modbluetooth::init_module();
    }
    if py_rs::mpconfig::PY_LWIP {
        let _ = modlwip::init_module();
    }
    if py_rs::mpconfig::PY_OPENAMP {
        let _ = modopenamp::init_module();
    }
}
