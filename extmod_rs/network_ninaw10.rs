//! rewrite of extmod/network_ninaw10.c
//! Host has no NINA-W10 WiFi module HAL (SPI, IRQ, firmware poll loop).
//! `network.NINAW10` STA/AP/scan require u-blox NINA driver port.
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
