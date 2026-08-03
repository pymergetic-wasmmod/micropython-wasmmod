//! rewrite of extmod/wasmmod/forward.c + extmod/wasmmod/forward.h
// symmetry: gaps
// gaps:
// - `forward_raw` / `register_one` need WAMR (`wasm_runtime_register_natives_raw`, `wasm_export.h`)
// - `mp_wasm_register_forwarders` validates imports but cannot publish WAMR natives on host rewrite

use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

use super::pack::{find_section_id, imports_find_section, imports_parse, read_uleb};
use super::runtime::{self, WasmModule, MP_WASM_ERRBUF, MP_WASM_NAME_MAX};

static REGISTRY: std::sync::LazyLock<Mutex<HashMap<String, u64>>> =
    std::sync::LazyLock::new(|| Mutex::new(HashMap::new()));
static FORWARDERS: std::sync::LazyLock<Mutex<HashSet<(String, String)>>> =
    std::sync::LazyLock::new(|| Mutex::new(HashSet::new()));
static NEXT_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

fn wamr_sig_char(vt: u8) -> Option<char> {
    match vt {
        0x7f => Some('i'),
        0x7e => Some('I'),
        0x7d => Some('f'),
        0x7c => Some('F'),
        _ => None,
    }
}

/// `mp_wasm_registry_add`
pub fn registry_add(mod_: &WasmModule) {
    let name = runtime::module_name(mod_).to_string();
    let mut reg = REGISTRY.lock().unwrap();
    reg.entry(name)
        .or_insert_with(|| NEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed));
}

/// `mp_wasm_registry_remove`
pub fn registry_remove(mod_: &WasmModule) {
    REGISTRY
        .lock()
        .unwrap()
        .remove(runtime::module_name(mod_));
}

/// Used by `runtime::module_close`.
pub fn registry_remove_by_ptr(mod_: Box<WasmModule>) {
    registry_remove(mod_.as_ref());
    drop(mod_);
}

/// `mp_wasm_registry_find`
pub fn registry_find(name: &str) -> bool {
    REGISTRY.lock().unwrap().contains_key(name)
}

struct TypeInfo {
    nparams: u32,
    nresults: u32,
    params: Vec<u8>,
    results: Vec<u8>,
    ok: bool,
}

/// Parse Wasm type+import sections; return numeric kinds for (module, field).
fn import_func_types(wasm: &[u8], module: &str, field: &str) -> Option<TypeInfo> {
    let types_payload = find_section_id(wasm, 1)?;
    let imports_payload = find_section_id(wasm, 2)?;

    let mut tp = 0usize;
    let n_types = read_uleb(&mut tp, types_payload.len(), types_payload)?;
    let mut types: Vec<TypeInfo> = Vec::with_capacity(n_types as usize);

    for _ in 0..n_types {
        if tp >= types_payload.len() || types_payload[tp] != 0x60 {
            return None;
        }
        tp += 1;
        let np = read_uleb(&mut tp, types_payload.len(), types_payload)?;
        let mut params = Vec::with_capacity(np as usize);
        let mut ok = true;
        for _ in 0..np {
            if tp >= types_payload.len() {
                return None;
            }
            let vt = types_payload[tp];
            tp += 1;
            params.push(vt);
            if wamr_sig_char(vt).is_none() {
                ok = false;
            }
        }
        let nr = read_uleb(&mut tp, types_payload.len(), types_payload)?;
        let mut results = Vec::with_capacity(nr as usize);
        for _ in 0..nr {
            if tp >= types_payload.len() {
                return None;
            }
            let vt = types_payload[tp];
            tp += 1;
            results.push(vt);
            if wamr_sig_char(vt).is_none() {
                ok = false;
            }
        }
        types.push(TypeInfo {
            nparams: np,
            nresults: nr,
            params,
            results,
            ok: ok && nr <= 1,
        });
    }

    let mut ip = 0usize;
    let n_imports = read_uleb(&mut ip, imports_payload.len(), imports_payload)?;
    for _ in 0..n_imports {
        let mlen = read_uleb(&mut ip, imports_payload.len(), imports_payload)?;
        if ip + mlen as usize > imports_payload.len() {
            break;
        }
        let m = std::str::from_utf8(&imports_payload[ip..ip + mlen as usize]).ok()?;
        ip += mlen as usize;
        let flen = read_uleb(&mut ip, imports_payload.len(), imports_payload)?;
        if ip + flen as usize > imports_payload.len() {
            break;
        }
        let f = std::str::from_utf8(&imports_payload[ip..ip + flen as usize]).ok()?;
        ip += flen as usize;
        if ip >= imports_payload.len() {
            break;
        }
        let kind = imports_payload[ip];
        ip += 1;
        match kind {
            0 => {
                let typeidx = read_uleb(&mut ip, imports_payload.len(), imports_payload)? as usize;
                if m == module
                    && f == field
                    && typeidx < types.len()
                    && types[typeidx].ok
                {
                    return Some(types.remove(typeidx));
                }
            }
            1 => {
                let flags = read_uleb(&mut ip, imports_payload.len(), imports_payload)?;
                let _ = read_uleb(&mut ip, imports_payload.len(), imports_payload)?;
                if flags & 1 != 0 {
                    let _ = read_uleb(&mut ip, imports_payload.len(), imports_payload)?;
                }
            }
            2 => {
                let flags = read_uleb(&mut ip, imports_payload.len(), imports_payload)?;
                let _ = read_uleb(&mut ip, imports_payload.len(), imports_payload)?;
                if flags & 1 != 0 {
                    let _ = read_uleb(&mut ip, imports_payload.len(), imports_payload)?;
                }
            }
            3 => {
                if ip + 2 > imports_payload.len() {
                    break;
                }
                ip += 2;
            }
            _ => break,
        }
    }
    None
}

