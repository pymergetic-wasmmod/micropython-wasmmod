//! rewrite of extmod/wasmmod/packload.c
//!
//! Remaining gaps:
//! - `score_pack_file_for_upy_host` tagged `.upy.*.mpy` preference parity incomplete
//! - `mp_wasm_load_pack_path` AOT sibling (.aot / .mpack metadata) when `PY_WASM_AOT`
//! - `mod_wasm_import_hook` full namespace/discover_fill / VFS listdir parity
//! - `mp_wasm_has_descendants` namespace packages need VFS listdir wiring
// symmetry: done

use std::sync::atomic::{AtomicI32, Ordering};

use py_rs::compile;
use py_rs::emitglue::{self, CompiledModule, ProtoFun};
use py_rs::lexer::Lexer;
use py_rs::mpconfig;
use py_rs::mpstate;
use py_rs::obj::{self, Obj};
use py_rs::objdict;
use py_rs::objmodule;
use py_rs::objstr;
use py_rs::parse::ParseInputKind;
use py_rs::qstr;
use py_rs::raise::{self, MpRaise};
use py_rs::reader::READER_IS_ROM;

use super::cdn;
use super::fetch;
use super::finder;
use super::forward;
use super::modobj;
use super::pack::{self, PackExport, PackFile, PackInfo, PACK_KIND_PY};
use super::resolve::{self, DepNode};
use super::runtime::{self, WasmModule, MP_WASM_ERRBUF};

const PACK_KIND_PYC: u8 = 4;

static IMPORT_HOOK_DEPTH: AtomicI32 = AtomicI32::new(0);

/// Mirror `mp_wasm_import_hook_depth`.
pub fn import_hook_depth() -> i32 {
    IMPORT_HOOK_DEPTH.load(Ordering::Relaxed)
}

/// Reset or set hook depth (used by uninstall_hook).
pub fn set_import_hook_depth(v: i32) {
    IMPORT_HOOK_DEPTH.store(v, Ordering::Relaxed);
}

fn raise_runtime_msg(msg: impl Into<String>) -> ! {
    let s = msg.into();
    let exc = py_rs::objexcept::new_exception_args(
        py_rs::objexcept::type_runtime_error(),
        1,
        &[objstr::new_str(s.as_bytes())],
    );
    raise::raise_obj(exc);
}

fn raise_import(name: &str) -> ! {
    let msg = format!("no wasm pack named '{name}'");
    let exc = py_rs::objexcept::new_exception_args(
        py_rs::objexcept::type_import_error(),
        1,
        &[objstr::new_str(msg.as_bytes())],
    );
    raise::raise_obj(exc);
}

fn pack_logical_path_len(path: &str) -> usize {
    let path_len = path.len();
    let bytes = path.as_bytes();
    for i in 0..path_len {
        if bytes[i] != b'.' {
            continue;
        }
        if i + 5 <= path_len && &bytes[i..i + 5] == b".upy." {
            return i;
        }
        if i + 5 <= path_len && &bytes[i..i + 5] == b".cpy." {
            return i;
        }
    }
    if path_len >= 3 && &bytes[path_len - 3..] == b".py" {
        return path_len - 3;
    }
    if path_len >= 4 && &bytes[path_len - 4..] == b".mpy" {
        return path_len - 4;
    }
    if path_len >= 4 && &bytes[path_len - 4..] == b".pyc" {
        return path_len - 4;
    }
    path_len
}

fn pack_logical_eq(a: &str, b: &str) -> bool {
    let la = pack_logical_path_len(a);
    let lb = pack_logical_path_len(b);
    la == lb && a.as_bytes()[..la] == b.as_bytes()[..lb]
}

fn path_to_dotted(pack_name: &str, path: &str) -> String {
    let n = pack_logical_path_len(path);
    let trimmed = &path[..n];
    if trimmed == "__init__" {
        return pack_name.to_string();
    }
    let trimmed = trimmed.strip_suffix("/__init__").unwrap_or(trimmed);
    let mut out = String::with_capacity(pack_name.len() + trimmed.len() + 2);
    out.push_str(pack_name);
    out.push('.');
    for c in trimmed.chars() {
        out.push(if c == '/' { '.' } else { c });
    }
    out
}

