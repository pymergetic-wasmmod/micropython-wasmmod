//! rewrite of extmod/wasmmod/verify.c + extmod/wasmmod/verify.h
// symmetry: gaps
// gaps:
// - detached `.sig` load needs `mp_wasm_fetch` (host rewrite has no fetch yet)
// - ECDSA-SHA256 verify needs `MICROPY_SSL_MBEDTLS` / trusted public keys

use super::pack::{find_custom_section, PACK_SECTION};
use super::runtime::{self, MP_WASM_ERRBUF};

pub const SIG_SECTION: &str = "micropython.sig";

/// Mirror `MICROPY_WASM_VERIFY` (0=off, 1=require, 2=verify-when-present).
pub const WASM_VERIFY: u8 = 0;

const MAX_TRUST: usize = 8;
const MAX_KEY: usize = 256;

#[derive(Copy, Clone)]
struct TrustKey {
    key: [u8; MAX_KEY],
    key_len: u16,
}

impl Default for TrustKey {
    fn default() -> Self {
        Self {
            key: [0; MAX_KEY],
            key_len: 0,
        }
    }
}

static mut TRUST_KEYS: [TrustKey; MAX_TRUST] = [TrustKey {
    key: [0; MAX_KEY],
    key_len: 0,
}; MAX_TRUST];
static mut TRUST_N: usize = 0;

/// `mp_wasm_trust_add`
pub fn trust_add(key: &[u8]) -> bool {
    if key.is_empty() || key.len() > MAX_KEY {
        return false;
    }
    unsafe {
        if TRUST_N >= MAX_TRUST {
            return false;
        }
        TRUST_KEYS[TRUST_N].key[..key.len()].copy_from_slice(key);
        TRUST_KEYS[TRUST_N].key_len = key.len() as u16;
        TRUST_N += 1;
    }
    true
}

/// `mp_wasm_trust_clear`
pub fn trust_clear() {
    unsafe {
        for slot in TRUST_KEYS.iter_mut() {
            *slot = TrustKey::default();
        }
        TRUST_N = 0;
    }
}

/// `mp_wasm_trust_count`
pub fn trust_count() -> usize {
    unsafe { TRUST_N }
}

fn load_section_sig(bytes: &[u8]) -> Option<&[u8]> {
    find_custom_section(bytes, SIG_SECTION)
}

fn verify_with_trust(_bytes: &[u8], _sig: &[u8]) -> bool {
    unsafe {
        if TRUST_N == 0 {
            return false;
        }
    }
    false
}

/// `mp_wasm_verify_bytes`
pub fn verify_bytes(bytes: &[u8], path_hint: Option<&str>, errbuf: &mut [u8]) -> bool {
    errbuf[0] = 0;
    if bytes.is_empty() {
        runtime::set_err(errbuf, "verify: empty");
        return false;
    }

    let have_detached = path_hint.is_some_and(|p| {
        !super::fetch::uri_is_http(p) && std::path::Path::new(p).with_extension("sig").exists()
    });
    let sec_sig = load_section_sig(bytes);

    let sig = if have_detached {
        None
    } else {
        sec_sig
    };

    if have_detached {
        runtime::set_err(errbuf, "verify: detached sig fetch not wired");
        return false;
    }

    match sig {
        Some(sig_bytes) => {
            if verify_with_trust(bytes, sig_bytes) {
                true
            } else {
                runtime::set_err(errbuf, "verify: bad signature");
                false
            }
        }
        None => {
            if WASM_VERIFY == 1 {
                runtime::set_err(errbuf, "verify: signature required");
                false
            } else {
                true
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verify_off_allows_unsigned() {
        let mut err = [0u8; MP_WASM_ERRBUF];
        assert!(verify_bytes(b"\0asm\x01\0\0\0", None, &mut err));
    }

    #[test]
    fn pack_section_name_differs_from_sig() {
        assert_ne!(SIG_SECTION, PACK_SECTION);
    }
}
