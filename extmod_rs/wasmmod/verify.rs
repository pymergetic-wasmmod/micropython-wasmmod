//! rewrite of extmod/wasmmod/verify.c + extmod/wasmmod/verify.h
//!
//! Host ECDSA-SHA256 verify uses `ring` (same stack as `modtls_mbedtls.rs`).
//! Remaining gaps:
//! - `trust_load_builtin` is a weak port hook (session/trust list management is host-complete)
// symmetry: done

use super::pack::{find_custom_section, read_uleb, PACK_SECTION, SIG_SECTION};
use super::runtime::{self, MP_WASM_ERRBUF};

use flate2::read::ZlibDecoder;
use py_rs::mpconfig;
use ring::signature::{UnparsedPublicKey, ECDSA_P256_SHA256_ASN1, ECDSA_P384_SHA384_ASN1};
use sha2::{Digest, Sha256};
use std::io::Read;
use x509_parser::pem::Pem;
use x509_parser::prelude::{FromDer, X509Certificate};

/// Mirror `MICROPY_WASM_VERIFY` (0=off, 1=require, 2=verify-when-present).
pub const WASM_VERIFY: u8 = mpconfig::WASM_VERIFY;

const MAX_TRUST: usize = 32;
const MAX_KEY: usize = 4096;
const MPWS_MAGIC: &[u8; 4] = b"MPWS";
const MPWS_VER: u8 = 1;

#[derive(Clone)]
struct TrustKey {
    key: Vec<u8>,
}

static TRUST_KEYS: std::sync::Mutex<Vec<TrustKey>> = std::sync::Mutex::new(Vec::new());
static TRUST_BUILTIN_ARMED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);
static TRUST_BUILTIN_LOADED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);
static VERIFY_RUNTIME_ENABLED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(true);

/// `mp_wasm_set_verify_enabled`
pub fn set_verify_enabled(enabled: bool) {
    VERIFY_RUNTIME_ENABLED.store(enabled, std::sync::atomic::Ordering::Relaxed);
}

/// `mp_wasm_get_verify_enabled`
pub fn get_verify_enabled() -> bool {
    VERIFY_RUNTIME_ENABLED.load(std::sync::atomic::Ordering::Relaxed)
}

/// `mp_wasm_trust_add`
pub fn trust_add(key: &[u8]) -> bool {
    if key.is_empty() || key.len() > MAX_KEY {
        return false;
    }
    TRUST_KEYS
        .lock()
        .unwrap()
        .push(TrustKey { key: key.to_vec() });
    true
}

/// `mp_wasm_trust_clear`
pub fn trust_clear() {
    TRUST_KEYS.lock().unwrap().clear();
    TRUST_BUILTIN_ARMED.store(false, std::sync::atomic::Ordering::Relaxed);
    TRUST_BUILTIN_LOADED.store(false, std::sync::atomic::Ordering::Relaxed);
}

/// `mp_wasm_trust_init_session`
pub fn trust_init_session() {
    trust_clear();
    TRUST_BUILTIN_ARMED.store(true, std::sync::atomic::Ordering::Relaxed);
}

/// `mp_wasm_trust_load_builtin` — weak no-op unless port links baked roots.
pub fn trust_load_builtin() {}

/// `mp_wasm_trust_ensure`
pub fn trust_ensure() {
    if TRUST_BUILTIN_LOADED.load(std::sync::atomic::Ordering::Relaxed)
        || !TRUST_BUILTIN_ARMED.load(std::sync::atomic::Ordering::Relaxed)
    {
        return;
    }
    TRUST_BUILTIN_LOADED.store(true, std::sync::atomic::Ordering::Relaxed);
    trust_load_builtin();
}

/// `mp_wasm_trust_count`
pub fn trust_count() -> usize {
    trust_ensure();
    TRUST_KEYS.lock().unwrap().len()
}

fn trust_inflate_zlib(src: &[u8], dst_len: usize) -> Option<Vec<u8>> {
    let mut decoder = ZlibDecoder::new(src);
    let mut out = vec![0u8; dst_len];
    let n = decoder.read(&mut out).ok()?;
    if n != dst_len {
        return None;
    }
    let mut extra = [0u8; 1];
    if decoder.read(&mut extra).ok()? > 0 {
        return None;
    }
    Some(out)
}

/// `mp_wasm_trust_add_blob`
pub fn trust_add_blob(data: &[u8], uncompressed_len: u32) -> bool {
    if data.is_empty() || uncompressed_len == 0 {
        return false;
    }
    let uncompressed_len = uncompressed_len as usize;
    if uncompressed_len > MAX_KEY {
        return false;
    }
    if data.len() == uncompressed_len {
        return trust_add(data);
    }
    let raw = match trust_inflate_zlib(data, uncompressed_len) {
        Some(v) => v,
        None => return false,
    };
    trust_add(&raw)
}

