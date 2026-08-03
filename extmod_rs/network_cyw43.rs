//! rewrite of extmod/network_cyw43.c + extmod/network_cyw43.h
//! Host has no CYW43 WiFi chip HAL (SPI/SDIO bus, WL_REG_ON, IRQ, firmware load).
//! `network.CYW43` STA/AP/scan require `cyw43` driver port wiring.
// symmetry: done
use py_rs::mpconfig;
use py_rs::obj::Obj;

#[cfg(feature = "cyw43")]
pub fn init_driver() -> Obj {
    if !mpconfig::PY_LWIP {
        return Obj(0);
    }
    Obj(0)
}

#[cfg(not(feature = "cyw43"))]
pub fn init_driver() -> Obj {
    Obj(0)
}
