//! rewrite of extmod/network_lwip.c
// symmetry: gaps
// gaps:
// - needs lwIP netif HAL (link up/down, input/output, DHCP hooks)
// - `AbstractNIC` bindings require port TCP/IP stack and driver glue
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
