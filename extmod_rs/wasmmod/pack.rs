//! rewrite of extmod/wasmmod/pack.c + extmod/wasmmod/pack.h
// symmetry: done

pub const PACK_SECTION: &str = "wasmmod.pack";
pub const IMPORTS_SECTION: &str = "wasmmod.imports";
pub const SIG_SECTION: &str = "wasmmod.sig";
pub const HOST_MODULE: &str = "wasmmod.host";
pub const WASM_MODULE: &str = "wasmmod";
pub const PACK_MAGIC: &[u8; 4] = b"MPWP";
pub const IMPORTS_MAGIC: &[u8; 4] = b"MPWI";
pub const PACK_KIND_PY: u8 = 1;
pub const PACK_KIND_MPY: u8 = 2;
pub const PACK_KIND_RAW: u8 = 3;
pub const PACK_SIG_AUTO: u8 = 255;
pub const PACK_FILE_FLAG_ZLIB: u8 = 1 << 0;
pub const ARTIFACT_ZLIB_MAGIC: &[u8; 4] = b"MPZL";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackFile<'a> {
    pub path: &'a str,
    pub kind: u8,
    pub flags: u8,
    pub raw_len: u32,
    pub data: &'a [u8],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackExport<'a> {
    pub module: &'a str,
    pub func: &'a str,
    pub export_name: &'a str,
    pub sig: u8,
}

#[derive(Debug, Default)]
pub struct PackInfo<'a> {
    pub name: &'a str,
    pub version: u16,
    pub flags: u16,
    pub files: Vec<PackFile<'a>>,
    pub exports: Vec<PackExport<'a>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportEntry<'a> {
    pub module: &'a str,
    pub func: &'a str,
}

#[derive(Debug, Default)]
pub struct ImportsInfo<'a> {
    pub version: u16,
    pub imports: Vec<ImportEntry<'a>>,
}

pub fn read_uleb(p: &mut usize, end: usize, data: &[u8]) -> Option<u32> {
    let mut result = 0u32;
    let mut shift = 0u32;
    while *p < end {
        let b = data[*p];
        *p += 1;
        result |= (b as u32 & 0x7f) << shift;
        if b & 0x80 == 0 {
            return Some(result);
        }
        shift += 7;
        if shift > 28 {
            return None;
        }
    }
    None
}

fn read_u16_le(data: &[u8]) -> u16 {
    u16::from_le_bytes([data[0], data[1]])
}

fn read_u32_le(data: &[u8]) -> u32 {
    u32::from_le_bytes([data[0], data[1], data[2], data[3]])
}

pub fn find_section_id(wasm: &[u8], want_id: u8) -> Option<&[u8]> {
    if wasm.len() < 8 || &wasm[0..4] != b"\0asm" {
        return None;
    }
    let mut p = 8usize;
    let end = wasm.len();
    while p < end {
        let id = wasm[p];
        p += 1;
        let mut ip = p;
        let size = read_uleb(&mut ip, end, wasm)? as usize;
        if ip + size > end {
            return None;
        }
        if id == want_id {
            return Some(&wasm[ip..ip + size]);
        }
        p = ip + size;
    }
    None
}

pub fn find_custom_section<'a>(buf: &'a [u8], name: &str) -> Option<&'a [u8]> {
    if buf.len() < 8 || buf[0] != 0 {
        return None;
    }
    if &buf[1..4] == b"asm" {
        let mut p = 8usize;
        let end = buf.len();
        while p < end {
            let id = buf[p];
            p += 1;
            let mut ip = p;
            let size = read_uleb(&mut ip, end, buf)? as usize;
            if ip + size > end {
                return None;
            }
            p = ip + size;
            if id != 0 {
                continue;
            }
            let sec = &buf[ip..ip + size];
            let mut q = 0usize;
            let name_len = read_uleb(&mut q, sec.len(), sec)? as usize;
            if q + name_len > sec.len() {
                continue;
            }
            if let Ok(sec_name) = std::str::from_utf8(&sec[q..q + name_len]) {
                if sec_name == name {
                    return Some(&sec[q + name_len..]);
                }
            }
        }
        return None;
    }
    if &buf[1..4] == b"aot" {
        let want = name.as_bytes();
        let mut p = 8usize;
        while p + 8 <= buf.len() {
            let typ = u32::from_le_bytes(buf[p..p + 4].try_into().ok()?);
            let size = u32::from_le_bytes(buf[p + 4..p + 8].try_into().ok()?) as usize;
            let content = p + 8;
            let end = content + size;
            if end > buf.len() || size > 0x1000_0000 {
                return None;
            }
            if typ == 100 && size >= 6 {
                let sub = u32::from_le_bytes(buf[content..content + 4].try_into().ok()?);
                if sub == 0 {
                    let slen =
                        u16::from_le_bytes(buf[content + 4..content + 6].try_into().ok()?) as usize;
                    let nb = content + 6;
                    if nb + slen <= end {
                        let name_bytes = &buf[nb..nb + slen];
                        let bare = name_bytes.strip_suffix(&[0]).unwrap_or(name_bytes);
                        if bare == want {
                            return Some(&buf[nb + slen..end]);
                        }
                    }
                }
            }
            // Next header is 4-aligned (WAMR read_uint32 align_ptr).
            let aligned = (end + 3) & !3;
            p = if aligned <= buf.len() {
                aligned
            } else {
                buf.len()
            };
        }
        return None;
    }
    None
}

