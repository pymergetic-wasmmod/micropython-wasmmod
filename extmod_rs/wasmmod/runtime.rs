//! rewrite of extmod/wasmmod/runtime.c + extmod/wasmmod/runtime.h
//!
//! Remaining gaps:
//! - Full WAMR-parity forwarder caching; basic registry dispatch works via wasmi linker
//! - AOT / JIT execution modes
// symmetry: done

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{LazyLock, Mutex, OnceLock};

use py_rs::mpconfig;
use wasmi::{
    Caller, Engine, Error, Extern, ExternType, FuncType, Instance, Linker, Memory, Module, Ref,
    Store, Val, ValType,
};

use super::pack::{HOST_MODULE, WASM_MODULE};

pub const MP_WASM_NAME_MAX: usize = 255;
pub const MP_WASM_ERRBUF: usize = 128;

/// Minimal wasm module exporting `add(i32,i32)->i32` for tests.
pub const TEST_ADD_WASM: &[u8] = &[
    0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, 0x01, 0x07, 0x01, 0x60, 0x02, 0x7f, 0x7f, 0x01,
    0x7f, 0x03, 0x02, 0x01, 0x00, 0x07, 0x07, 0x01, 0x03, 0x61, 0x64, 0x64, 0x00, 0x00, 0x0a, 0x09,
    0x01, 0x07, 0x00, 0x20, 0x00, 0x20, 0x01, 0x6a, 0x0b,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WasmValKind {
    I32,
    I64,
    F32,
    F64,
}

struct ModuleExec {
    store: Store<()>,
    instance: Instance,
}

/// Loaded Wasm module: bytes + metadata + wasmi engine state.
pub struct WasmModule {
    name: [u8; MP_WASM_NAME_MAX + 1],
    name_len: usize,
    buf: Vec<u8>,
    meta: Vec<u8>,
    meta_owned: bool,
    compiled: Module,
    exec: Mutex<ModuleExec>,
}

impl WasmModule {
    fn set_name_fields(&mut self, name: &str) {
        self.name.fill(0);
        let n = name.len().min(MP_WASM_NAME_MAX);
        self.name[..n].copy_from_slice(&name.as_bytes()[..n]);
        self.name_len = n;
    }

    fn set_meta(&mut self, code: &[u8], meta: Option<&[u8]>) {
        if let Some(m) = meta {
            if m == code {
                self.meta = self.buf.clone();
            } else {
                self.meta = m.to_vec();
                self.meta_owned = true;
            }
        } else {
            self.meta = self.buf.clone();
        }
    }

    fn with_exec<R>(&self, f: impl FnOnce(&mut Store<()>, Instance) -> R) -> R {
        let mut exec = self.exec.lock().unwrap();
        let instance = exec.instance;
        f(&mut exec.store, instance)
    }

    fn with_exec_mut<R>(&self, f: impl FnOnce(&mut Store<()>, Instance) -> R) -> R {
        let mut exec = self.exec.lock().unwrap();
        let instance = exec.instance;
        f(&mut exec.store, instance)
    }
}

static ENGINE: LazyLock<Engine> = LazyLock::new(Engine::default);
static HOST_LINKER: LazyLock<Mutex<Linker<()>>> = LazyLock::new(|| {
    let mut linker = Linker::new(&ENGINE);
    register_host_imports(&mut linker);
    register_loader_imports(&mut linker);
    Mutex::new(linker)
});

static RUNTIME_READY: AtomicBool = AtomicBool::new(false);

/// Python→Wasm call by module id (registered from `modobj`).
pub type CallExportI32Fn = fn(u64, &str, &[i32], &mut i32) -> bool;
static CALL_EXPORT_I32: OnceLock<CallExportI32Fn> = OnceLock::new();

pub fn set_call_export_i32(f: CallExportI32Fn) {
    let _ = CALL_EXPORT_I32.set(f);
}

pub fn call_export_i32_named(pack: &str, func: &str, args: &[i32], out: &mut i32) -> bool {
    let Some(call) = CALL_EXPORT_I32.get() else {
        return false;
    };
    let Some(id) = super::forward::registry_mod_id(pack) else {
        return false;
    };
    call(id, func, args, out)
}

/// Guest→guest forward dispatch (set from `modobj` to avoid a module cycle).
pub type InvokeByNameFn = fn(&str, &str, &[Val], &mut [Val]) -> Result<(), ()>;
static INVOKE_BY_NAME: OnceLock<InvokeByNameFn> = OnceLock::new();

pub fn set_invoke_by_name(f: InvokeByNameFn) {
    let _ = INVOKE_BY_NAME.set(f);
}

pub fn set_err(errbuf: &mut [u8], msg: &str) {
    if errbuf.is_empty() {
        return;
    }
    let n = msg.len().min(errbuf.len() - 1);
    errbuf[..n].copy_from_slice(&msg.as_bytes()[..n]);
    errbuf[n] = 0;
}

fn val_type_to_kind(ty: ValType) -> Option<WasmValKind> {
    match ty {
        ValType::I32 => Some(WasmValKind::I32),
        ValType::I64 => Some(WasmValKind::I64),
        ValType::F32 => Some(WasmValKind::F32),
        ValType::F64 => Some(WasmValKind::F64),
        _ => None,
    }
}

fn kinds_from_func_type(ty: &FuncType) -> Option<(Vec<WasmValKind>, Vec<WasmValKind>)> {
    let params: Option<Vec<_>> = ty.params().iter().map(|&t| val_type_to_kind(t)).collect();
    let results: Option<Vec<_>> = ty.results().iter().map(|&t| val_type_to_kind(t)).collect();
    Some((params?, results?))
}

fn val_to_i32(v: &Val) -> i32 {
    match v {
        Val::I32(x) => *x,
        _ => 0,
    }
}

fn zero_results(results: &mut [Val]) {
    for r in results.iter_mut() {
        *r = match &*r {
            Val::I32(_) => Val::I32(0),
            Val::I64(_) => Val::I64(0),
            Val::F32(_) => Val::F32(0f32.into()),
            Val::F64(_) => Val::F64(0f64.into()),
            other => other.clone(),
        };
    }
}

fn caller_memory(caller: &Caller<'_, ()>) -> Option<Memory> {
    caller.get_export("memory").and_then(|e| match e {
        Extern::Memory(m) => Some(m),
        _ => None,
    })
}

pub(crate) fn caller_linear_read(
    caller: &Caller<'_, ()>,
    off: u32,
    n: u32,
    buf: &mut [u8],
) -> bool {
    if buf.len() != n as usize {
        return false;
    }
    if n == 0 {
        return true;
    }
    let Some(mem) = caller_memory(caller) else {
        return false;
    };
    mem.read(caller, off as usize, buf).is_ok()
}

pub(crate) fn caller_linear_write(caller: &mut Caller<'_, ()>, off: u32, data: &[u8]) -> bool {
    if data.is_empty() {
        return true;
    }
    let Some(mem) = caller_memory(caller) else {
        return false;
    };
    mem.write(caller, off as usize, data).is_ok()
}

fn guest_name<'a>(
    caller: &Caller<'_, ()>,
    off: i32,
    len: i32,
    buf: &'a mut [u8],
) -> Option<&'a str> {
    if len < 0 || len as usize >= buf.len() {
        return None;
    }
    if len > 0 && !caller_linear_read(caller, off as u32, len as u32, &mut buf[..len as usize]) {
        return None;
    }
    buf[len as usize] = 0;
    std::str::from_utf8(&buf[..len as usize]).ok()
}

