//! Resolve drivers: map ``name@version`` → artifact bytes (path layout or metal-cdn).

use std::sync::{Mutex, OnceLock};

use super::fetch;
use super::finder;
use super::resolve::{DepNode, DepSource};
use super::runtime::{self, MP_WASM_ERRBUF};

/// Where an artifact can be fetched from.
#[derive(Debug, Clone)]
pub struct ArtifactRef {
    pub uris: Vec<String>,
    pub channel: Option<String>,
}

/// Pluggable package resolver (CDN / VFS / URL root).
pub trait CdnDriver: Send {
    fn name(&self) -> &'static str;
    /// Candidate URIs for ``node`` (tried in order until one fetches).
    fn resolve(&self, node: &DepNode) -> Result<ArtifactRef, String>;
    /// Whether missing MPWD for an MPWI peer is a hard error.
    fn require_explicit_deps(&self) -> bool {
        false
    }
}

/// Current driver + optional Bearer token for metal-cdn HTTP.
struct CdnState {
    driver: Box<dyn CdnDriver>,
    token: Option<String>,
    base_url: Option<String>,
}

fn state() -> &'static Mutex<CdnState> {
    static STATE: OnceLock<Mutex<CdnState>> = OnceLock::new();
    STATE.get_or_init(|| {
        Mutex::new(CdnState {
            driver: Box::new(PathDriver),
            token: None,
            base_url: None,
        })
    })
}

/// Install a driver (Path or Metal CDN).
pub fn set_driver(driver: Box<dyn CdnDriver>) {
    let mut g = state().lock().unwrap();
    g.driver = driver;
}

pub fn set_token(token: Option<String>) {
    state().lock().unwrap().token = token.filter(|t| !t.is_empty());
}

pub fn set_base_url(url: Option<String>) {
    let mut g = state().lock().unwrap();
    g.base_url = url.map(|u| {
        let mut s = u;
        while s.ends_with('/') {
            s.pop();
        }
        s
    });
}

pub fn token() -> Option<String> {
    state().lock().unwrap().token.clone()
}

pub fn base_url() -> Option<String> {
    state().lock().unwrap().base_url.clone()
}

pub fn driver_name() -> String {
    state().lock().unwrap().driver.name().to_string()
}

pub fn require_explicit_deps() -> bool {
    state().lock().unwrap().driver.require_explicit_deps()
}

/// Resolve + fetch one package pin.
pub fn fetch_node(node: &DepNode) -> Result<Vec<u8>, String> {
    let (uris, token) = {
        let g = state().lock().unwrap();
        let art = g.driver.resolve(node)?;
        (art.uris, g.token.clone())
    };
    if uris.is_empty() {
        return Err(format!("cdn: no URIs for {}", node.key()));
    }
    let mut err = [0u8; MP_WASM_ERRBUF];
    let mut last = String::from("cdn: all candidates failed");
    for uri in &uris {
        // Bearer: only for metal-cdn HTTP; PathDriver local paths ignore token.
        if token.is_some() && fetch::uri_is_http(uri) {
            if let Some(bytes) = fetch_http_auth(uri, token.as_deref(), &mut err) {
                return Ok(bytes);
            }
        } else if let Some(bytes) = fetch::fetch(uri, &mut err) {
            return Ok(bytes);
        }
        last = err_msg(&err);
        if last.is_empty() {
            last = format!("cdn: fetch failed: {uri}");
        }
    }
    Err(last)
}

fn err_msg(errbuf: &[u8]) -> String {
    std::str::from_utf8(errbuf)
        .unwrap_or("")
        .trim_end_matches('\0')
        .to_string()
}