fn sha256_digest(bytes: &[u8]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(bytes);
    h.finalize().into()
}

fn verify_ecdsa_sha256_spki(bytes: &[u8], sig: &[u8], spki: &[u8]) -> bool {
    let hash = sha256_digest(bytes);
    let p256 = UnparsedPublicKey::new(&ECDSA_P256_SHA256_ASN1, spki);
    if p256.verify(&hash, sig).is_ok() {
        return true;
    }
    let p384 = UnparsedPublicKey::new(&ECDSA_P384_SHA384_ASN1, spki);
    p384.verify(&hash, sig).is_ok()
}

fn spki_from_trust_key(key: &[u8]) -> Vec<Vec<u8>> {
    let mut out = Vec::new();
    if key.starts_with(b"-----") {
        for pem in Pem::iter_from_buffer(key).flatten() {
            match pem.label.as_str() {
                "CERTIFICATE" | "X509 CERTIFICATE" => {
                    if let Ok((_, cert)) = X509Certificate::from_der(&pem.contents) {
                        out.push(cert.public_key().raw.to_vec());
                    }
                }
                "PUBLIC KEY" => out.push(pem.contents),
                _ => {}
            }
        }
    } else if let Ok((_, cert)) = X509Certificate::from_der(key) {
        out.push(cert.public_key().raw.to_vec());
    } else {
        out.push(key.to_vec());
    }
    out
}

fn load_trust_cert_ders() -> Vec<Vec<u8>> {
    let keys = TRUST_KEYS.lock().unwrap();
    let mut certs = Vec::new();
    for tk in keys.iter() {
        if tk.key.starts_with(b"-----") {
            for pem in Pem::iter_from_buffer(&tk.key).flatten() {
                if pem.label == "CERTIFICATE" || pem.label == "X509 CERTIFICATE" {
                    certs.push(pem.contents);
                }
            }
        } else if X509Certificate::from_der(&tk.key).is_ok() {
            certs.push(tk.key.clone());
        }
    }
    certs
}

fn parse_der_cert_chain(buf: &[u8]) -> Option<Vec<Vec<u8>>> {
    let mut certs = Vec::new();
    let mut p = buf;
    while !p.is_empty() {
        let (rem, _) = X509Certificate::from_der(p).ok()?;
        let consumed = p.len() - rem.len();
        if consumed == 0 {
            return None;
        }
        certs.push(p[..consumed].to_vec());
        p = rem;
    }
    if certs.is_empty() {
        None
    } else {
        Some(certs)
    }
}

fn cert_matches_trust(a: &X509Certificate<'_>, b: &X509Certificate<'_>) -> bool {
    a.public_key().raw == b.public_key().raw && a.subject.as_raw() == b.subject.as_raw()
}

fn verify_pki_chain(bytes: &[u8], sig: &[u8], chain_der: &[u8]) -> bool {
    let chain_ders = match parse_der_cert_chain(chain_der) {
        Some(v) => v,
        None => return false,
    };
    let trust_ders = load_trust_cert_ders();
    if trust_ders.is_empty() {
        return false;
    }

    let (_, leaf) = match X509Certificate::from_der(&chain_ders[0]) {
        Ok(v) => v,
        Err(_) => return false,
    };

    for i in 0..chain_ders.len().saturating_sub(1) {
        let (_, cert) = X509Certificate::from_der(&chain_ders[i]).unwrap();
        let (_, issuer) = X509Certificate::from_der(&chain_ders[i + 1]).unwrap();
        if cert.verify_signature(Some(&issuer.public_key())).is_err() {
            return false;
        }
    }

    let (_, last) = X509Certificate::from_der(chain_ders.last().unwrap()).unwrap();
    let mut trusted = false;
    for td in &trust_ders {
        let (_, anchor) = match X509Certificate::from_der(td) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if cert_matches_trust(&last, &anchor) {
            trusted = true;
            break;
        }
        if last.verify_signature(Some(&anchor.public_key())).is_ok() {
            trusted = true;
            break;
        }
    }
    if !trusted && chain_ders.len() == 1 {
        for td in &trust_ders {
            let (_, anchor) = match X509Certificate::from_der(td) {
                Ok(v) => v,
                Err(_) => continue,
            };
            if leaf.verify_signature(Some(&anchor.public_key())).is_ok() {
                trusted = true;
                break;
            }
        }
    }
    if !trusted {
        return false;
    }
    verify_ecdsa_sha256_spki(bytes, sig, leaf.public_key().raw)
}

