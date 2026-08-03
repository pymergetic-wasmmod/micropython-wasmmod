//! rewrite of extmod/network_ppp_lwip.c
// symmetry: gaps
// gaps:
// - needs PPP modem/link HAL (UART or USB serial, lwIP PPPoS netif)
// - dial-up/data path requires port modem driver and lwIP PPP integration
use py_rs::mpconfig;
use py_rs::obj::Obj;

#[cfg(feature = "lwip")]
pub fn init_driver() -> Obj {
    if !mpconfig::PY_LWIP {
        return Obj(0);
    }
    Obj(0)
}

#[cfg(not(feature = "lwip"))]
pub fn init_driver() -> Obj {
    Obj(0)
}