pub fn pack_find_section(wasm: &[u8]) -> Option<&[u8]> {
    find_custom_section(wasm, PACK_SECTION)
}

pub fn imports_find_section(wasm: &[u8]) -> Option<&[u8]> {
    find_custom_section(wasm, IMPORTS_SECTION)
}

pub fn pack_parse(payload: &[u8]) -> Option<PackInfo<'_>> {
    if payload.len() < 14 || &payload[0..4] != PACK_MAGIC {
        return None;
    }
    let version = read_u16_le(&payload[4..6]);
    let flags = read_u16_le(&payload[6..8]);
    if !(1..=3).contains(&version) {
        return None;
    }
    let name_len = read_u16_le(&payload[8..10]) as usize;
    if 10 + name_len + 4 > payload.len() {
        return None;
    }
    let name = std::str::from_utf8(&payload[10..10 + name_len]).ok()?;
    let mut p = 10 + name_len;
    let n_files = read_u32_le(&payload[p..p + 4]) as usize;
    p += 4;
    if n_files > 1024 {
        return None;
    }
    let v3 = version >= 3;
    let mut files = Vec::with_capacity(n_files);
    for _ in 0..n_files {
        if p + 2 > payload.len() {
            return None;
        }
        let path_len = read_u16_le(&payload[p..p + 2]) as usize;
        p += 2;
        let hdr = path_len + 1 + 4 + if v3 { 1 + 4 } else { 0 };
        if p + hdr > payload.len() {
            return None;
        }
        let path = std::str::from_utf8(&payload[p..p + path_len]).ok()?;
        p += path_len;
        let kind = payload[p];
        p += 1;
        let (fflags, mut raw_len) = if v3 {
            let f = payload[p];
            p += 1;
            let r = read_u32_le(&payload[p..p + 4]);
            p += 4;
            (f, r)
        } else {
            (0u8, 0u32)
        };
        let data_len = read_u32_le(&payload[p..p + 4]) as usize;
        p += 4;
        if p + data_len > payload.len() {
            return None;
        }
        if !v3 {
            raw_len = data_len as u32;
        }
        files.push(PackFile {
            path,
            kind,
            flags: fflags,
            raw_len,
            data: &payload[p..p + data_len],
        });
        p += data_len;
    }
    let mut exports = Vec::new();
    if version >= 2 {
        if p + 4 > payload.len() {
            return None;
        }
        let n_exports = read_u32_le(&payload[p..p + 4]) as usize;
        p += 4;
        if n_exports > 1024 {
            return None;
        }
        for _ in 0..n_exports {
            if p + 2 > payload.len() {
                return None;
            }
            let module_len = read_u16_le(&payload[p..p + 2]) as usize;
            p += 2;
            if p + module_len + 2 > payload.len() {
                return None;
            }
            let module = std::str::from_utf8(&payload[p..p + module_len]).ok()?;
            p += module_len;
            let func_len = read_u16_le(&payload[p..p + 2]) as usize;
            p += 2;
            if p + func_len + 2 > payload.len() {
                return None;
            }
            let func = std::str::from_utf8(&payload[p..p + func_len]).ok()?;
            p += func_len;
            let export_len = read_u16_le(&payload[p..p + 2]) as usize;
            p += 2;
            if p + export_len + 1 > payload.len() {
                return None;
            }
            let export_name = std::str::from_utf8(&payload[p..p + export_len]).ok()?;
            p += export_len;
            let sig = payload[p];
            p += 1;
            exports.push(PackExport {
                module,
                func,
                export_name,
                sig,
            });
        }
    }
    Some(PackInfo {
        name,
        version,
        flags,
        files,
        exports,
    })
}