fn register_host_imports(linker: &mut Linker<()>) {
    use super::host;
    let hm = HOST_MODULE;
    let _ = linker.func_wrap(hm, "call_i32", |_: Caller<'_, ()>, slot: i32, arg: i32| {
        host::call_slot_i32(slot, arg)
    });
    let _ = linker.func_wrap(hm, "call0_i32", |_: Caller<'_, ()>, slot: i32| {
        host::call_slot0_i32(slot)
    });
    let _ = linker.func_wrap(hm, "call_i64", |_: Caller<'_, ()>, slot: i32, arg: i64| {
        host::call_slot_i64(slot, arg)
    });
    let _ = linker.func_wrap(hm, "call_f32", |_: Caller<'_, ()>, slot: i32, arg: f32| {
        host::call_slot_f32(slot, arg)
    });
    let _ = linker.func_wrap(hm, "call_f64", |_: Caller<'_, ()>, slot: i32, arg: f64| {
        host::call_slot_f64(slot, arg)
    });
    let _ = linker.func_wrap(hm, "mem_alloc", |_: Caller<'_, ()>, size: i32| {
        host::mem_alloc(size as u32)
    });
    let _ = linker.func_wrap(hm, "mem_free", |_: Caller<'_, ()>, cookie: i32| {
        host::mem_free(cookie);
    });
    let _ = linker.func_wrap(hm, "mem_len", |_: Caller<'_, ()>, cookie: i32| {
        host::mem_len(cookie) as i32
    });
    let _ = linker.func_wrap(
        hm,
        "call_buf",
        |caller: Caller<'_, ()>, slot: i32, off: i32, len: i32| -> i32 {
            if len < 0 {
                return -1;
            }
            let mut buf = vec![0u8; len as usize];
            if len > 0 && !caller_linear_read(&caller, off as u32, len as u32, &mut buf) {
                return -1;
            }
            host::call_slot_buf(slot, &buf)
        },
    );
    let _ = linker.func_wrap(
        hm,
        "call_mem",
        |_: Caller<'_, ()>, slot: i32, cookie: i32| -> i32 { host::call_slot_mem(slot, cookie) },
    );
    let _ = linker.func_wrap(
        hm,
        "call_obj",
        |_: Caller<'_, ()>, slot: i32, handle: i32| -> i32 { host::call_slot_obj(slot, handle) },
    );
    let _ = linker.func_wrap(
        hm,
        "call0_py",
        |caller: Caller<'_, ()>, mod_off: i32, mod_len: i32, attr_off: i32, attr_len: i32| -> i32 {
            let mut mname = [0u8; host::HOST_NAME_MAX];
            let mut aname = [0u8; host::HOST_NAME_MAX];
            let Some(mod_name) = guest_name(&caller, mod_off, mod_len, &mut mname) else {
                return -1;
            };
            let Some(attr_name) = guest_name(&caller, attr_off, attr_len, &mut aname) else {
                return -1;
            };
            host::call0_py(mod_name, attr_name)
        },
    );
    let _ = linker.func_wrap(
        hm,
        "call_py",
        |caller: Caller<'_, ()>,
         mod_off: i32,
         mod_len: i32,
         attr_off: i32,
         attr_len: i32,
         arg: i32|
         -> i32 {
            let mut mname = [0u8; host::HOST_NAME_MAX];
            let mut aname = [0u8; host::HOST_NAME_MAX];
            let Some(mod_name) = guest_name(&caller, mod_off, mod_len, &mut mname) else {
                return -1;
            };
            let Some(attr_name) = guest_name(&caller, attr_off, attr_len, &mut aname) else {
                return -1;
            };
            host::call_py(mod_name, attr_name, arg)
        },
    );
    let _ = linker.func_wrap(
        hm,
        "mem_copy_in",
        |caller: Caller<'_, ()>, cookie: i32, src_off: i32, n: i32| -> i32 {
            if n < 0 {
                return -1;
            }
            let mut buf = vec![0u8; n as usize];
            if n > 0 && !caller_linear_read(&caller, src_off as u32, n as u32, &mut buf) {
                return -1;
            }
            host::mem_copy_in(cookie, &buf)
        },
    );
    let _ = linker.func_wrap(
        hm,
        "mem_copy_out",
        |mut caller: Caller<'_, ()>, cookie: i32, dest_off: i32, n: i32| -> i32 {
            if n < 0 {
                return -1;
            }
            let mut buf = vec![0u8; n as usize];
            if host::mem_copy_out(cookie, &mut buf) != 0 {
                return -1;
            }
            if n > 0 && !caller_linear_write(&mut caller, dest_off as u32, &buf) {
                return -1;
            }
            0
        },
    );
    let _ = linker.func_wrap(
        hm,
        "mem_copy_in_at",
        |caller: Caller<'_, ()>, cookie: i32, cookie_off: i32, src_off: i32, n: i32| -> i32 {
            if n < 0 {
                return -1;
            }
            let mut buf = vec![0u8; n as usize];
            if n > 0 && !caller_linear_read(&caller, src_off as u32, n as u32, &mut buf) {
                return -1;
            }
            host::mem_copy_in_at(cookie, cookie_off, &buf)
        },
    );
    let _ = linker.func_wrap(
        hm,
        "mem_copy_out_at",
        |mut caller: Caller<'_, ()>, cookie: i32, cookie_off: i32, dest_off: i32, n: i32| -> i32 {
            if n < 0 {
                return -1;
            }
            let mut buf = vec![0u8; n as usize];
            if host::mem_copy_out_at(cookie, cookie_off, &mut buf) != 0 {
                return -1;
            }
            if n > 0 && !caller_linear_write(&mut caller, dest_off as u32, &buf) {
                return -1;
            }
            0
        },
    );
}

