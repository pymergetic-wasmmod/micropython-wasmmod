//! rewrite of extmod/wasmmod/fetch.c + extmod/wasmmod/fetch.h
// symmetry: gaps
// gaps:
// - mp_wasm_fetch file/HTTP load requires port VFS hook or MICROPY_WASM_FETCH (host unix has no fetch yet)

pub fn uri_is_http(uri: &str) -> bool {
    uri.starts_with("http://") || uri.starts_with("https://")
}

pub fn join_uri(root: &str, rel: &str) -> String {
    let mut out = String::from(root);
    if !out.ends_with('/') {
        out.push('/');
    }
    let rel = rel.strip_prefix('/').unwrap_or(rel);
    out.push_str(rel);
    out
}

#[cfg(feature = "wasm")]
pub fn fetch(_uri: &str, _errbuf: &mut [u8]) -> Option<Vec<u8>> {
    None
}

#[cfg(not(feature = "wasm"))]
pub fn fetch(_uri: &str, _errbuf: &mut [u8]) -> Option<Vec<u8>> {
    None
}
