//! rewrite of extmod/wasmmod/finder.c + extmod/wasmmod/finder.h
//!
//! Remaining gaps:
//! - AOT/arch-tagged artifact names need `PY_WASM_AOT` + `wasm.arch` list
// symmetry: done

use std::path::Path;
use std::sync::{LazyLock, Mutex};

use super::fetch;

static WASM_PATH: LazyLock<Mutex<Vec<String>>> = LazyLock::new(|| Mutex::new(Vec::new()));

/// `dotted_to_slash`
pub fn dotted_to_slash(dotted: &str) -> String {
    dotted.replace('.', "/")
}

fn path_is_frozen(root: &str) -> bool {
    root == ".frozen"
}

/// Whether a wasm pack might live under `root` for `slash_name` (no I/O).
/// `aot_ver`: WAMR AOT_CURRENT_VERSION (e.g. 6 → `.aot6`); 0 = legacy `.aot` only.
pub fn candidate_rel_paths(slash_name: &str, aot: bool, aot_ver: u32) -> Vec<String> {
    let mut out = Vec::new();
    if aot {
        let mut exts: Vec<String> = Vec::new();
        if aot_ver > 0 {
            exts.push(format!(".aot{aot_ver}"));
        }
        exts.push(".aot".into());
        for ext in &exts {
            out.push(format!("{slash_name}/__init__{ext}.zlib"));
            out.push(format!("{slash_name}/__init__{ext}"));
            out.push(format!("{slash_name}{ext}.zlib"));
            out.push(format!("{slash_name}{ext}"));
        }
    }
    out.push(format!("{slash_name}/__init__.wasm.zlib"));
    out.push(format!("{slash_name}/__init__.wasm"));
    out.push(format!("{slash_name}.wasm.zlib"));
    out.push(format!("{slash_name}.wasm"));
    out
}

/// `mp_wasm_path_ensure`
pub fn path_ensure() {
    let _guard = WASM_PATH.lock().unwrap();
}

/// Append a search root (`wasm.path` entry).
pub fn path_push(root: impl Into<String>) {
    path_ensure();
    WASM_PATH.lock().unwrap().push(root.into());
}

/// Replace the search path list (tests / host bootstrap).
pub fn path_set(roots: Vec<String>) {
    *WASM_PATH.lock().unwrap() = roots;
}

fn path_roots() -> Vec<String> {
    let from_vm = py_rs::mpstate::with_vm(|vm| {
        if vm.mp_wasm_path == py_rs::obj::OBJ_NULL {
            return Vec::new();
        }
        if !py_rs::obj::is_exact_type(vm.mp_wasm_path, py_rs::objlist::type_list()) {
            return Vec::new();
        }
        let (n, items) = py_rs::objlist::list_get(vm.mp_wasm_path);
        let mut out = Vec::new();
        for i in 0..n {
            if py_rs::obj::is_str_or_bytes(items[i]) {
                let (data, len) = py_rs::objstr::get_str_data_len(items[i]);
                if let Ok(s) = std::str::from_utf8(&data[..len]) {
                    out.push(s.to_string());
                }
            }
        }
        out
    });
    if !from_vm.is_empty() {
        return from_vm;
    }
    WASM_PATH.lock().unwrap().clone()
}

fn try_vfs_file(root: &str, rel: &str) -> Option<String> {
    let path = fetch::join_uri(root, rel);
    if Path::new(&path).is_file() {
        Some(path)
    } else {
        None
    }
}

fn try_one_arch_ext(root: &str, stem: &str, allow_pkg: bool, ext: &str) -> Option<String> {
    if allow_pkg {
        let rel = format!("{stem}/__init__{ext}");
        let zrel = format!("{rel}.zlib");
        if let Some(p) = try_vfs_file(root, &zrel) {
            return Some(p);
        }
        if let Some(p) = try_vfs_file(root, &rel) {
            return Some(p);
        }
    }
    let rel = format!("{stem}{ext}");
    let zrel = format!("{rel}.zlib");
    if let Some(p) = try_vfs_file(root, &zrel) {
        return Some(p);
    }
    try_vfs_file(root, &rel)
}

fn try_stem_variants(root: &str, stem: &str, allow_pkg: bool) -> Option<String> {
    try_one_arch_ext(root, stem, allow_pkg, ".wasm")
}

fn find_in_root(root: &str, dotted_name: &str, slash_name: &str) -> Option<String> {
    if path_is_frozen(root) || fetch::uri_is_http(root) {
        return None;
    }
    let root = if root.is_empty() { "." } else { root };

    if let Some(p) = try_stem_variants(root, slash_name, true) {
        return Some(p);
    }

    if dotted_name.contains('.') {
        if let Some(p) = try_stem_variants(root, dotted_name, false) {
            return Some(p);
        }
    }

    None
}

fn find_in_list(roots: &[String], dotted_name: &str, slash_name: &str) -> Option<String> {
    for root in roots {
        if let Some(p) = find_in_root(root, dotted_name, slash_name) {
            return Some(p);
        }
    }
    None
}

/// `mp_wasm_find_pack`
pub fn find_pack(dotted_name: &str) -> Option<String> {
    if dotted_name.is_empty() {
        return None;
    }
    let slash = dotted_to_slash(dotted_name);
    path_ensure();
    find_in_list(&path_roots(), dotted_name, &slash)
}

/// `mp_wasm_find_pack_on_wasm_path` — same as find_pack on host rewrite (wasm.path only).
pub fn find_pack_on_wasm_path(dotted_name: &str) -> Option<String> {
    find_pack(dotted_name)
}

/// Resolve pack bytes via find + fetch.
pub fn find_and_fetch(dotted_name: &str, errbuf: &mut [u8]) -> Option<(String, Vec<u8>)> {
    let path = find_pack(dotted_name)?;
    let bytes = fetch::fetch(&path, errbuf)?;
    Some((path, bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("wasmmod-finder-{stamp}-{name}"))
    }

    #[test]
    fn find_pack_resolves_flat_wasm() {
        let dir = temp_dir("flat");
        std::fs::create_dir_all(&dir).unwrap();
        let wasm_path = dir.join("mypack.wasm");
        std::fs::File::create(&wasm_path)
            .unwrap()
            .write_all(b"\0asm\x01\0\0\0")
            .unwrap();

        path_set(vec![dir.to_string_lossy().into_owned()]);
        let found = find_pack("mypack").expect("pack path");
        assert!(found.ends_with("mypack.wasm"));

        let mut err = [0u8; super::super::runtime::MP_WASM_ERRBUF];
        let (path, bytes) = find_and_fetch("mypack", &mut err).expect("fetch pack");
        assert_eq!(path, found);
        assert_eq!(&bytes[..4], b"\0asm");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn find_pack_tree_form() {
        let dir = temp_dir("tree");
        let nested = dir.join("sub").join("pkg");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::File::create(nested.join("__init__.wasm"))
            .unwrap()
            .write_all(b"\0asm\x01\0\0\0")
            .unwrap();

        path_set(vec![dir.to_string_lossy().into_owned()]);
        let found = find_pack("sub.pkg").expect("tree pack");
        assert!(found.ends_with("sub/pkg/__init__.wasm"));
        let _ = std::fs::remove_dir_all(dir);
    }
}
