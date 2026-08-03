/* micropython-rust public runtime façade — C ABI entry points (`pm_mpy_*`).
 * Rust mirror: `py_rs/pm/mpy/` (`pm::mpy::*`).
 * Include as: `#include <micropython-rust/pm_mpy.h>`
 */
#ifndef MICROPYTHON_RUST_PM_MPY_H
#define MICROPYTHON_RUST_PM_MPY_H

#include "pm_mpy_types.h"

#ifdef __cplusplus
extern "C" {
#endif

/* --- runtime -------------------------------------------------------------- */

pm_mpy_status_t pm_mpy_runtime_init(void);
pm_mpy_status_t pm_mpy_runtime_deinit(void);
pm_mpy_status_t pm_mpy_runtime_call(pm_mpy_obj_t fun, pm_mpy_obj_t arg, pm_mpy_obj_t *out);
pm_mpy_status_t pm_mpy_runtime_call_n_kw(
    pm_mpy_obj_t fun,
    size_t n_args,
    size_t n_kw,
    const pm_mpy_obj_t *args,
    pm_mpy_obj_t *out);
pm_mpy_status_t pm_mpy_runtime_eval(const char *src, pm_mpy_obj_t *out);
pm_mpy_status_t pm_mpy_runtime_exec(const char *src);
pm_mpy_status_t pm_mpy_runtime_execfile(const char *path);

/* --- modules -------------------------------------------------------------- */

pm_mpy_module_t pm_mpy_module_new(const char *name);
pm_mpy_status_t pm_mpy_module_register(const char *name, pm_mpy_module_t module);
pm_mpy_module_t pm_mpy_module_get(const char *name);
pm_mpy_status_t pm_mpy_module_set_attr(pm_mpy_module_t module, pm_mpy_qstr_t attr, pm_mpy_obj_t value);
pm_mpy_status_t pm_mpy_module_get_attr(pm_mpy_module_t module, pm_mpy_qstr_t attr, pm_mpy_obj_t *out);
pm_mpy_obj_t pm_mpy_module_globals(pm_mpy_module_t module);
pm_mpy_module_t pm_mpy_module_from_map(
    const char *name,
    const char *const *keys,
    const pm_mpy_obj_t *values,
    size_t n);

/* --- import --------------------------------------------------------------- */

pm_mpy_obj_t pm_mpy_import_import_module(const char *name);
pm_mpy_status_t pm_mpy_import___import__(size_t n_args, const pm_mpy_obj_t *args, pm_mpy_obj_t *out);
int pm_mpy_import_stat(const char *path);

/* --- objects -------------------------------------------------------------- */

pm_mpy_obj_t pm_mpy_obj_new_int(int64_t value);
pm_mpy_obj_t pm_mpy_obj_new_str(const uint8_t *data, size_t len);
pm_mpy_obj_t pm_mpy_obj_new_bytes(const uint8_t *data, size_t len);
pm_mpy_obj_t pm_mpy_obj_new_bool(bool value);
pm_mpy_obj_t pm_mpy_obj_new_list(size_t n, const pm_mpy_obj_t *items);
pm_mpy_obj_t pm_mpy_obj_new_dict(size_t n);
pm_mpy_obj_t pm_mpy_obj_new_tuple(size_t n, const pm_mpy_obj_t *items);
pm_mpy_status_t pm_mpy_obj_getattr(pm_mpy_obj_t base, pm_mpy_qstr_t attr, pm_mpy_obj_t *out);
pm_mpy_status_t pm_mpy_obj_setattr(pm_mpy_obj_t base, pm_mpy_qstr_t attr, pm_mpy_obj_t value);

/* --- qstr / lookup / exceptions ------------------------------------------- */

pm_mpy_qstr_t pm_mpy_qstr_from_str(const char *text);
pm_mpy_status_t pm_mpy_lookup(pm_mpy_obj_t map_obj, pm_mpy_obj_t index, pm_mpy_obj_t *out);
pm_mpy_status_t pm_mpy_exc_raise(pm_mpy_obj_t exc);
pm_mpy_status_t pm_mpy_exc_raise_type_msg(pm_mpy_status_t kind, const char *msg);

#ifdef __cplusplus
}
#endif

#endif /* MICROPYTHON_RUST_PM_MPY_H */
