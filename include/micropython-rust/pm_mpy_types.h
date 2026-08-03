/* micropython-rust public runtime façade — opaque ABI types.
 * Distinct from upstream MicroPython mp_* headers.
 * Include as: `#include <micropython-rust/pm_mpy_types.h>`
 */
#ifndef MICROPYTHON_RUST_PM_MPY_TYPES_H
#define MICROPYTHON_RUST_PM_MPY_TYPES_H

#include <stddef.h>
#include <stdint.h>
#include <stdbool.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct pm_mpy_obj pm_mpy_obj_t;
typedef struct pm_mpy_module pm_mpy_module_t;
typedef struct pm_mpy_qstr pm_mpy_qstr_t;

typedef enum pm_mpy_status {
    PM_MPY_OK = 0,
    PM_MPY_ERR = -1,
    PM_MPY_TYPE = -2,
    PM_MPY_VALUE = -3,
    PM_MPY_RUNTIME = -4,
} pm_mpy_status_t;

#ifdef __cplusplus
}
#endif

#endif /* MICROPYTHON_RUST_PM_MPY_TYPES_H */