fn score_pack_file_for_host(f: &PackFile<'_>) -> i32 {
    if f.path.contains(".cpy.") {
        return -1;
    }
    if f.kind == PACK_KIND_PYC {
        return -1;
    }
    if f.kind == PACK_KIND_PY {
        return 1;
    }
    if f.kind != pack::PACK_KIND_MPY {
        return -1;
    }
    if !mpconfig::PERSISTENT_CODE_LOAD {
        return -1;
    }
    let Some(data) = pack::pack_file_bytes(f) else {
        return -1;
    };
    if data.len() < 4 || data[0] != b'M' || data[1] != py_rs::persistentcode::MPY_VERSION {
        return -1;
    }
    // Native arch in feature byte: only accept bytecode (arch == 0) for now.
    let arch = (data[2] >> 2) & 0x2f;
    if arch != 0 {
        return -1;
    }
    if data[3] as u32 > py_rs::smallint::BITS {
        return -1;
    }

    let path = f.path.as_bytes();
    let path_len = path.len();
    let mut tag: Option<&[u8]> = None;
    for i in 0..path_len.saturating_sub(4) {
        if &path[i..i + 5] == b".upy." {
            tag = Some(&path[i + 5..]);
            break;
        }
    }
    if let Some(tag) = tag {
        if tag.len() >= 8 && &tag[..3] == b"mpy" {
            let mut mpy_ver = 0u32;
            let mut p = 3usize;
            while p < tag.len() && tag[p].is_ascii_digit() {
                mpy_ver = mpy_ver * 10 + (tag[p] - b'0') as u32;
                p += 1;
            }
            let mut sib = 0u32;
            for j in 0..tag.len().saturating_sub(4) {
                if tag[j] == b'.' && j + 4 < tag.len() && &tag[j..j + 4] == b".sib" {
                    let mut k = j + 4;
                    while k < tag.len() && tag[k].is_ascii_digit() {
                        sib = sib * 10 + (tag[k] - b'0') as u32;
                        k += 1;
                    }
                    break;
                }
            }
            if mpy_ver != py_rs::persistentcode::MPY_VERSION as u32
                || sib == 0
                || sib > py_rs::smallint::BITS
            {
                return -1;
            }
            return 100 + sib as i32;
        }
    }
    50 + data[3] as i32
}

fn exec_py_into_module(module_obj: Obj, src_name: &str, data: &[u8]) {
    let globals_ptr = objmodule::module_get_globals(module_obj);
    let globals = obj::from_ptr(globals_ptr as *const objdict::ObjDict as *const ());
    let qname = qstr::from_str(src_name);
    let lex = Lexer::new_from_str_len(qname, data, READER_IS_ROM);
    compile::parse_compile_execute(lex, ParseInputKind::FileInput, Some(globals), Some(globals));
}

fn exec_mpy_into_module(module_obj: Obj, src_name: &str, data: &[u8]) {
    if !mpconfig::PERSISTENT_CODE_LOAD {
        raise::raise(MpRaise::ValueError("mpy load disabled"));
    }
    let ctx = obj::as_ptr(module_obj) as *mut py_rs::bc::ModuleContext;
    let mut cm = CompiledModule {
        context: ctx,
        rc: core::ptr::null(),
        has_native: false,
        n_qstr: 0,
        n_obj: 0,
        arch_flags: 0,
    };
    py_rs::persistentcode::raw_code_load_mem(data, &mut cm);
    if mpconfig::MODULE_FILE {
        py_rs::runtime::store_attr(
            module_obj,
            qstr::from_str("__file__"),
            objstr::new_str(src_name.as_bytes()),
        );
    }
    let mod_globals = unsafe { (*ctx).module.globals };
    let old_globals = py_rs::runtime::globals_get();
    let old_locals = py_rs::runtime::locals_get();
    py_rs::runtime::globals_set(obj::from_ptr(
        mod_globals as *const objdict::ObjDict as *const (),
    ));
    py_rs::runtime::locals_set(obj::from_ptr(
        mod_globals as *const objdict::ObjDict as *const (),
    ));
    py_rs::nlr::push_jump_callback(move || {
        py_rs::runtime::globals_locals_set_from_nlr_jump_callback(old_globals, old_locals);
    });
    let module_fun = emitglue::make_function_from_proto_fun(cm.rc as ProtoFun, ctx, None);
    py_rs::runtime::call_function_0(module_fun);
    py_rs::nlr::pop_jump_callback(false);
}