fn register_loader_imports(linker: &mut Linker<()>) {
    use super::loader;
    let wm = WASM_MODULE;
    let _ = linker.func_wrap(
        wm,
        "version",
        |mut caller: Caller<'_, ()>, off: i32, maxlen: i32| -> i32 {
            loader::guest_version(&mut caller, off, maxlen)
        },
    );
    let _ = linker.func_wrap(wm, "mode", |_: Caller<'_, ()>| -> i32 {
        loader::guest_mode()
    });
    let _ = linker.func_wrap(wm, "verify", |_: Caller<'_, ()>| -> i32 {
        loader::guest_verify()
    });
    let _ = linker.func_wrap(wm, "trust_count", |_: Caller<'_, ()>| -> i32 {
        loader::guest_trust_count()
    });
    let _ = linker.func_wrap(
        wm,
        "call_i32",
        |caller: Caller<'_, ()>,
         pack_off: i32,
         pack_len: i32,
         func_off: i32,
         func_len: i32,
         nargs: i32,
         args_off: i32|
         -> i32 {
            loader::guest_call_i32(
                &caller, pack_off, pack_len, func_off, func_len, nargs, args_off,
            )
        },
    );
}

fn define_import_stub(
    linker: &mut Linker<()>,
    module: &str,
    field: &str,
    func_type: FuncType,
) -> Result<(), String> {
    if module == HOST_MODULE || module == WASM_MODULE {
        return Ok(());
    }
    let mod_name = module.to_string();
    let func_name = field.to_string();
    let is_fwd = super::forward::is_forwarder(module, field);
    linker
        .func_new(
            module,
            field,
            func_type,
            move |_caller: Caller<'_, ()>, params: &[Val], results: &mut [Val]| {
                if is_fwd {
                    if let Some(invoke) = INVOKE_BY_NAME.get() {
                        if invoke(&mod_name, &func_name, params, results).is_ok() {
                            return Ok(());
                        }
                    }
                }
                zero_results(results);
                Ok(())
            },
        )
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn satisfy_imports(
    linker: &mut Linker<()>,
    store: &mut Store<()>,
    module: &Module,
) -> Result<(), String> {
    for import in module.imports() {
        let module_name = import.module();
        let field_name = import.name();
        match import.ty() {
            ExternType::Func(func_ty) => {
                if module_name == HOST_MODULE {
                    continue;
                }
                if linker.get(&*store, module_name, field_name).is_some() {
                    continue;
                }
                define_import_stub(linker, module_name, field_name, func_ty.clone())?;
            }
            ExternType::Memory(mem_ty) => {
                if linker.get(&*store, module_name, field_name).is_some() {
                    continue;
                }
                let mem = Memory::new(&mut *store, *mem_ty).map_err(|e| e.to_string())?;
                linker
                    .define(module_name, field_name, mem)
                    .map_err(|e| e.to_string())?;
            }
            ExternType::Table(table_ty) => {
                if linker.get(&*store, module_name, field_name).is_some() {
                    continue;
                }
                let init = Ref::default_for_ty(table_ty.element());
                let table =
                    wasmi::Table::new(&mut *store, *table_ty, init).map_err(|e| e.to_string())?;
                linker
                    .define(module_name, field_name, table)
                    .map_err(|e| e.to_string())?;
            }
            ExternType::Global(global_ty) => {
                if linker.get(&*store, module_name, field_name).is_some() {
                    continue;
                }
                let val = Val::default_for_ty(global_ty.content());
                let global = wasmi::Global::new(&mut *store, val, global_ty.mutability());
                linker
                    .define(module_name, field_name, global)
                    .map_err(|e| e.to_string())?;
            }
        }
    }
    Ok(())
}