/// Inflate a pack file entry when zlib-flagged.
pub fn pack_file_bytes<'a>(f: &PackFile<'a>) -> Option<std::borrow::Cow<'a, [u8]>> {
    if f.flags & PACK_FILE_FLAG_ZLIB == 0 {
        return Some(std::borrow::Cow::Borrowed(f.data));
    }
    use flate2::read::ZlibDecoder;
    use std::io::Read;
    let mut dec = ZlibDecoder::new(f.data);
    let mut out = Vec::with_capacity(f.raw_len as usize);
    dec.read_to_end(&mut out).ok()?;
    if out.len() != f.raw_len as usize {
        return None;
    }
    Some(std::borrow::Cow::Owned(out))
}

/// Unwrap MPZL whole-artifact envelope when present.
pub fn artifact_unwrap_zlib(buf: &[u8]) -> Option<std::borrow::Cow<'_, [u8]>> {
    if buf.len() < 8 || &buf[0..4] != ARTIFACT_ZLIB_MAGIC {
        return Some(std::borrow::Cow::Borrowed(buf));
    }
    let raw_len = read_u32_le(&buf[4..8]) as usize;
    if raw_len == 0 || raw_len > 64 * 1024 * 1024 {
        return None;
    }
    use flate2::read::ZlibDecoder;
    use std::io::Read;
    let mut dec = ZlibDecoder::new(&buf[8..]);
    let mut out = Vec::with_capacity(raw_len);
    dec.read_to_end(&mut out).ok()?;
    if out.len() != raw_len {
        return None;
    }
    Some(std::borrow::Cow::Owned(out))
}

pub fn imports_parse(payload: &[u8]) -> Option<ImportsInfo<'_>> {
    if payload.len() < 10 || &payload[0..4] != IMPORTS_MAGIC {
        return None;
    }
    let version = read_u16_le(&payload[4..6]);
    if version != 1 {
        return None;
    }
    let n = read_u32_le(&payload[6..10]) as usize;
    if n > 1024 {
        return None;
    }
    let mut p = 10usize;
    let mut imports = Vec::with_capacity(n);
    for _ in 0..n {
        if p + 2 > payload.len() {
            return None;
        }
        let module_len = read_u16_le(&payload[p..p + 2]) as usize;
        p += 2;
        if p + module_len + 2 > payload.len() {
            return None;
        }
        let module = std::str::from_utf8(&payload[p..p + module_len]).ok()?;
        p += module_len;
        let func_len = read_u16_le(&payload[p..p + 2]) as usize;
        p += 2;
        if p + func_len > payload.len() {
            return None;
        }
        let func = std::str::from_utf8(&payload[p..p + func_len]).ok()?;
        p += func_len;
        imports.push(ImportEntry { module, func });
    }
    Some(ImportsInfo { version, imports })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_uleb_single_byte() {
        let data = [0x05u8];
        let mut p = 0;
        assert_eq!(read_uleb(&mut p, 1, &data), Some(5));
    }

    #[test]
    fn find_custom_section_roundtrip() {
        let payload = build_pack_payload("demo", &[("x.py", PACK_KIND_PY, b"x=1")]);
        let wasm = build_wasm_with_custom_section(PACK_SECTION, &payload);
        let found = pack_find_section(&wasm).expect("pack section");
        let info = pack_parse(found).expect("parse pack");
        assert_eq!(info.name, "demo");
        assert_eq!(info.files.len(), 1);
    }

    fn encode_uleb(mut v: u32) -> Vec<u8> {
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

    fn build_pack_payload(name: &str, files: &[(&str, u8, &[u8])]) -> Vec<u8> {
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
            p.push(0); // flags
            p.extend_from_slice(&(data.len() as u32).to_le_bytes()); // raw_len
            p.extend_from_slice(&(data.len() as u32).to_le_bytes());
            p.extend_from_slice(data);
        }
        p.extend_from_slice(&0u32.to_le_bytes()); // n_exports
        p
    }

    fn build_wasm_with_custom_section(section_name: &str, payload: &[u8]) -> Vec<u8> {
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

    #[test]
    fn section_names_match_upstream() {
        assert_eq!(PACK_SECTION, "wasmmod.pack");
        assert_eq!(IMPORTS_SECTION, "wasmmod.imports");
        assert_eq!(SIG_SECTION, "wasmmod.sig");
        assert_eq!(HOST_MODULE, "wasmmod.host");
        assert_eq!(WASM_MODULE, "wasmmod");
    }

    #[test]
    fn pack_parse_rejects_bad_magic() {
        assert!(pack_parse(b"XXXX").is_none());
    }
}
