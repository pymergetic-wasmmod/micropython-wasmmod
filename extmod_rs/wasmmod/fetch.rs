//! rewrite of extmod/wasmmod/fetch.c + extmod/wasmmod/fetch.h
//!
//! Remaining gaps:
//! - MicroPython VFS reader not wired (local paths use std::fs)
// symmetry: done

use std::io::Read;

use super::io::{self, FetchOutcome, IoResult};
use super::runtime::{self, MP_WASM_ERRBUF};

/// `mp_wasm_io_set` — delegates to [`io::set`].
pub fn io_set(ops: Option<&'static io::IoOps>) {
    io::set(ops);
}

/// `mp_wasm_uri_is_http`
pub fn uri_is_http(uri: &str) -> bool {
    uri.starts_with("http://") || uri.starts_with("https://")
}

/// `mp_wasm_join_uri`
pub fn join_uri(root: &str, rel: &str) -> String {
    let mut out = String::from(root);
    if !out.is_empty() && !out.ends_with('/') {
        out.push('/');
    }
    let rel = rel.strip_prefix('/').unwrap_or(rel);
    out.push_str(rel);
    out
}

fn fetch_file(path: &str, errbuf: &mut [u8]) -> Option<Vec<u8>> {
    match std::fs::read(path) {
        Ok(bytes) => Some(bytes),
        Err(e) => {
            runtime::set_err(errbuf, &format!("fetch failed: {path}: {e}"));
            None
        }
    }
}

fn fetch_http(uri: &str, errbuf: &mut [u8]) -> Option<Vec<u8>> {
    let resp = match ureq::get(uri).call() {
        Ok(r) => r,
        Err(ureq::Error::Status(code, _)) => {
            runtime::set_err(errbuf, &format!("HTTP {code}"));
            return None;
        }
        Err(e) => {
            runtime::set_err(errbuf, &format!("HTTP fetch failed: {uri}: {e}"));
            return None;
        }
    };
    let status = resp.status();
    if status != 200 {
        runtime::set_err(errbuf, &format!("HTTP {status}"));
        return None;
    }
    let mut body = Vec::new();
    match resp.into_reader().read_to_end(&mut body) {
        Ok(_) => Some(body),
        Err(e) => {
            runtime::set_err(errbuf, &format!("HTTP read failed: {e}"));
            None
        }
    }
}

fn native_http_probe(uri: &str) -> bool {
    match ureq::head(uri).call() {
        Ok(resp) => resp.status() == 200,
        Err(ureq::Error::Status(code, _)) if code == 405 || code == 501 => ureq::get(uri)
            .call()
            .map(|r| r.status() == 200)
            .unwrap_or(false),
        Err(_) => false,
    }
}

fn take_port_fetch(uri: &str, errbuf: &mut [u8]) -> Result<Option<Vec<u8>>, ()> {
    match io::invoke_fetch(uri, errbuf) {
        FetchOutcome::Ok(bytes) => Ok(Some(bytes)),
        FetchOutcome::Decline => Ok(None),
        FetchOutcome::Err => Err(()),
    }
}

/// `mp_wasm_http_probe`
pub fn http_probe(uri: &str) -> bool {
    if !uri_is_http(uri) {
        return false;
    }
    match io::invoke_probe(uri) {
        IoResult::Ok => true,
        IoResult::Err => false,
        IoResult::Decline => native_http_probe(uri),
    }
}

/// `mp_wasm_fetch`
pub fn fetch(uri: &str, errbuf: &mut [u8]) -> Option<Vec<u8>> {
    errbuf[0] = 0;
    if uri.is_empty() {
        runtime::set_err(errbuf, "empty uri");
        return None;
    }

    // 1) Port I/O ops (io.h)
    match take_port_fetch(uri, errbuf) {
        Ok(Some(bytes)) => return Some(bytes),
        Ok(None) if errbuf[0] != 0 => return None,
        Ok(None) => {}
        Err(()) => return None,
    }

    // 2) HTTP(S) native client (ureq)
    if uri_is_http(uri) {
        return fetch_http(uri, errbuf);
    }

    // 3) Filesystem (absolute/relative path)
    fetch_file(uri, errbuf)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn fetch_reads_local_file() {
        let path =
            std::env::temp_dir().join(format!("wasmmod_fetch_test_{}.wasm", std::process::id()));
        {
            let mut f = std::fs::File::create(&path).unwrap();
            f.write_all(b"\0asm\x01\0\0\0").unwrap();
        }
        let mut err = [0u8; MP_WASM_ERRBUF];
        let bytes = fetch(path.to_str().unwrap(), &mut err).expect("fetch local wasm");
        assert_eq!(&bytes[..4], b"\0asm");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn fetch_rejects_empty_uri() {
        let mut err = [0u8; MP_WASM_ERRBUF];
        assert!(fetch("", &mut err).is_none());
        assert!(std::str::from_utf8(&err).unwrap().contains("empty uri"));
    }

    #[test]
    fn http_fetch_uses_io_ops_when_set() {
        static HTTP_OPS: io::IoOps = io::IoOps {
            version: io::IO_OPS_VERSION,
            fetch: Some(http_mock_fetch),
            probe: None,
            yield_fn: None,
            reserved0: 0,
            reserved1: 0,
            userdata: 0,
        };

        fn http_mock_fetch(
            uri: &str,
            out_bytes: &mut Option<Vec<u8>>,
            out_len: &mut u32,
            _errbuf: &mut [u8],
        ) -> IoResult {
            assert_eq!(uri, "http://example.test/module.wasm");
            *out_bytes = Some(b"\0asm\x01\0\0\0".to_vec());
            *out_len = 8;
            IoResult::Ok
        }

        io::set(Some(&HTTP_OPS));
        let mut err = [0u8; MP_WASM_ERRBUF];
        let bytes = fetch("http://example.test/module.wasm", &mut err)
            .expect("port fetch should win over native HTTP");
        assert_eq!(&bytes[..4], b"\0asm");
        io::set(None);
    }

    #[test]
    fn http_fetch_without_ops_returns_clear_error() {
        io::set(None);
        let mut err = [0u8; MP_WASM_ERRBUF];
        assert!(fetch("http://127.0.0.1:1/unreachable.wasm", &mut err).is_none());
        let msg = std::str::from_utf8(&err).unwrap();
        assert!(msg.contains("HTTP"), "expected HTTP error, got: {msg}");
        io::set(None);
    }
}