fn instantiate_module(compiled: &Module) -> Result<(Store<()>, Instance), String> {
    let mut store = Store::new(&ENGINE, ());
    let linker = HOST_LINKER.lock().unwrap().clone();
    let mut linker = linker;
    satisfy_imports(&mut linker, &mut store, compiled)?;
    let instance = linker
        .instantiate_and_start(&mut store, compiled)
        .map_err(|e| e.to_string())?;
    Ok((store, instance))
}

/// `mp_wasm_valkind_is_numeric`
pub fn valkind_is_numeric(kind: WasmValKind) -> bool {
    matches!(
        kind,
        WasmValKind::I32 | WasmValKind::I64 | WasmValKind::F32 | WasmValKind::F64
    )
}

/// `mp_wasm_runtime_init`
pub fn runtime_init() -> bool {
    if !mpconfig::PY_WASM {
        return false;
    }
    if RUNTIME_READY.load(Ordering::Relaxed) {
        return true;
    }
    LazyLock::force(&ENGINE);
    LazyLock::force(&HOST_LINKER);
    let _ = super::host::register_host();
    let _ = super::loader::register();
    RUNTIME_READY.store(true, Ordering::Relaxed);
    true
}

/// `mp_wasm_runtime_deinit`
pub fn runtime_deinit() {
    if !mpconfig::PY_WASM {
        return;
    }
    RUNTIME_READY.store(false, Ordering::Relaxed);
}