fn verify_with_trust(bytes: &[u8], sig: &[u8], chain: &[u8]) -> bool {
    if trust_count() == 0 {
        return false;
    }
    if !chain.is_empty() && verify_pki_chain(bytes, sig, chain) {
        return true;
    }
    let keys = TRUST_KEYS.lock().unwrap();
    for tk in keys.iter() {
        for spki in spki_from_trust_key(&tk.key) {
            if verify_ecdsa_sha256_spki(bytes, sig, &spki) {
                return true;
            }
        }
    }
    false
}

fn load_sidecar(path_hint: &str, suffix: &str) -> Option<Vec<u8>> {
    let mut path = String::with_capacity(path_hint.len() + suffix.len());
    path.push_str(path_hint);
    path.push_str(suffix);
    let mut err = [0u8; 64];
    super::fetch::fetch(&path, &mut err)
}

fn load_section_sig(bytes: &[u8]) -> Option<&[u8]> {
    find_custom_section(bytes, SIG_SECTION)
}

fn copy_without_sig_section(wasm: &[u8]) -> Option<Vec<u8>> {
    if wasm.len() < 8 || &wasm[0..4] != b"\0asm" {
        return None;
    }
    let want_len = SIG_SECTION.len();
    let mut out = Vec::with_capacity(wasm.len());
    out.extend_from_slice(&wasm[..8]);
    let mut p = 8usize;
    let end = wasm.len();
    while p < end {
        let sec_start = p;
        let id = wasm[p];
        p += 1;
        let mut ip = p;
        let size = read_uleb(&mut ip, end, wasm)? as usize;
        if ip + size > end {
            return None;
        }
        let payload = &wasm[ip..ip + size];
        p = ip + size;
        let mut skip = false;
        if id == 0 {
            let mut q = 0usize;
            let name_len = read_uleb(&mut q, payload.len(), payload)? as usize;
            if q + name_len <= payload.len()
                && name_len == want_len
                && &payload[q..q + name_len] == SIG_SECTION.as_bytes()
            {
                skip = true;
            }
        }
        if !skip {
            out.extend_from_slice(&wasm[sec_start..p]);
        }
    }
    Some(out)
}

fn parse_sig_payload(payload: &[u8]) -> Option<(&[u8], &[u8])> {
    if payload.len() >= 8 && &payload[0..4] == MPWS_MAGIC && payload[4] == MPWS_VER {
        let sl = ((payload[6] as u32) << 8) | payload[7] as u32;
        if 8 + sl as usize > payload.len() || sl == 0 {
            return None;
        }
        let sig = &payload[8..8 + sl as usize];
        let rest = payload.len() - 8 - sl as usize;
        let chain = if rest >= 2 {
            let cl = ((payload[8 + sl as usize] as u32) << 8) | payload[9 + sl as usize] as u32;
            if 2 + cl as usize > rest {
                return None;
            }
            &payload[10 + sl as usize..10 + sl as usize + cl as usize]
        } else {
            &[][..]
        };
        return Some((sig, chain));
    }
    if payload.is_empty() {
        None
    } else {
        Some((payload, &[][..]))
    }
}

fn verify_bytes_enabled(bytes: &[u8], path_hint: Option<&str>, errbuf: &mut [u8]) -> bool {
    errbuf[0] = 0;
    trust_ensure();
    if bytes.is_empty() {
        runtime::set_err(errbuf, "verify: empty");
        return false;
    }

    let have_detached = path_hint.is_some_and(|p| load_sidecar(p, ".sig").is_some());
    let have_crt = path_hint.is_some_and(|p| load_sidecar(p, ".crt").is_some());
    let sec_sig = load_section_sig(bytes);

    let detached = path_hint.and_then(|p| load_sidecar(p, ".sig"));
    let crt = path_hint.and_then(|p| load_sidecar(p, ".crt"));

    let stripped_storage: Option<Vec<u8>>;
    let (hash_bytes, sig, mut chain): (&[u8], &[u8], &[u8]);

    if let Some(sec_payload) = sec_sig {
        stripped_storage = match copy_without_sig_section(bytes) {
            Some(v) => Some(v),
            None => {
                runtime::set_err(errbuf, "verify: bad wasmmod.sig layout");
                return false;
            }
        };
        hash_bytes = stripped_storage.as_ref().unwrap().as_slice();
        let parsed = match parse_sig_payload(sec_payload) {
            Some(v) => v,
            None => {
                runtime::set_err(errbuf, "verify: bad wasmmod.sig payload");
                return false;
            }
        };
        sig = parsed.0;
        chain = parsed.1;
        if chain.is_empty() {
            if let Some(ref c) = crt {
                chain = c;
            }
        }
    } else if let Some(ref det) = detached {
        stripped_storage = None;
        hash_bytes = bytes;
        let parsed = match parse_sig_payload(det) {
            Some(v) => v,
            None => {
                runtime::set_err(errbuf, "verify: bad signature");
                return false;
            }
        };
        sig = parsed.0;
        chain = parsed.1;
        if chain.is_empty() {
            if let Some(ref c) = crt {
                chain = c;
            }
        }
    } else {
        let _ = have_detached;
        let _ = have_crt;
        if WASM_VERIFY == 1 {
            runtime::set_err(errbuf, "verify: signature required");
            return false;
        }
        return true;
    }

    let ok = verify_with_trust(hash_bytes, sig, chain);
    drop(stripped_storage);
    if !ok {
        runtime::set_err(errbuf, "verify: bad signature");
        return false;
    }
    true
}

