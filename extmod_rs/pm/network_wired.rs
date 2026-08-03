//! Wired `pm_mpy_network_*` accessors.
// symmetry: done

use super::network::network_export;
use py_rs::pm::mpy::pm_mpy_obj_t;

/// `pm_mpy_network_country` — return the `country` export from `network`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_network_country() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(network_export("country"))
}

/// `pm_mpy_network_hostname` — return the `hostname` export from `network`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_network_hostname() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(network_export("hostname"))
}

/// `pm_mpy_network_ipconfig` — return the `ipconfig` export from `network`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_network_ipconfig() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(network_export("ipconfig"))
}

/// `pm_mpy_network_PPP` — return the `PPP` export from `network`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_network_PPP() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(network_export("PPP"))
}

/// `pm_mpy_network_route` — return the `route` export from `network`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_network_route() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(network_export("route"))
}

/// `pm_mpy_network_WLAN` — return the `WLAN` export from `network`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_network_WLAN() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(network_export("WLAN"))
}

/// `pm_mpy_network_STAT_IDLE` — return the `STAT_IDLE` export from `network`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_network_STAT_IDLE() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(network_export("STAT_IDLE"))
}

/// `pm_mpy_network_STAT_CONNECTING` — return the `STAT_CONNECTING` export from `network`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_network_STAT_CONNECTING() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(network_export("STAT_CONNECTING"))
}

/// `pm_mpy_network_STAT_WRONG_PASSWORD` — return the `STAT_WRONG_PASSWORD` export from `network`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_network_STAT_WRONG_PASSWORD() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(network_export("STAT_WRONG_PASSWORD"))
}

/// `pm_mpy_network_STAT_NO_AP_FOUND` — return the `STAT_NO_AP_FOUND` export from `network`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_network_STAT_NO_AP_FOUND() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(network_export("STAT_NO_AP_FOUND"))
}

/// `pm_mpy_network_STAT_CONNECT_FAIL` — return the `STAT_CONNECT_FAIL` export from `network`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_network_STAT_CONNECT_FAIL() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(network_export("STAT_CONNECT_FAIL"))
}

/// `pm_mpy_network_STAT_GOT_IP` — return the `STAT_GOT_IP` export from `network`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_network_STAT_GOT_IP() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(network_export("STAT_GOT_IP"))
}

/// `pm_mpy_network_WIZNET5K` — return the `WIZNET5K` export from `network`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_network_WIZNET5K() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(network_export("WIZNET5K"))
}

/// `pm_mpy_network_STA_IF` — return the `STA_IF` export from `network`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_network_STA_IF() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(network_export("STA_IF"))
}

/// `pm_mpy_network_AP_IF` — return the `AP_IF` export from `network`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_network_AP_IF() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(network_export("AP_IF"))
}