pub fn runtime_ready() -> bool {
    RUNTIME_READY.load(Ordering::Relaxed)
}

/// `mp_wasm_module_name`
pub fn module_name(mod_: &WasmModule) -> &str {
    std::str::from_utf8(&mod_.name[..mod_.name_len]).unwrap_or("")
}

/// `mp_wasm_module_set_name`
pub fn module_set_name(mod_: &mut WasmModule, name: &str) {
    mod_.set_name_fields(name);
}

/// `mp_wasm_module_bytes`
pub fn module_bytes(mod_: &WasmModule) -> &[u8] {
    &mod_.buf
}

/// `mp_wasm_module_meta_bytes`
pub fn module_meta_bytes(mod_: &WasmModule) -> &[u8] {
    &mod_.meta
}

/// `mp_wasm_module_load_ex`
pub fn module_load_ex(
    code: &[u8],
    meta: Option<&[u8]>,
    name: Option<&str>,
    path_hint: Option<&str>,
    errbuf: &mut [u8],
) -> Option<Box<WasmModule>> {
    errbuf[0] = 0;
    if !runtime_init() {
        set_err(errbuf, "wasm runtime init failed");
        return None;
    }
    if code.is_empty() {
        set_err(errbuf, "empty wasm");
        return None;
    }
    if !super::verify::verify_bytes(code, path_hint, errbuf) {
        return None;
    }

    let meta_bytes = meta.unwrap_or(code);
    if !super::forward::register_forwarders(meta_bytes, errbuf) {
        return None;
    }

    let compiled = match Module::new(&ENGINE, code) {
        Ok(m) => m,
        Err(e) => {
            set_err(errbuf, &e.to_string());
            return None;
        }
    };

    let (store, instance) = match instantiate_module(&compiled) {
        Ok(v) => v,
        Err(e) => {
            set_err(errbuf, &e);
            return None;
        }
    };

    let mut mod_ = WasmModule {
        name: [0; MP_WASM_NAME_MAX + 1],
        name_len: 0,
        buf: code.to_vec(),
        meta: Vec::new(),
        meta_owned: false,
        compiled,
        exec: Mutex::new(ModuleExec { store, instance }),
    };
    mod_.set_name_fields(name.unwrap_or("wasm"));
    mod_.set_meta(code, meta);
    Some(Box::new(mod_))
}