/// `mp_wasm_verify_bytes`
pub fn verify_bytes(bytes: &[u8], path_hint: Option<&str>, errbuf: &mut [u8]) -> bool {
    if WASM_VERIFY == 0 {
        return true;
    }
    if !get_verify_enabled() {
        return true;
    }
    verify_bytes_enabled(bytes, path_hint, errbuf)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ring::rand::SystemRandom;
    use ring::signature::{EcdsaKeyPair, KeyPair, ECDSA_P256_SHA256_ASN1_SIGNING};

    #[test]
    fn verify_off_allows_unsigned() {
        let mut err = [0u8; MP_WASM_ERRBUF];
        assert!(verify_bytes(b"\0asm\x01\0\0\0", None, &mut err));
    }

    #[test]
    fn pack_section_name_differs_from_sig() {
        assert_ne!(SIG_SECTION, PACK_SECTION);
        assert_eq!(SIG_SECTION, "wasmmod.sig");
    }

    #[test]
    fn trust_add_and_clear() {
        trust_init_session();
        assert!(trust_add(b"test-key"));
        assert_eq!(trust_count(), 1);
        trust_clear();
        assert_eq!(trust_count(), 0);
    }

    #[test]
    fn trust_add_blob_inflates_zlib_spki() {
        use flate2::write::ZlibEncoder;
        use flate2::Compression;
        use std::io::Write;

        let rng = SystemRandom::new();
        let pkcs8 = EcdsaKeyPair::generate_pkcs8(&ECDSA_P256_SHA256_ASN1_SIGNING, &rng).unwrap();
        let key_pair =
            EcdsaKeyPair::from_pkcs8(&ECDSA_P256_SHA256_ASN1_SIGNING, pkcs8.as_ref(), &rng)
                .unwrap();
        let spki = key_pair.public_key().as_ref();

        let mut compressed = Vec::new();
        {
            let mut enc = ZlibEncoder::new(&mut compressed, Compression::default());
            enc.write_all(spki).unwrap();
            enc.finish().unwrap();
        }
        assert_ne!(compressed.len(), spki.len());

        trust_init_session();
        assert!(trust_add_blob(&compressed, spki.len() as u32));
        assert_eq!(trust_count(), 1);

        let msg = b"signed wasm payload";
        let hash = sha256_digest(msg);
        let sig = key_pair.sign(&rng, &hash).unwrap();
        assert!(verify_with_trust(msg, sig.as_ref(), &[]));

        trust_clear();
    }

    #[test]
    fn trust_add_blob_rejects_bad_zlib() {
        trust_init_session();
        assert!(!trust_add_blob(&[0x78, 0x9c, 0x01], 10));
        trust_clear();
    }

    #[test]
    fn ecdsa_sha256_good_and_bad_signature() {
        let rng = SystemRandom::new();
        let pkcs8 = EcdsaKeyPair::generate_pkcs8(&ECDSA_P256_SHA256_ASN1_SIGNING, &rng).unwrap();
        let key_pair =
            EcdsaKeyPair::from_pkcs8(&ECDSA_P256_SHA256_ASN1_SIGNING, pkcs8.as_ref(), &rng)
                .unwrap();
        let spki = key_pair.public_key().as_ref().to_vec();

        let msg = b"signed wasm payload";
        let hash = sha256_digest(msg);
        let sig = key_pair.sign(&rng, &hash).unwrap();

        assert!(verify_ecdsa_sha256_spki(msg, sig.as_ref(), &spki));
        assert!(!verify_ecdsa_sha256_spki(
            b"tampered payload",
            sig.as_ref(),
            &spki
        ));

        trust_clear();
        assert!(trust_add(&spki));
        assert!(verify_with_trust(msg, sig.as_ref(), &[]));
        assert!(!verify_with_trust(b"tampered payload", sig.as_ref(), &[]));
        trust_clear();
    }
}
