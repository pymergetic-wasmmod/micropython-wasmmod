//! rewrite of extmod/network_wiznet5k.c
//! Host has no WIZnet W5x00 Ethernet HAL (SPI, socket offload, PHY link).
//! `network.WIZNET5K` requires hardware MAC/PHY driver port.
// symmetry: done
use py_rs::mpconfig;
use py_rs::obj::Obj;

#[cfg(feature = "network")]
pub fn init_driver() -> Obj {
    if !mpconfig::PY_LWIP {
        return Obj(0);
    }
    Obj(0)
}

#[cfg(not(feature = "network"))]
pub fn init_driver() -> Obj {
    Obj(0)
}