/// `mp_wasm_module_load`
pub fn module_load(bytes: &[u8], name: Option<&str>, errbuf: &mut [u8]) -> Option<Box<WasmModule>> {
    module_load_ex(bytes, None, name, None, errbuf)
}

/// `mp_wasm_module_close`
pub fn module_close(mod_: Box<WasmModule>) {
    super::forward::registry_remove(mod_.as_ref());
    drop(mod_);
}

/// Numeric export types for `func`.
pub fn module_func_type_kinds(
    mod_: &WasmModule,
    func: &str,
) -> Option<(Vec<WasmValKind>, Vec<WasmValKind>)> {
    mod_.with_exec(|store, instance| {
        let f = instance.get_func(&store, func)?;
        let ty = f.ty(&store);
        kinds_from_func_type(&ty)
    })
}

/// `mp_wasm_module_func_types` — returns true when export exists with numeric types.
pub fn module_func_types(mod_: &WasmModule, func: &str) -> bool {
    module_func_type_kinds(mod_, func).is_some()
}

fn call_export_vals(
    mod_: &WasmModule,
    func: &str,
    inputs: &[Val],
    outputs: &mut [Val],
    errbuf: &mut [u8],
) -> bool {
    errbuf[0] = 0;
    mod_.with_exec_mut(|store, instance| {
        let Some(f) = instance.get_func(&store, func) else {
            set_err(errbuf, "export not found");
            return false;
        };
        match f.call(store, inputs, outputs) {
            Ok(()) => true,
            Err(e) => {
                set_err(errbuf, &e.to_string());
                false
            }
        }
    })
}

/// Invoke an export on `mod_` (used by guest→guest forward dispatch).
pub fn module_invoke_export(
    mod_: &WasmModule,
    func: &str,
    inputs: &[Val],
    outputs: &mut [Val],
) -> Result<(), ()> {
    let mut err = [0u8; MP_WASM_ERRBUF];
    if call_export_vals(mod_, func, inputs, outputs, &mut err) {
        Ok(())
    } else {
        Err(())
    }
}

/// `mp_wasm_module_call_vals`
pub fn module_call_vals(
    mod_: &WasmModule,
    func: &str,
    inputs: &[Val],
    outputs: &mut [Val],
    errbuf: &mut [u8],
) -> bool {
    call_export_vals(mod_, func, inputs, outputs, errbuf)
}

/// `mp_wasm_module_call0`
pub fn module_call0(
    mod_: &WasmModule,
    func: &str,
    out_result: &mut i32,
    errbuf: &mut [u8],
) -> bool {
    module_call_i32(mod_, func, &[], out_result, errbuf)
}

/// `mp_wasm_module_call_i32`
pub fn module_call_i32(
    mod_: &WasmModule,
    func: &str,
    args: &[i32],
    out_result: &mut i32,
    errbuf: &mut [u8],
) -> bool {
    let inputs: Vec<Val> = args.iter().map(|&a| Val::I32(a)).collect();
    let mut outputs = [Val::I32(0)];
    if !module_call_vals(mod_, func, &inputs, &mut outputs, errbuf) {
        return false;
    }
    *out_result = val_to_i32(&outputs[0]);
    true
}

/// `mp_wasm_module_export_names`
pub fn module_export_names(mod_: &WasmModule) -> Vec<String> {
    mod_.with_exec(|store, instance| {
        instance
            .exports(&store)
            .filter_map(|export| {
                let name = export.name().to_string();
                export.into_func().map(|_| name)
            })
            .collect()
    })
}

