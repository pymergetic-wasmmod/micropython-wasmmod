//! rewrite of extmod/network_esp_hosted.c
//! Host has no ESP-hosted coprocessor HAL (SPI/UART transport, handshake, reset).
//! WiFi control path requires ESP AT/hosted firmware port integration.
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
