//! Wired `pm_mpy_asyncio_*` accessors.
// symmetry: done

use super::asyncio::asyncio_export;
use py_rs::pm::mpy::pm_mpy_obj_t;

/// `pm_mpy_asyncio_TaskQueue` — return the `TaskQueue` export from `_asyncio`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_asyncio_TaskQueue() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(asyncio_export("TaskQueue"))
}

/// `pm_mpy_asyncio_Task` — return the `Task` export from `_asyncio`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_asyncio_Task() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(asyncio_export("Task"))
}
