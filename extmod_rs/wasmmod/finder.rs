//! rewrite of extmod/wasmmod/finder.c + extmod/wasmmod/finder.h
// symmetry: gaps
// gaps:
// - `mp_wasm_find_pack` needs VFS/`wasm.path`/`sys.path` lookup wired on host rewrite
// - `mp_wasm_import_wasm` needs `mp_wasm_load_pack_path` from wasmmod + fetch

/// `dotted_to_slash`
pub fn dotted_to_slash(dotted: &str) -> String {
    dotted.replace('.', "/")
}

fn path_is_frozen(root: &str) -> bool {
    root == ".frozen"
}

/// Whether a wasm pack might live under `root` for `slash_name` (no I/O on host rewrite).
pub fn candidate_rel_paths(slash_name: &str, aot: bool) -> Vec<String> {
    let mut out = Vec::new();
    if aot {
        out.push(format!("{slash_name}/__init__.aot"));
        out.push(format!("{slash_name}.aot"));
    }
    out.push(format!("{slash_name}/__init__.wasm"));
    out.push(format!("{slash_name}.wasm"));
    out
}

/// `mp_wasm_find_pack`
pub fn find_pack(dotted_name: &str) -> Option<String> {
    if dotted_name.is_empty() {
        return None;
    }
    let slash = dotted_to_slash(dotted_name);
    let _ = (slash, path_is_frozen);
    None
}