fn exec_pack_file_into_module(module_obj: Obj, src_name: &str, f: &PackFile<'_>) {
    let Some(data) = pack::pack_file_bytes(f) else {
        raise::raise(MpRaise::ValueError("wasm pack file inflate failed"));
    };
    if f.kind == PACK_KIND_PY {
        exec_py_into_module(module_obj, src_name, &data);
        return;
    }
    if f.kind == pack::PACK_KIND_MPY {
        exec_mpy_into_module(module_obj, src_name, &data);
        return;
    }
    raise::raise(MpRaise::ValueError("wasm pack file kind not supported"));
}

fn ensure_parent_packages(full_name: &str) {
    for (i, c) in full_name.char_indices() {
        if c != '.' {
            continue;
        }
        let parent = &full_name[..i];
        let qparent = qstr::from_str(parent);
        let pmod = objmodule::new_module(qparent);
        let globals = objmodule::module_get_globals(pmod);
        let path_key = obj::new_qstr(qstr::from_str("__path__"));
        let globals_obj = obj::from_ptr(globals as *const objdict::ObjDict as *const ());
        if objdict::dict_get(globals_obj, path_key) == obj::OBJ_NULL {
            objdict::dict_store(globals_obj, path_key, objstr::new_str(parent.as_bytes()));
        }
    }
}

fn link_module_to_parent(dotted_name: &str) {
    let Some(dot) = dotted_name.rfind('.') else {
        return;
    };
    let parent = &dotted_name[..dot];
    let leaf = &dotted_name[dot + 1..];
    let loaded = mpstate::with_vm(|vm| vm.mp_loaded_modules_dict);
    let parent_mod = objdict::dict_get(loaded, obj::new_qstr(qstr::from_str(parent)));
    let child_mod = objdict::dict_get(loaded, obj::new_qstr(qstr::from_str(dotted_name)));
    if parent_mod != obj::OBJ_NULL && child_mod != obj::OBJ_NULL {
        let pglobals = objmodule::module_get_globals(parent_mod);
        objdict::dict_store(
            obj::from_ptr(pglobals as *const objdict::ObjDict as *const ()),
            obj::new_qstr(qstr::from_str(leaf)),
            child_mod,
        );
    }
}

fn path_is_package_init(path: &str) -> bool {
    let n = pack_logical_path_len(path);
    n == 8 && &path.as_bytes()[..8] == b"__init__"
        || n > 9 && &path.as_bytes()[n - 9..n] == b"/__init__"
}

fn stem_from_path(path: &str) -> String {
    let base = path.rsplit(['/', '\\']).next().unwrap_or(path);
    let mut n = base.len();
    if base.ends_with(".wasm") {
        n -= 5;
    } else if base.ends_with(".aot") {
        n -= 4;
    }
    if n == 8 && &base.as_bytes()[..8] == b"__init__" {
        if let Some(slash) = path.rfind(['/', '\\']) {
            let dir = &path[..slash];
            return dir.rsplit(['/', '\\']).next().unwrap_or(dir).to_string();
        }
    }
    base[..n].to_string()
}

fn module_for_export_suffix(pack_name: &str, suffix: &str) -> Obj {
    if suffix.is_empty() || suffix == "." {
        return objmodule::new_module(qstr::from_str(pack_name));
    }
    let name = format!("{pack_name}.{suffix}");
    ensure_parent_packages(&name);
    objmodule::new_module(qstr::from_str(&name))
}

