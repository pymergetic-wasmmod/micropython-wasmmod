//! rewrite of extmod/wasmmod/wasmmod.c
// symmetry: gaps
// gaps:
// - `WasmModule` / `WasmFunc` Python types and export binding need WAMR runtime
// - `wasm.load` / `load_pack` / `import_wasm` / import-hook need fetch + module load
// - pack publish (`load_pack_from_parts`, `bind_pack_exports`) needs runtime call/export helpers
// - host/path list + previous-import root pointers need VM state wiring with WAMR lifecycle

use py_rs::mpconfig;

pub const VERIFY_CONST: i32 = super::verify::WASM_VERIFY as i32;
pub const AOT_CONST: i32 = 0;

/// `ends_with` (AOT path helpers in wasmmod.c)
pub fn ends_with(s: &str, suf: &str) -> bool {
    s.ends_with(suf)
}

/// `replace_suffix` — writes into `out` when `path` ends with `old_suf`.
pub fn replace_suffix(path: &str, old_suf: &str, new_suf: &str, out: &mut String) -> bool {
    if !path.ends_with(old_suf) {
        return false;
    }
    out.clear();
    out.push_str(&path[..path.len() - old_suf.len()]);
    out.push_str(new_suf);
    true
}

/// `path_is_package_init`
pub fn path_is_package_init(path: &str) -> bool {
    path == "__init__.py" || path.ends_with("/__init__.py")
}

/// `stem_from_path`
pub fn stem_from_path(path: &str) -> String {
    let base = path.rsplit(['/', '\\']).next().unwrap_or(path);
    let mut n = base.len();
    if base.ends_with(".wasm") {
        n -= 5;
    } else if base.ends_with(".aot") {
        n -= 4;
    }
    if n == 8 && base.as_bytes().get(..8) == Some(b"__init__") {
        if let Some(slash) = path.rfind(['/', '\\']) {
            let dir = &path[..slash];
            return dir.rsplit(['/', '\\']).next().unwrap_or(dir).to_string();
        }
    }
    base[..n].to_string()
}

/// `path_to_dotted` — append a pack-relative path onto `pack_name`.
pub fn path_to_dotted(pack_name: &str, path: &str) -> String {
    let mut n = path.len();
    if path.ends_with(".py") {
        n -= 3;
    } else if path.ends_with(".mpy") {
        n -= 4;
    }
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

/// Register built-in `wasm` module (`MP_REGISTER_EXTENSIBLE_MODULE`).
pub fn init_module() -> py_rs::obj::Obj {
    if !mpconfig::PY_WASM {
        return py_rs::obj::OBJ_NULL;
    }
    py_rs::obj::OBJ_NULL
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_to_dotted_strips_py() {
        assert_eq!(path_to_dotted("mypack", "sub/mod.py"), "mypack.sub.mod");
    }

    #[test]
    fn stem_from_wasm_file() {
        assert_eq!(stem_from_path("dir/pkg.wasm"), "pkg");
    }
}