fn fwd_exists(module: &str, func: &str) -> bool {
    FORWARDERS
        .lock()
        .unwrap()
        .contains(&(module.to_string(), func.to_string()))
}

fn register_one(
    module: &str,
    func: &str,
    nparams: u32,
    param_kinds: &[u8],
    nresults: u32,
    result_kinds: &[u8],
    errbuf: &mut [u8],
) -> bool {
    if fwd_exists(module, func) {
        return true;
    }
    if nresults > 1 {
        runtime::set_err(
            errbuf,
            &format!("forwarder {module}.{func}: multi-result not supported"),
        );
        return false;
    }
    for &vt in param_kinds {
        if wamr_sig_char(vt).is_none() {
            runtime::set_err(
                errbuf,
                &format!("forwarder {module}.{func}: unsupported param type"),
            );
            return false;
        }
    }
    if nresults == 1 && wamr_sig_char(result_kinds[0]).is_none() {
        runtime::set_err(
            errbuf,
            &format!("forwarder {module}.{func}: unsupported result type"),
        );
        return false;
    }
    let _ = (nparams, param_kinds, nresults, result_kinds);
    runtime::set_err(
        errbuf,
        &format!("register forwarder {module}.{func} failed"),
    );
    false
}

/// `mp_wasm_register_forwarders`
pub fn register_forwarders(wasm: &[u8], errbuf: &mut [u8]) -> bool {
    errbuf[0] = 0;
    let payload = match imports_find_section(wasm) {
        Some(p) => p,
        None => return true,
    };
    let info = match imports_parse(payload) {
        Some(i) => i,
        None => {
            runtime::set_err(errbuf, "bad micropython.imports section");
            return false;
        }
    };
    let mut ok = true;
    for im in &info.imports {
        let module = if im.module.len() > MP_WASM_NAME_MAX {
            &im.module[..MP_WASM_NAME_MAX]
        } else {
            im.module
        };
        let func = if im.func.len() > MP_WASM_NAME_MAX {
            &im.func[..MP_WASM_NAME_MAX]
        } else {
            im.func
        };
        if module.starts_with("micropython.") {
            continue;
        }
        let (nparams, nresults, params, results) = match import_func_types(wasm, module, func) {
            Some(t) => (t.nparams, t.nresults, t.params, t.results),
            None => (0, 1, Vec::new(), vec![0x7f]),
        };
        if !register_one(
            module,
            func,
            nparams,
            &params,
            nresults,
            &results,
            errbuf,
        ) {
            ok = false;
            break;
        }
        FORWARDERS
            .lock()
            .unwrap()
            .insert((module.to_string(), func.to_string()));
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_forwarders_no_section_ok() {
        let wasm = b"\0asm\x01\0\0\0";
        let mut err = [0u8; MP_WASM_ERRBUF];
        assert!(register_forwarders(wasm, &mut err));
    }
}
