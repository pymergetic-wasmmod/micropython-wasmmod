//! Wired `pm_mpy_websocket_*` accessors.
// symmetry: done

use super::websocket::websocket_export;
use py_rs::pm::mpy::pm_mpy_obj_t;

#[no_mangle]
pub unsafe extern "C" fn pm_mpy_websocket_websocket() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(websocket_export("websocket"))
}