fn bind_pack_exports(
    _root: Obj,
    wasm_obj: Obj,
    mod_id: u64,
    pack_name: &str,
    exports: &[PackExport<'_>],
) {
    for ex in exports {
        if ex.func.is_empty() || ex.export_name.is_empty() {
            continue;
        }
        let target = module_for_export_suffix(pack_name, ex.module);
        let qexport = qstr::from_str(ex.export_name);
        let f = modobj::func_new(mod_id, qexport);
        let tglobals = objmodule::module_get_globals(target);
        objdict::dict_store(
            obj::from_ptr(tglobals as *const objdict::ObjDict as *const ()),
            obj::new_qstr(qstr::from_str(ex.func)),
            f,
        );
    }
}

fn bind_all_exports_from_section(wasm_obj: Obj, mod_id: u64, code: &[u8], py_mod: Obj) {
    let names = runtime::module_export_func_names(code);
    let pglobals = objmodule::module_get_globals(py_mod);
    let pglobals_obj = obj::from_ptr(pglobals as *const objdict::ObjDict as *const ());
    for name in names {
        if name == "mp_pack_load" || name == "mp_pack_unload" {
            continue;
        }
        let qexport = qstr::from_str(&name);
        let f = modobj::func_new(mod_id, qexport);
        objdict::dict_store(pglobals_obj, obj::new_qstr(qstr::from_str(&name)), f);
    }
    let _ = wasm_obj;
}

/// `load_pack_from_parts`
pub fn load_pack_from_parts(
    code: &[u8],
    meta: Option<&[u8]>,
    path_hint: Option<&str>,
    name_override: Option<&str>,
) -> Obj {
    let meta_bytes = meta.unwrap_or(code);
    let pack_name = resolve_pack_name(meta_bytes, path_hint, name_override);
    let deps = resolve::deps_from_artifact(meta_bytes);
    if !deps.is_empty() || cdn::driver_name() == "metal-cdn" {
        return load_closure_from_root(
            DepNode::new(&pack_name, ""),
            code.to_vec(),
            path_hint.map(str::to_string),
        );
    }
    let pending = instantiate_pack(code, meta_bytes, path_hint, &pack_name);
    finish_pack(pending)
}

fn resolve_pack_name(
    meta_bytes: &[u8],
    path_hint: Option<&str>,
    name_override: Option<&str>,
) -> String {
    if let Some(n) = name_override {
        if !n.is_empty() {
            return n.to_string();
        }
    }
    if let Some(payload) = pack::pack_find_section(meta_bytes) {
        if let Some(peek) = pack::pack_parse(payload) {
            if !peek.name.is_empty() {
                return peek.name.to_string();
            }
        }
    }
    if let Some(hint) = path_hint {
        let stem = stem_from_path(hint);
        if !stem.is_empty() {
            return stem;
        }
    }
    "wasm_pack".to_string()
}

struct PendingPack {
    name: String,
    code: Vec<u8>,
    wmod: Box<WasmModule>,
}

fn instantiate_pack(
    code: &[u8],
    meta_bytes: &[u8],
    path_hint: Option<&str>,
    pack_name: &str,
) -> PendingPack {
    if pack_name.is_empty() {
        raise_runtime_msg("wasm pack: empty name");
    }
    let mut err = [0u8; MP_WASM_ERRBUF];
    let mut wmod =
        runtime::module_load_ex(code, Some(meta_bytes), Some(pack_name), path_hint, &mut err)
            .unwrap_or_else(|| {
                let msg = std::str::from_utf8(&err)
                    .unwrap_or("")
                    .trim_end_matches('\0');
                raise_runtime_msg(if msg.is_empty() {
                    "wasm load failed"
                } else {
                    msg
                });
            });
    runtime::module_set_name(wmod.as_mut(), pack_name);
    PendingPack {
        name: pack_name.to_string(),
        code: code.to_vec(),
        wmod,
    }
}

/// Phased closure load: fetch → instantiate all → registry all → connect → run.
pub fn load_closure_from_root(
    root: DepNode,
    root_bytes: Vec<u8>,
    _root_path: Option<String>,
) -> Obj {
    let mut source = cdn::FetchingDepSource::new();
    source.cache.insert(root.key(), root_bytes);

    let closure = resolve::resolve_closure(root.clone(), &mut source).unwrap_or_else(|e| {
        raise_runtime_msg(format!("resolve closure: {e}"));
    });

    if cdn::require_explicit_deps() {
        for node in closure.nodes() {
            let bytes = source.ensure(node).unwrap_or_else(|e| raise_runtime_msg(e));
            if let Some(payload) = pack::imports_find_section(bytes) {
                if let Some(info) = pack::imports_parse(payload) {
                    for im in &info.imports {
                        if im.module == pack::HOST_MODULE || im.module == pack::WASM_MODULE {
                            continue;
                        }
                        if !closure.nodes().any(|n| n.name == im.module) {
                            raise_runtime_msg(format!(
                                "cdn: import {} not in closure of {}",
                                im.module, node.name
                            ));
                        }
                    }
                }
            }
        }
    }

    let mut pending: Vec<PendingPack> = Vec::new();
    for node in closure.nodes() {
        let bytes = source
            .ensure(node)
            .unwrap_or_else(|e| raise_runtime_msg(e))
            .to_vec();
        pending.push(instantiate_pack(&bytes, &bytes, None, &node.name));
    }

    let mut registered: Vec<(String, Vec<u8>, Obj, Obj)> = Vec::new();
    for p in pending {
        let name = p.name.clone();
        let code = p.code.clone();
        let qpack = qstr::from_str(&name);
        let root_mod = objmodule::new_module(qpack);
        let wasm_obj = modobj::wrap_loaded(p.wmod);
        modobj::set_pack_name(wasm_obj, qpack);
        registered.push((name, code, root_mod, wasm_obj));
    }

    for (_name, code, _, _) in &registered {
        if let Err(e) = forward::connect_imports(code) {
            raise_runtime_msg(e);
        }
    }

    let mut root_obj = obj::CONST_NONE;
    for (name, code, root_mod, wasm_obj) in registered {
        let obj = finish_registered_pack(&name, &code, root_mod, wasm_obj);
        if name == root.name || root_obj == obj::CONST_NONE {
            root_obj = obj;
        }
    }
    root_obj
}

fn finish_pack(pending: PendingPack) -> Obj {
    let name = pending.name.clone();
    let code = pending.code.clone();
    let qpack = qstr::from_str(&name);
    let root = objmodule::new_module(qpack);
    let wasm_obj = modobj::wrap_loaded(pending.wmod);
    modobj::set_pack_name(wasm_obj, qpack);
    if let Err(e) = forward::connect_imports(&code) {
        if cdn::require_explicit_deps() {
            raise_runtime_msg(e);
        }
    }
    finish_registered_pack(&name, &code, root, wasm_obj)
}

fn finish_registered_pack(pack_name: &str, code: &[u8], root: Obj, wasm_obj: Obj) -> Obj {
    let mut err = [0u8; MP_WASM_ERRBUF];
    let _ = modobj::call0_on_obj(wasm_obj, "mp_pack_load", &mut err);

    let rglobals = objmodule::module_get_globals(root);
    let rglobals_obj = obj::from_ptr(rglobals as *const objdict::ObjDict as *const ());
    objdict::dict_store(
        rglobals_obj,
        obj::new_qstr(qstr::from_str("__wasm__")),
        wasm_obj,
    );
    objdict::dict_store(
        rglobals_obj,
        obj::new_qstr(qstr::from_str("__path__")),
        objstr::new_str(pack_name.as_bytes()),
    );

    let info: Option<PackInfo<'_>> = pack::pack_find_section(code).and_then(pack::pack_parse);
    let mod_id = modobj::module_id_from_obj(wasm_obj).expect("wrapped module id");

    if let Some(ref info) = info {
        if !info.exports.is_empty() {
            bind_pack_exports(root, wasm_obj, mod_id, pack_name, &info.exports);
        } else {
            bind_all_exports_from_section(wasm_obj, mod_id, code, root);
        }

        let mut best_idx: Vec<usize> = Vec::new();
        let mut best_score: Vec<i32> = Vec::new();
        for (i, f) in info.files.iter().enumerate() {
            if f.kind != PACK_KIND_PY && f.kind != pack::PACK_KIND_MPY && f.kind != PACK_KIND_PYC {
                continue;
            }
            let score = score_pack_file_for_host(f);
            if score < 0 {
                continue;
            }
            if let Some(slot) = best_idx
                .iter()
                .position(|&bi| pack_logical_eq(f.path, info.files[bi].path))
            {
                if score > best_score[slot] {
                    best_idx[slot] = i;
                    best_score[slot] = score;
                }
            } else {
                best_idx.push(i);
                best_score.push(score);
            }
        }

        for &bi in &best_idx {
            let f = &info.files[bi];
            let dotted_name = path_to_dotted(pack_name, f.path);
            ensure_parent_packages(&dotted_name);
            let qmod = qstr::from_str(&dotted_name);
            let mod_ = objmodule::new_module(qmod);
            let mglobals = objmodule::module_get_globals(mod_);
            let mglobals_obj = obj::from_ptr(mglobals as *const objdict::ObjDict as *const ());
            objdict::dict_store(
                mglobals_obj,
                obj::new_qstr(qstr::from_str("__wasm__")),
                wasm_obj,
            );
            if path_is_package_init(f.path) {
                objdict::dict_store(
                    mglobals_obj,
                    obj::new_qstr(qstr::from_str("__path__")),
                    objstr::new_str(dotted_name.as_bytes()),
                );
            }
            let src_name = format!("{pack_name}:{}", f.path);
            exec_pack_file_into_module(mod_, &src_name, f);
            link_module_to_parent(&dotted_name);
        }
    }

    let loaded = mpstate::with_vm(|vm| vm.mp_loaded_modules_dict);
    objdict::dict_store(loaded, obj::new_qstr(qstr::from_str(pack_name)), root);
    root
}

/// `mp_wasm_load_pack_path`
pub fn load_pack_path(path: &str, name_override: Option<&str>) -> Obj {
    let mut fetch_err = [0u8; MP_WASM_ERRBUF];
    let code = fetch::fetch(path, &mut fetch_err).unwrap_or_else(|| {
        raise::raise(MpRaise::OSError(2));
    });
    load_pack_from_parts(&code, None, Some(path), name_override)
}

fn lookup_loaded(dotted_name: &str) -> Obj {
    let loaded = mpstate::with_vm(|vm| vm.mp_loaded_modules_dict);
    objdict::dict_get(loaded, obj::new_qstr(qstr::from_str(dotted_name)))
}

/// `mp_wasm_import_wasm_at`
pub fn import_wasm_at(dotted_name: &str, known_path: Option<&str>) -> Obj {
    let existing = lookup_loaded(dotted_name);
    if existing != obj::OBJ_NULL {
        return existing;
    }

    let path = if let Some(p) = known_path {
        p.to_string()
    } else {
        finder::find_pack(dotted_name).unwrap_or_else(|| raise_import(dotted_name))
    };

    if dotted_name.contains('.') {
        if let Some(parent) = dotted_name.rsplit_once('.').map(|(p, _)| p) {
            if lookup_loaded(parent) == obj::OBJ_NULL {
                if finder::find_pack(parent).is_some() {
                    let _ = import_wasm_at(parent, None);
                }
            }
        }
    }

    let mod_ = load_pack_path(&path, Some(dotted_name));
    let existing = lookup_loaded(dotted_name);
    if existing != obj::OBJ_NULL {
        link_module_to_parent(dotted_name);
        return existing;
    }
    link_module_to_parent(dotted_name);
    mod_
}

/// `mp_wasm_import_wasm`
pub fn import_wasm(dotted_name: &str) -> Obj {
    import_wasm_at(dotted_name, None)
}

/// `mod_wasm_unload`
pub fn unload(name: &str) -> Obj {
    let nlen = name.len();
    let loaded = mpstate::with_vm(|vm| vm.mp_loaded_modules_dict);
    let loaded_ptr = objdict::dict_ptr(loaded);
    let map = unsafe { &mut (*loaded_ptr).map };

    if let Some(el) = py_rs::map::lookup(
        map,
        obj::new_qstr(qstr::from_str(name)),
        py_rs::map::LookupKind::Lookup,
    ) {
        if el.value != obj::OBJ_NULL {
            let mglobals = objmodule::module_get_globals(el.value);
            let wasm_key = obj::new_qstr(qstr::from_str("__wasm__"));
            let wasm_obj = objdict::dict_get(
                obj::from_ptr(mglobals as *const objdict::ObjDict as *const ()),
                wasm_key,
            );
            if wasm_obj != obj::OBJ_NULL && obj::is_exact_type(wasm_obj, modobj::type_wasm_module())
            {
                modobj::close_module(wasm_obj);
            }
        }
    }

    let mut keys: Vec<Obj> = Vec::new();
    for i in 0..map.alloc {
        if !py_rs::map::slot_is_filled(map, i) || !obj::is_qstr(map.table[i].key) {
            continue;
        }
        let (data, len) = objstr::get_str_data_len(map.table[i].key);
        let Ok(k) = std::str::from_utf8(&data[..len]) else {
            continue;
        };
        let klen = k.len();
        if (klen == nlen && k == name)
            || (klen > nlen && k.as_bytes().get(nlen) == Some(&b'.') && k.starts_with(name))
        {
            keys.push(map.table[i].key);
        }
    }
    for key in keys {
        let _ = py_rs::map::lookup(map, key, py_rs::map::LookupKind::RemoveIfFound);
    }
    obj::CONST_NONE
}

#[cfg(test)]
mod testutil {
    use super::pack::{PACK_KIND_PY, PACK_MAGIC, PACK_SECTION};

    pub fn encode_uleb(mut v: u32) -> Vec<u8> {
        let mut out = Vec::new();
        loop {
            let mut b = (v & 0x7f) as u8;
            v >>= 7;
            if v != 0 {
                b |= 0x80;
            }
            out.push(b);
            if v == 0 {
                break;
            }
        }
        out
    }

    pub fn build_pack_payload(name: &str, files: &[(&str, u8, &[u8])]) -> Vec<u8> {
        let mut p = Vec::new();
        p.extend_from_slice(PACK_MAGIC);
        p.extend_from_slice(&3u16.to_le_bytes());
        p.extend_from_slice(&0u16.to_le_bytes());
        p.extend_from_slice(&(name.len() as u16).to_le_bytes());
        p.extend_from_slice(name.as_bytes());
        p.extend_from_slice(&(files.len() as u32).to_le_bytes());
        for (path, kind, data) in files {
            p.extend_from_slice(&(path.len() as u16).to_le_bytes());
            p.extend_from_slice(path.as_bytes());
            p.push(*kind);
            p.push(0);
            p.extend_from_slice(&(data.len() as u32).to_le_bytes());
            p.extend_from_slice(&(data.len() as u32).to_le_bytes());
            p.extend_from_slice(data);
        }
        p.extend_from_slice(&0u32.to_le_bytes());
        p
    }

    pub fn build_wasm_with_custom_section(section_name: &str, payload: &[u8]) -> Vec<u8> {
        let mut wasm = vec![0u8, b'a', b's', b'm', 1, 0, 0, 0];
        let name_bytes = section_name.as_bytes();
        let mut body = Vec::new();
        body.extend_from_slice(&encode_uleb(name_bytes.len() as u32));
        body.extend_from_slice(name_bytes);
        body.extend_from_slice(payload);
        let mut sec = Vec::new();
        sec.push(0);
        sec.extend_from_slice(&encode_uleb(body.len() as u32));
        sec.extend_from_slice(&body);
        wasm.extend_from_slice(&sec);
        wasm
    }

    pub fn build_test_pack_wasm(pack_name: &str, py_path: &str, py_src: &[u8]) -> Vec<u8> {
        let payload = build_pack_payload(pack_name, &[(py_path, PACK_KIND_PY, py_src)]);
        build_wasm_with_custom_section(PACK_SECTION, &payload)
    }
}

#[cfg(test)]
mod tests {
    use super::testutil::build_test_pack_wasm;
    use super::*;
    use py_rs::gc;
    use py_rs::modbuiltins;
    use py_rs::mpconfig;
    use py_rs::mpstate;
    use py_rs::runtime;

    fn setup() {
        let _ = gc::init();
        py_rs::qstr::init();
        mpstate::init();
        runtime::init();
        let _ = modbuiltins::init_builtins_module();
        mpstate::with_vm(|vm| {
            if vm.mp_loaded_modules_dict == obj::OBJ_NULL {
                vm.mp_loaded_modules_dict =
                    objdict::new_dict(mpconfig::LOADED_MODULES_DICT_SIZE as usize);
            }
        });
    }

    #[test]
    fn load_pack_from_parts_registers_py_module() {
        setup();
        let wasm = build_test_pack_wasm("testpack", "mod.py", b"X = 41\nY = X + 1\n");
        let mut nlr_buf = py_rs::nlr::NlrBuf::default();
        let root = py_rs::nlr::protect(&mut nlr_buf, || {
            load_pack_from_parts(&wasm, None, None, None)
        })
        .expect("load pack");
        assert_ne!(root, obj::OBJ_NULL);

        let loaded = mpstate::with_vm(|vm| vm.mp_loaded_modules_dict);
        let mod_ = objdict::dict_get(loaded, obj::new_qstr(qstr::from_str("testpack.mod")));
        assert_ne!(mod_, obj::OBJ_NULL);

        let globals = objmodule::module_get_globals(mod_);
        let x = objdict::dict_get(
            obj::from_ptr(globals as *const objdict::ObjDict as *const ()),
            obj::new_qstr(qstr::from_str("X")),
        );
        assert_eq!(obj::small_int_value(x), 41);

        let y = objdict::dict_get(
            obj::from_ptr(globals as *const objdict::ObjDict as *const ()),
            obj::new_qstr(qstr::from_str("Y")),
        );
        assert_eq!(obj::small_int_value(y), 42);

        let _ = root;
    }

    #[test]
    fn path_to_dotted_strips_extensions() {
        assert_eq!(path_to_dotted("pkg", "sub/mod.py"), "pkg.sub.mod");
    }
}