/// Parse Wasm export section for function export names (static parse, no engine).
pub fn module_export_func_names(wasm: &[u8]) -> Vec<String> {
    let payload = match super::pack::find_section_id(wasm, 7) {
        Some(p) => p,
        None => return Vec::new(),
    };
    let mut p = 0usize;
    let end = payload.len();
    let n_exports = match super::pack::read_uleb(&mut p, end, payload) {
        Some(n) => n as usize,
        None => return Vec::new(),
    };
    let mut out = Vec::new();
    for _ in 0..n_exports {
        let name_len = match super::pack::read_uleb(&mut p, end, payload) {
            Some(n) => n as usize,
            None => break,
        };
        if p + name_len + 1 > end {
            break;
        }
        let name = std::str::from_utf8(&payload[p..p + name_len])
            .unwrap_or("")
            .to_string();
        p += name_len;
        let kind = payload[p];
        p += 1;
        let _ = super::pack::read_uleb(&mut p, end, payload);
        if kind == 0 {
            out.push(name);
        }
    }
    out
}

/// `mp_wasm_module_numeric_export_arity`
pub fn module_numeric_export_arity(mod_: &WasmModule, name: &str) -> bool {
    module_func_type_kinds(mod_, name).is_some()
}

/// `mp_wasm_module_i32_export_arity`
pub fn module_i32_export_arity(mod_: &WasmModule, name: &str) -> bool {
    module_func_type_kinds(mod_, name).is_some_and(|(p, r)| {
        p.iter().all(|k| *k == WasmValKind::I32)
            && r.iter().all(|k| *k == WasmValKind::I32)
            && r.len() <= 1
    })
}

fn guest_memory(mod_: &WasmModule) -> Option<Memory> {
    mod_.with_exec(|store, instance| instance.get_memory(&store, "memory"))
}

/// `mp_wasm_module_mem_read`
pub fn module_mem_read(mod_: &WasmModule, off: u32, n: u32, dst: &mut [u8]) -> bool {
    if dst.len() < n as usize {
        return false;
    }
    let Some(mem) = guest_memory(mod_) else {
        return false;
    };
    mod_.with_exec(|store, _| {
        mem.read(&store, off as usize, &mut dst[..n as usize])
            .is_ok()
    })
}

/// `mp_wasm_module_mem_write`
pub fn module_mem_write(mod_: &WasmModule, off: u32, n: u32, src: &[u8]) -> bool {
    if src.len() < n as usize {
        return false;
    }
    let Some(mem) = guest_memory(mod_) else {
        return false;
    };
    mod_.with_exec_mut(|store, _| mem.write(store, off as usize, &src[..n as usize]).is_ok())
}

/// `mp_wasm_module_mem_alloc` — calls guest `malloc` export when present.
pub fn module_mem_alloc(mod_: &WasmModule, n: u32) -> u32 {
    if n == 0 {
        return 0;
    }
    let mut out = 0i32;
    let mut err = [0u8; MP_WASM_ERRBUF];
    if module_call_i32(mod_, "malloc", &[n as i32], &mut out, &mut err) && out > 0 {
        out as u32
    } else {
        0
    }
}

/// `mp_wasm_module_mem_free` — calls guest `free` export when present.
pub fn module_mem_free(mod_: &WasmModule, off: u32) {
    if off == 0 {
        return;
    }
    let mut err = [0u8; MP_WASM_ERRBUF];
    let _ = module_call_i32(mod_, "free", &[off as i32], &mut 0i32, &mut err);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn module_name_roundtrip() {
        let mut err = [0u8; MP_WASM_ERRBUF];
        let m = module_load_ex(TEST_ADD_WASM, None, Some("hello"), None, &mut err).unwrap();
        assert_eq!(module_name(&m), "hello");
    }

    #[test]
    fn add_module_load_and_call() {
        assert!(runtime_init());
        let mut err = [0u8; MP_WASM_ERRBUF];
        let m = module_load_ex(TEST_ADD_WASM, None, Some("add"), None, &mut err)
            .expect("load add wasm");
        let mut out = 0i32;
        assert!(
            module_call_i32(&m, "add", &[2, 3], &mut out, &mut err),
            "{err:?}"
        );
        assert_eq!(out, 5);
    }
}