fn fetch_http_auth(uri: &str, token: Option<&str>, errbuf: &mut [u8]) -> Option<Vec<u8>> {
    use std::io::Read;
    let mut req = ureq::get(uri);
    if let Some(t) = token {
        req = req.set("Authorization", &format!("Bearer {t}"));
    }
    let resp = match req.call() {
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
    if resp.status() != 200 {
        runtime::set_err(errbuf, &format!("HTTP {}", resp.status()));
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

/// VFS / flat URL-root layout (current finder candidates). Version is optional
/// ``name@version`` path suffix when present on disk/CDN mirrors.
pub struct PathDriver;

impl CdnDriver for PathDriver {
    fn name(&self) -> &'static str {
        "path"
    }

    fn resolve(&self, node: &DepNode) -> Result<ArtifactRef, String> {
        let mut uris = Vec::new();
        // Prefer versioned stems when version is set: hello@0.1.0.wasm
        let versioned = if node.version.is_empty() {
            None
        } else {
            Some(format!("{}@{}", node.name, node.version))
        };
        for stem in versioned.iter().map(|s| s.as_str()).chain(std::iter::once(node.name.as_str()))
        {
            if let Some(path) = finder::find_pack(stem) {
                uris.push(path);
            }
            // Also emit relative candidate names for HTTP roots on wasm.path
            for rel in finder::candidate_rel_paths(&stem.replace('.', "/"), false, 0) {
                // join against each wasm.path root
                for root in finder::path_roots() {
                    let uri = fetch::join_uri(&root, &rel);
                    if !uris.iter().any(|u| u == &uri) {
                        uris.push(uri);
                    }
                }
            }
        }
        if uris.is_empty() {
            return Err(format!(
                "path driver: pack {} not found on wasm.path",
                node.key()
            ));
        }
        Ok(ArtifactRef {
            uris,
            channel: None,
        })
    }
}

/// metal-cdn HTTP: ``{base}/artifacts/pin/{version}/{filename}`` (and lead fallback).
pub struct MetalCdnDriver {
    pub base: String,
}

impl MetalCdnDriver {
    pub fn new(base: impl Into<String>) -> Self {
        let mut base = base.into();
        while base.ends_with('/') {
            base.pop();
        }
        Self { base }
    }

    fn looks_like_cdn_base(url: &str) -> bool {
        let u = url.trim_end_matches('/');
        u.ends_with("/cdn") || u.contains("/cdn/") || u.ends_with("cdn")
    }
}

impl CdnDriver for MetalCdnDriver {
    fn name(&self) -> &'static str {
        "metal-cdn"
    }

    fn require_explicit_deps(&self) -> bool {
        true
    }

    fn resolve(&self, node: &DepNode) -> Result<ArtifactRef, String> {
        let ver = node.version.as_str();
        let name = node.name.as_str();
        let mut filenames = Vec::new();
        for rel in finder::candidate_rel_paths(name, false, 0) {
            // artifacts API wants basename, not package/__init__.wasm path
            let base = rel.rsplit('/').next().unwrap_or(&rel);
            if !filenames.iter().any(|f| f == base) {
                filenames.push(base.to_string());
            }
        }
        // Common flat names
        for ext in [
            ".wasm.zlib",
            ".wasm",
            ".aot.zlib",
            ".aot",
        ] {
            let f = format!("{name}{ext}");
            if !filenames.iter().any(|x| x == &f) {
                filenames.push(f);
            }
        }

        let mut uris = Vec::new();
        if !ver.is_empty() {
            for f in &filenames {
                uris.push(format!("{}/artifacts/pin/{ver}/{f}", self.base));
            }
        }
        for f in &filenames {
            uris.push(format!("{}/artifacts/lead/{f}", self.base));
        }
        Ok(ArtifactRef {
            uris,
            channel: if ver.is_empty() {
                Some("lead".into())
            } else {
                Some(format!("@{ver}"))
            },
        })
    }
}

/// Configure driver from ``install_hook`` / ``wasm.cdn`` URL.
pub fn configure_from_url(url: &str, token: Option<&str>) {
    set_token(token.map(str::to_string));
    set_base_url(Some(url.to_string()));
    if MetalCdnDriver::looks_like_cdn_base(url) {
        set_driver(Box::new(MetalCdnDriver::new(url)));
    } else {
        // Flat pack root — keep PathDriver; URL is already on wasm.path.
        set_driver(Box::new(PathDriver));
    }
}

/// Reset to PathDriver (tests / uninstall).
pub fn reset_to_path() {
    set_token(None);
    set_base_url(None);
    set_driver(Box::new(PathDriver));
}

/// ``DepSource`` that fetches each unseen node via the active driver and
/// reads MPWD from the artifact (cached).
pub struct FetchingDepSource {
    pub cache: std::collections::HashMap<String, Vec<u8>>,
}

impl FetchingDepSource {
    pub fn new() -> Self {
        Self {
            cache: std::collections::HashMap::new(),
        }
    }

    pub fn ensure(&mut self, node: &DepNode) -> Result<&[u8], String> {
        let key = node.key();
        if !self.cache.contains_key(&key) {
            let bytes = fetch_node(node)?;
            self.cache.insert(key.clone(), bytes);
        }
        Ok(self.cache.get(&key).unwrap().as_slice())
    }
}

impl Default for FetchingDepSource {
    fn default() -> Self {
        Self::new()
    }
}

impl DepSource for FetchingDepSource {
    fn deps_of(&mut self, node: &DepNode) -> Result<Vec<DepNode>, String> {
        let bytes = self.ensure(node)?;
        Ok(super::resolve::deps_from_artifact(bytes))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metal_cdn_resolve_pin_urls() {
        let d = MetalCdnDriver::new("http://127.0.0.1:8000/cdn");
        let art = d
            .resolve(&DepNode::new("hello", "0.1.0"))
            .expect("resolve");
        assert!(art
            .uris
            .iter()
            .any(|u| u.contains("/artifacts/pin/0.1.0/hello.wasm")));
        assert!(art
            .uris
            .iter()
            .any(|u| u.contains("/artifacts/lead/hello.wasm")));
    }

    #[test]
    fn looks_like_cdn() {
        assert!(MetalCdnDriver::looks_like_cdn_base("http://x/cdn"));
        assert!(MetalCdnDriver::looks_like_cdn_base("http://x/cdn/"));
        assert!(!MetalCdnDriver::looks_like_cdn_base("http://x/packs/"));
    }
}
