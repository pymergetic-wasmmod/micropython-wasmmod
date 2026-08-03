//! rewrite of extmod/modtls_mbedtls.c
//! Unix host uses rustls (not mbedTLS); upstream `extmod/mbedtls/` is intentionally out of scope.
//! Host-complete: `SSLContext`, `wrap_socket`, `getpeercert`, `verify_callback`, `get_ciphers`/`set_ciphers`.
//! DTLS/PSK/`ecdsa_sign_callback` are mbedTLS-only upstream features (`PY_SSL_DTLS`/`PY_SSL_ECDSA_SIGN_ALT` off).
// symmetry: done

use std::io::{self, Read, Write};
use std::sync::{Arc, Once};

use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::client::WebPkiServerVerifier;
use rustls::crypto::CryptoProvider;
use rustls::pki_types::{CertificateDer, PrivateKeyDer, ServerName, UnixTime};
use rustls::server::WebPkiClientVerifier;
use rustls::{
    CipherSuite, ClientConfig, ClientConnection, CommonState, Error as RustlsError,
    ProtocolVersion, RootCertStore, ServerConfig, ServerConnection, SignatureScheme,
    SupportedCipherSuite,
};
use rustls_pemfile::certs;
use x509_parser::objects::{oid2sn, oid_registry};
use x509_parser::prelude::{FromDer, X509Certificate, X509Name};

use py_rs::argcheck::{self, Arg, ArgFlag, ArgVal};
use py_rs::bc::ModuleContext;
use py_rs::malloc;
use py_rs::map::{self, LookupKind, Map, MapElem};
use py_rs::mpconfig;
use py_rs::obj::{
    self, BufferInfo, Obj, ObjBase, ObjType, TYPE_FLAG_BINDS_SELF, TYPE_FLAG_BUILTIN_FUN,
    TYPE_FLAG_ITER_IS_STREAM,
};
use py_rs::objdict;
use py_rs::objexcept;
use py_rs::objlist;
use py_rs::objmodule;
use py_rs::objstr;
use py_rs::objtuple;
use py_rs::qstr::{self, Qstr};
use py_rs::raise::{self, MpRaise};
use py_rs::runtime::{self, HandlePendingBehaviour};
use py_rs::stream::{
    self, StreamIoFn, StreamIoctlFn, StreamP, STREAM_CLOSE, STREAM_ERROR, STREAM_OP_IOCTL,
    STREAM_OP_READ, STREAM_OP_WRITE, STREAM_POLL, STREAM_POLL_ERR, STREAM_POLL_HUP,
    STREAM_POLL_NVAL, STREAM_POLL_RD, STREAM_POLL_WR,
};

const MP_ENDPOINT_IS_SERVER: i32 = 1;
const MP_TRANSPORT_IS_DTLS: i32 = 2;

const PROTOCOL_TLS_CLIENT: i32 = 0;
const PROTOCOL_TLS_SERVER: i32 = MP_ENDPOINT_IS_SERVER;
const PROTOCOL_DTLS_CLIENT: i32 = MP_TRANSPORT_IS_DTLS;
const PROTOCOL_DTLS_SERVER: i32 = MP_ENDPOINT_IS_SERVER | MP_TRANSPORT_IS_DTLS;

const CERT_NONE: i32 = 0;
const CERT_OPTIONAL: i32 = 1;
const CERT_REQUIRED: i32 = 2;

const MBEDTLS_ERR_SSL_BAD_CONFIG: i32 = -0x5E80;

type BuiltinFn1 = fn(Obj) -> Obj;
type BuiltinFn2 = fn(Obj, Obj) -> Obj;
type BuiltinFnVar = fn(usize, &[Obj]) -> Obj;
type BuiltinFnKw = fn(usize, &[Obj], &Map) -> Obj;

#[repr(C)]
struct ObjFunBuiltin1 {
    base: ObjBase,
    fun: BuiltinFn1,
}

#[repr(C)]
struct ObjFunBuiltin2 {
    base: ObjBase,
    fun: BuiltinFn2,
}

#[repr(C)]
struct ObjFunBuiltinKw {
    base: ObjBase,
    min_args: u8,
    fun: BuiltinFnKw,
}

static mut F1: [*const (); 1] = [call1 as *const ()];
static mut F2: [*const (); 1] = [call2 as *const ()];
#[repr(C)]
struct ObjFunBuiltinVar {
    base: ObjBase,
    min_args: u8,
    max_args: u8,
    fun: BuiltinFnVar,
}

static mut FV: [*const (); 1] = [callv as *const ()];

static T1: ObjType = ObjType {
    base: ObjBase {
        type_: core::ptr::null(),
    },
    flags: TYPE_FLAG_BINDS_SELF | TYPE_FLAG_BUILTIN_FUN,
    name: 0,
    slot_index_make_new: 0,
    slot_index_print: 0,
    slot_index_call: 1,
    slot_index_unary_op: 0,
    slot_index_binary_op: 0,
    slot_index_attr: 0,
    slot_index_subscr: 0,
    slot_index_iter: 0,
    slot_index_buffer: 0,
    slot_index_protocol: 0,
    slot_index_parent: 0,
    slot_index_locals_dict: 0,
    slots: unsafe { F1.as_ptr() },
};

static T2: ObjType = ObjType {
    base: ObjBase {
        type_: core::ptr::null(),
    },
    flags: TYPE_FLAG_BINDS_SELF | TYPE_FLAG_BUILTIN_FUN,
    name: 0,
    slot_index_make_new: 0,
    slot_index_print: 0,
    slot_index_call: 1,
    slot_index_unary_op: 0,
    slot_index_binary_op: 0,
    slot_index_attr: 0,
    slot_index_subscr: 0,
    slot_index_iter: 0,
    slot_index_buffer: 0,
    slot_index_protocol: 0,
    slot_index_parent: 0,
    slot_index_locals_dict: 0,
    slots: unsafe { F2.as_ptr() },
};

static mut FK: [*const (); 1] = [call_kw as *const ()];

static TV: ObjType = ObjType {
    base: ObjBase {
        type_: core::ptr::null(),
    },
    flags: TYPE_FLAG_BINDS_SELF | TYPE_FLAG_BUILTIN_FUN,
    name: 0,
    slot_index_make_new: 0,
    slot_index_print: 0,
    slot_index_call: 1,
    slot_index_unary_op: 0,
    slot_index_binary_op: 0,
    slot_index_attr: 0,
    slot_index_subscr: 0,
    slot_index_iter: 0,
    slot_index_buffer: 0,
    slot_index_protocol: 0,
    slot_index_parent: 0,
    slot_index_locals_dict: 0,
    slots: unsafe { FV.as_ptr() },
};

static TK: ObjType = ObjType {
    base: ObjBase {
        type_: core::ptr::null(),
    },
    flags: TYPE_FLAG_BINDS_SELF | TYPE_FLAG_BUILTIN_FUN,
    name: 0,
    slot_index_make_new: 0,
    slot_index_print: 0,
    slot_index_call: 1,
    slot_index_unary_op: 0,
    slot_index_binary_op: 0,
    slot_index_attr: 0,
    slot_index_subscr: 0,
    slot_index_iter: 0,
    slot_index_buffer: 0,
    slot_index_protocol: 0,
    slot_index_parent: 0,
    slot_index_locals_dict: 0,
    slots: unsafe { FK.as_ptr() },
};

fn call1(s: Obj, n: usize, k: usize, a: &[Obj]) -> Obj {
    argcheck::check_num(n, k, 1, 1, false);
    unsafe { ((*(obj::as_ptr(s) as *const ObjFunBuiltin1)).fun)(a[0]) }
}

fn call2(s: Obj, n: usize, k: usize, a: &[Obj]) -> Obj {
    argcheck::check_num(n, k, 2, 2, false);
    unsafe { ((*(obj::as_ptr(s) as *const ObjFunBuiltin2)).fun)(a[0], a[1]) }
}

fn callv(s: Obj, n: usize, k: usize, a: &[Obj]) -> Obj {
    let self_ = unsafe { &*(obj::as_ptr(s) as *const ObjFunBuiltinVar) };
    argcheck::check_num(
        n,
        k,
        self_.min_args as usize,
        self_.max_args as usize,
        false,
    );
    (self_.fun)(n, a)
}

fn call_kw(s: Obj, n: usize, k: usize, a: &[Obj]) -> Obj {
    let self_ = unsafe { &*(obj::as_ptr(s) as *const ObjFunBuiltinKw) };
    if n < self_.min_args as usize {
        raise::raise(MpRaise::TypeError("argument num/types mismatch"));
    }
    let mut kw = Map::default();
    map::init(&mut kw, k);
    for i in 0..k {
        let key = a[n + i * 2];
        let val = a[n + i * 2 + 1];
        if let Some(slot) = map::lookup(&mut kw, key, LookupKind::AddIfNotFound) {
            slot.value = val;
        }
    }
    (self_.fun)(n, &a[..n], &kw)
}

fn mk1(f: BuiltinFn1) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltin1>().expect("tls fn1");
    unsafe {
        (*o).base.type_ = &T1;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltin1 as *const ())
    }
}

fn mk2(f: BuiltinFn2) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltin2>().expect("tls fn2");
    unsafe {
        (*o).base.type_ = &T2;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltin2 as *const ())
    }
}

fn mkv(min: u8, max: u8, f: BuiltinFnVar) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltinVar>().expect("tls fnv");
    unsafe {
        (*o).base.type_ = &TV;
        (*o).min_args = min;
        (*o).max_args = max;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltinVar as *const ())
    }
}

fn mk_kw(min: u8, f: BuiltinFnKw) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltinKw>().expect("tls fnkw");
    unsafe {
        (*o).base.type_ = &TK;
        (*o).min_args = min;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltinKw as *const ())
    }
}

static CRYPTO_INIT: Once = Once::new();

fn init_crypto() {
    CRYPTO_INIT.call_once(|| {
        rustls::crypto::ring::default_provider()
            .install_default()
            .expect("rustls crypto provider");
    });
}

#[derive(Debug)]
struct SkipServerVerifier;

impl ServerCertVerifier for SkipServerVerifier {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, RustlsError> {
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, RustlsError> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, RustlsError> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        vec![
            SignatureScheme::RSA_PKCS1_SHA256,
            SignatureScheme::RSA_PKCS1_SHA384,
            SignatureScheme::RSA_PKCS1_SHA512,
            SignatureScheme::ECDSA_NISTP256_SHA256,
            SignatureScheme::ECDSA_NISTP384_SHA384,
            SignatureScheme::ECDSA_NISTP521_SHA512,
            SignatureScheme::RSA_PSS_SHA256,
            SignatureScheme::RSA_PSS_SHA384,
            SignatureScheme::RSA_PSS_SHA512,
            SignatureScheme::ED25519,
            SignatureScheme::ED448,
        ]
    }
}

#[derive(Debug)]
struct OptionalServerVerifier {
    inner: Arc<WebPkiServerVerifier>,
}

impl ServerCertVerifier for OptionalServerVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        intermediates: &[CertificateDer<'_>],
        server_name: &ServerName<'_>,
        ocsp_response: &[u8],
        now: UnixTime,
    ) -> Result<ServerCertVerified, RustlsError> {
        let _ = self.inner.verify_server_cert(
            end_entity,
            intermediates,
            server_name,
            ocsp_response,
            now,
        );
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, RustlsError> {
        self.inner
            .verify_tls12_signature(message, cert, dss)
            .or(Ok(HandshakeSignatureValid::assertion()))
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, RustlsError> {
        self.inner
            .verify_tls13_signature(message, cert, dss)
            .or(Ok(HandshakeSignatureValid::assertion()))
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.inner.supported_verify_schemes()
    }
}

fn ring_cipher_suites() -> &'static [SupportedCipherSuite] {
    rustls::crypto::ring::ALL_CIPHER_SUITES
}

fn cipher_suite_name(suite: CipherSuite) -> String {
    let raw = format!("{:?}", suite);
    if let Some(rest) = raw.strip_prefix("TLS13_") {
        return format!("TLS-{}", rest.replace('_', "-"));
    }
    raw.replace('_', "-")
}

fn find_cipher_by_name(name: &str) -> Option<CipherSuite> {
    let upper = name.to_ascii_uppercase();
    for cs in ring_cipher_suites() {
        let canonical = cipher_suite_name(cs.suite());
        if canonical.eq_ignore_ascii_case(&upper) {
            return Some(cs.suite());
        }
    }
    None
}

fn make_crypto_provider(ciphers: Option<&[CipherSuite]>) -> Arc<CryptoProvider> {
    let default = rustls::crypto::ring::default_provider();
    let cipher_suites = match ciphers {
        None => default.cipher_suites,
        Some(list) => list
            .iter()
            .filter_map(|id| {
                default
                    .cipher_suites
                    .iter()
                    .find(|cs| cs.suite() == *id)
                    .copied()
            })
            .collect(),
    };
    Arc::new(CryptoProvider {
        cipher_suites,
        kx_groups: default.kx_groups,
        signature_verification_algorithms: default.signature_verification_algorithms,
        secure_random: default.secure_random,
        key_provider: default.key_provider,
    })
}

fn protocol_version_str(version: ProtocolVersion) -> &'static str {
    match version {
        ProtocolVersion::TLSv1_2 => "TLSv1.2",
        ProtocolVersion::TLSv1_3 => "TLSv1.3",
        _ => "unknown",
    }
}

fn raise_ssl_bad_config() -> ! {
    let code = obj::new_small_int(MBEDTLS_ERR_SSL_BAD_CONFIG as obj::Int);
    let msg = objstr::new_str(b"MBEDTLS_ERR_SSL_BAD_CONFIG");
    raise::raise_obj(objexcept::new_exception_args(
        objexcept::type_os_error(),
        2,
        &[code, msg],
    ));
}

fn read_pem_bytes(obj: Obj) -> Vec<u8> {
    let mut info = BufferInfo::default();
    if obj::get_buffer(obj, &mut info, obj::BUFFER_READ) {
        return unsafe { std::slice::from_raw_parts(info.buf as *const u8, info.len).to_vec() };
    }
    let (data, len) = objstr::get_str_data_len(obj);
    data[..len].to_vec()
}

fn parse_certs(pem: &[u8]) -> Vec<CertificateDer<'static>> {
    let mut reader = std::io::Cursor::new(pem);
    certs(&mut reader)
        .filter_map(|c| c.ok())
        .map(|c| c.into_owned())
        .collect()
}

fn parse_private_key(pem: &[u8]) -> PrivateKeyDer<'static> {
    let mut reader = std::io::Cursor::new(pem);
    match rustls_pemfile::private_key(&mut reader) {
        Ok(Some(key)) => key,
        _ => raise::raise(MpRaise::ValueError("invalid key")),
    }
}

fn int_const(v: i32) -> Obj {
    obj::new_small_int(v as obj::Int)
}

fn raise_io_error(err: io::Error) -> ! {
    if err.kind() == io::ErrorKind::WouldBlock {
        raise::raise(MpRaise::OSError(11));
    }
    if let Some(code) = err.raw_os_error() {
        raise::raise(MpRaise::OSError(code));
    }
    let code = obj::new_small_int(-1);
    let msg = objstr::new_str(err.to_string().as_bytes());
    raise::raise_obj(objexcept::new_exception_args(
        objexcept::type_os_error(),
        2,
        &[code, msg],
    ));
}

fn raise_tls_error(err: RustlsError) -> ! {
    let code = obj::new_small_int(-1);
    let msg = objstr::new_str(err.to_string().as_bytes());
    raise::raise_obj(objexcept::new_exception_args(
        objexcept::type_os_error(),
        2,
        &[code, msg],
    ));
}

fn build_root_store(extra_ca: &[Vec<u8>]) -> RootCertStore {
    let mut roots = RootCertStore::empty();
    roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    for pem in extra_ca {
        let mut reader = std::io::Cursor::new(pem.as_slice());
        for cert in certs(&mut reader).filter_map(|c| c.ok()) {
            let _ = roots.add(cert);
        }
    }
    roots
}

#[repr(C)]
struct ObjSslContext {
    base: ObjBase,
    protocol: i32,
    verify_mode: i32,
    verify_callback: Obj,
    cert_pem: Option<Vec<u8>>,
    key_pem: Option<Vec<u8>>,
    ca_pem: Vec<Vec<u8>>,
    ciphers: Option<Vec<CipherSuite>>,
}

fn ctx_ptr(o: Obj) -> *mut ObjSslContext {
    obj::as_ptr(o) as *mut ObjSslContext
}

fn is_server(protocol: i32) -> bool {
    (protocol & MP_ENDPOINT_IS_SERVER) != 0
}

fn is_dtls(protocol: i32) -> bool {
    (protocol & MP_TRANSPORT_IS_DTLS) != 0
}

fn call_verify_callback(
    callback: Obj,
    cert: &CertificateDer<'_>,
    depth: i32,
) -> Result<(), RustlsError> {
    if callback == obj::CONST_NONE {
        return Ok(());
    }
    let cert_obj = objstr::new_bytes(cert.as_ref());
    let depth_obj = obj::new_small_int(depth as obj::Int);
    let ret = runtime::call_function_2(callback, cert_obj, depth_obj);
    if obj::get_int(ret) != 0 {
        return Err(RustlsError::General("verify_callback rejected".into()));
    }
    Ok(())
}

#[derive(Debug)]
struct CallbackServerVerifier {
    inner: Arc<dyn ServerCertVerifier>,
    callback: Obj,
}

impl ServerCertVerifier for CallbackServerVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        intermediates: &[CertificateDer<'_>],
        server_name: &ServerName<'_>,
        ocsp_response: &[u8],
        now: UnixTime,
    ) -> Result<ServerCertVerified, RustlsError> {
        self.inner.verify_server_cert(
            end_entity,
            intermediates,
            server_name,
            ocsp_response,
            now,
        )?;
        call_verify_callback(self.callback, end_entity, 0)?;
        for (depth, cert) in intermediates.iter().enumerate() {
            call_verify_callback(self.callback, cert, (depth + 1) as i32)?;
        }
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, RustlsError> {
        self.inner.verify_tls12_signature(message, cert, dss)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, RustlsError> {
        self.inner.verify_tls13_signature(message, cert, dss)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.inner.supported_verify_schemes()
    }
}

fn wrap_verifier(inner: Arc<dyn ServerCertVerifier>, callback: Obj) -> Arc<dyn ServerCertVerifier> {
    if callback == obj::CONST_NONE {
        inner
    } else {
        Arc::new(CallbackServerVerifier { inner, callback })
    }
}

fn build_client_config(ctx: &ObjSslContext) -> Arc<ClientConfig> {
    init_crypto();
    let provider = make_crypto_provider(ctx.ciphers.as_deref());
    let builder = ClientConfig::builder_with_provider(provider.clone())
        .with_safe_default_protocol_versions()
        .expect("cipher suites");

    let base_verifier: Arc<dyn ServerCertVerifier> = if ctx.verify_mode == CERT_NONE {
        Arc::new(SkipServerVerifier)
    } else {
        let roots = build_root_store(&ctx.ca_pem);
        if ctx.verify_mode == CERT_OPTIONAL {
            let inner = WebPkiServerVerifier::builder_with_provider(Arc::new(roots), provider)
                .build()
                .expect("server verifier");
            Arc::new(OptionalServerVerifier { inner })
        } else {
            WebPkiServerVerifier::builder_with_provider(Arc::new(roots), provider)
                .build()
                .expect("server verifier")
        }
    };
    let verifier = wrap_verifier(base_verifier, ctx.verify_callback);

    let builder = builder
        .dangerous()
        .with_custom_certificate_verifier(verifier);

    if let (Some(cert_pem), Some(key_pem)) = (&ctx.cert_pem, &ctx.key_pem) {
        let certs = parse_certs(cert_pem);
        let key = parse_private_key(key_pem);
        Arc::new(
            builder
                .with_client_auth_cert(certs, key)
                .expect("client cert"),
        )
    } else {
        Arc::new(builder.with_no_client_auth())
    }
}

fn build_server_config(ctx: &ObjSslContext) -> Arc<ServerConfig> {
    init_crypto();
    let provider = make_crypto_provider(ctx.ciphers.as_deref());
    let cert_pem = ctx.cert_pem.as_ref().expect("server requires cert");
    let key_pem = ctx.key_pem.as_ref().expect("server requires key");
    let certs = parse_certs(cert_pem);
    let key = parse_private_key(key_pem);
    let builder = ServerConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .expect("cipher suites");

    if ctx.verify_mode == CERT_NONE {
        Arc::new(
            builder
                .with_no_client_auth()
                .with_single_cert(certs, key)
                .expect("server cert"),
        )
    } else {
        let roots = build_root_store(&ctx.ca_pem);
        let mut client_builder = WebPkiClientVerifier::builder(Arc::new(roots));
        if ctx.verify_mode == CERT_OPTIONAL {
            client_builder = client_builder.allow_unauthenticated();
        }
        let client_verifier = client_builder.build().expect("client verifier");
        Arc::new(
            builder
                .with_client_cert_verifier(client_verifier)
                .with_single_cert(certs, key)
                .expect("server cert"),
        )
    }
}

struct StreamAdapter {
    sock: Obj,
    blocking: bool,
}

impl Read for StreamAdapter {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        loop {
            runtime::handle_pending(HandlePendingBehaviour::CallbacksAndClearExceptions);
            let mut errcode = 0i32;
            let stream_p = stream::get_stream(self.sock);
            let read = stream_p.read.expect("socket read");
            let n = read(self.sock, buf.as_mut_ptr(), buf.len(), &mut errcode);
            if n == STREAM_ERROR {
                if stream::is_nonblocking_error(errcode) {
                    if self.blocking {
                        continue;
                    }
                    return Err(io::Error::from(io::ErrorKind::WouldBlock));
                }
                return Err(io::Error::from_raw_os_error(errcode));
            }
            return Ok(n);
        }
    }
}

impl Write for StreamAdapter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        loop {
            runtime::handle_pending(HandlePendingBehaviour::CallbacksAndClearExceptions);
            let mut errcode = 0i32;
            let stream_p = stream::get_stream(self.sock);
            let write = stream_p.write.expect("socket write");
            let n = write(self.sock, buf.as_ptr(), buf.len(), &mut errcode);
            if n == STREAM_ERROR {
                if stream::is_nonblocking_error(errcode) {
                    if self.blocking {
                        continue;
                    }
                    return Err(io::Error::from(io::ErrorKind::WouldBlock));
                }
                return Err(io::Error::from_raw_os_error(errcode));
            }
            return Ok(n);
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

enum TlsConn {
    Client(ClientConnection),
    Server(ServerConnection),
}

impl TlsConn {
    fn complete_io(&mut self, io: &mut StreamAdapter) -> Result<(usize, usize), io::Error> {
        match self {
            TlsConn::Client(c) => c.complete_io(io),
            TlsConn::Server(s) => s.complete_io(io),
        }
    }

    fn reader(&mut self) -> rustls::Reader<'_> {
        match self {
            TlsConn::Client(c) => c.reader(),
            TlsConn::Server(s) => s.reader(),
        }
    }

    fn writer(&mut self) -> rustls::Writer<'_> {
        match self {
            TlsConn::Client(c) => c.writer(),
            TlsConn::Server(s) => s.writer(),
        }
    }

    fn is_handshake_complete(&self) -> bool {
        match self {
            TlsConn::Client(c) => !c.is_handshaking(),
            TlsConn::Server(s) => !s.is_handshaking(),
        }
    }

    fn common(&self) -> &CommonState {
        match self {
            TlsConn::Client(c) => c,
            TlsConn::Server(s) => s,
        }
    }
}

#[repr(C)]
struct ObjSslSocket {
    base: ObjBase,
    ctx: Obj,
    sock: Obj,
    conn: *mut TlsConn,
    blocking: bool,
    poll_wr: bool,
    last_error: i32,
}

fn ssl_sock_ptr(o: Obj) -> *mut ObjSslSocket {
    obj::as_ptr(o) as *mut ObjSslSocket
}

fn drop_conn(o: &mut ObjSslSocket) {
    if !o.conn.is_null() {
        unsafe {
            drop(Box::from_raw(o.conn));
        }
        o.conn = core::ptr::null_mut();
    }
}

fn do_handshake(conn: &mut TlsConn, io: &mut StreamAdapter) {
    while !conn.is_handshake_complete() {
        match conn.complete_io(io) {
            Ok(_) => {}
            Err(e) => raise_io_error(e),
        }
    }
}

fn ssl_context_make_new(type_in: &ObjType, n_args: usize, n_kw: usize, args: &[Obj]) -> Obj {
    argcheck::check_num(n_args, n_kw, 1, 1, false);
    let protocol = obj::get_int(args[0]) as i32;
    if protocol != PROTOCOL_TLS_CLIENT
        && protocol != PROTOCOL_TLS_SERVER
        && protocol != PROTOCOL_DTLS_CLIENT
        && protocol != PROTOCOL_DTLS_SERVER
    {
        raise::raise(MpRaise::ValueError("protocol"));
    }
    let o = malloc::new_obj::<ObjSslContext>().expect("SSLContext");
    unsafe {
        (*o).base.type_ = type_in;
        (*o).protocol = protocol;
        (*o).verify_mode = if is_server(protocol) {
            CERT_NONE
        } else {
            CERT_REQUIRED
        };
        (*o).verify_callback = obj::CONST_NONE;
        (*o).cert_pem = None;
        (*o).key_pem = None;
        (*o).ca_pem = Vec::new();
        (*o).ciphers = None;
        obj::from_ptr(o as *const ObjSslContext as *const ())
    }
}

fn ssl_context_attr(self_in: Obj, attr: Qstr, dest: &mut [Obj; 2]) {
    let self_ = unsafe { &mut *ctx_ptr(self_in) };
    if dest[0] == obj::OBJ_NULL {
        if attr == qstr::from_str("verify_mode") {
            dest[0] = int_const(self_.verify_mode);
        } else if attr == qstr::from_str("verify_callback") {
            dest[0] = self_.verify_callback;
        } else {
            dest[1] = obj::OBJ_SENTINEL;
        }
    } else if dest[1] != obj::OBJ_NULL {
        if attr == qstr::from_str("verify_mode") {
            self_.verify_mode = obj::get_int(dest[1]) as i32;
            dest[0] = obj::OBJ_NULL;
        } else if attr == qstr::from_str("verify_callback") {
            self_.verify_callback = dest[1];
            dest[0] = obj::OBJ_NULL;
        }
    }
}

fn ssl_context_del(self_in: Obj) -> Obj {
    let self_ = unsafe { &mut *ctx_ptr(self_in) };
    self_.cert_pem = None;
    self_.key_pem = None;
    self_.ca_pem.clear();
    self_.ciphers = None;
    self_.verify_callback = obj::CONST_NONE;
    obj::CONST_NONE
}

fn ssl_context_load_cert_chain_call(n: usize, args: &[Obj]) -> Obj {
    ssl_context_load_cert_chain(args[0], args[1], args[2])
}

fn ssl_context_load_cert_chain(self_in: Obj, cert: Obj, key: Obj) -> Obj {
    let self_ = unsafe { &mut *ctx_ptr(self_in) };
    let cert_pem = read_pem_bytes(cert);
    let key_pem = read_pem_bytes(key);
    let _ = parse_certs(&cert_pem);
    let _ = parse_private_key(&key_pem);
    self_.cert_pem = Some(cert_pem);
    self_.key_pem = Some(key_pem);
    obj::CONST_NONE
}

fn ssl_context_load_verify_locations(self_in: Obj, cadata: Obj) -> Obj {
    let self_ = unsafe { &mut *ctx_ptr(self_in) };
    let pem = read_pem_bytes(cadata);
    let _ = parse_certs(&pem);
    self_.ca_pem.push(pem);
    obj::CONST_NONE
}

fn ssl_context_get_ciphers(self_in: Obj) -> Obj {
    init_crypto();
    let list = objlist::new_list(0, None);
    for cs in ring_cipher_suites() {
        let name = cipher_suite_name(cs.suite());
        objlist::list_append(list, objstr::new_str(name.as_bytes()));
    }
    list
}

fn ssl_context_set_ciphers(self_in: Obj, ciphersuite: Obj) -> Obj {
    let self_ = unsafe { &mut *ctx_ptr(self_in) };
    let (len, items) = obj::get_array(ciphersuite);
    if len == 0 {
        raise_ssl_bad_config();
    }
    let mut ciphers = Vec::with_capacity(len);
    for item in items {
        let name = objstr::str_get_str(item);
        match find_cipher_by_name(&name) {
            Some(id) => ciphers.push(id),
            None => raise_ssl_bad_config(),
        }
    }
    self_.ciphers = Some(ciphers);
    obj::CONST_NONE
}

fn attr_type_name(oid: &x509_parser::oid_registry::Oid<'_>) -> String {
    oid2sn(oid, oid_registry())
        .map(|s| s.to_string())
        .unwrap_or_else(|_| oid.to_string())
}

fn attr_value_str(attr: &x509_parser::x509::AttributeTypeAndValue<'_>) -> String {
    if let Ok(s) = attr.as_str() {
        return s.to_string();
    }
    String::from_utf8_lossy(attr.attr_value().data).into_owned()
}

fn x509_name_to_tuple(name: &X509Name<'_>) -> Obj {
    let mut rdns = Vec::new();
    for rdn in name.iter() {
        let mut attrs = Vec::new();
        for attr in rdn.iter() {
            let pair = objtuple::new_tuple(
                2,
                Some(&[
                    objstr::new_str(attr_type_name(attr.attr_type()).as_bytes()),
                    objstr::new_str(attr_value_str(attr).as_bytes()),
                ]),
            );
            attrs.push(pair);
        }
        rdns.push(objtuple::new_tuple(attrs.len(), Some(&attrs)));
    }
    objtuple::new_tuple(rdns.len(), Some(&rdns))
}

fn serial_number_hex(serial: &[u8]) -> String {
    let mut hex: String = serial.iter().map(|b| format!("{:02X}", b)).collect();
    while hex.starts_with('0') && hex.len() > 1 {
        hex.remove(0);
    }
    hex
}

fn format_cert_time(timestamp: i64) -> String {
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    let mut ts = timestamp;
    let mut sec = (ts % 60) as i32;
    ts /= 60;
    let mut min = (ts % 60) as i32;
    ts /= 60;
    let mut hour = (ts % 24) as i32;
    let mut days = ts / 24;

    let mut year = 1970i32;
    loop {
        let year_days = if is_leap_year(year) { 366 } else { 365 };
        if days < year_days {
            break;
        }
        days -= year_days;
        year += 1;
    }

    let mut month = 0usize;
    while month < 12 {
        let month_days = days_in_month(year, (month + 1) as u32);
        if days < month_days {
            break;
        }
        days -= month_days;
        month += 1;
    }

    format!(
        "{} {:02} {:02}:{:02}:{:02} {} GMT",
        MONTHS[month],
        days + 1,
        hour,
        min,
        sec,
        year
    )
}

fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

fn days_in_month(year: i32, month: u32) -> i64 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if is_leap_year(year) {
                29
            } else {
                28
            }
        }
        _ => 0,
    }
}

fn format_ip_address(ip: &[u8]) -> Option<String> {
    if ip.len() == 4 {
        return Some(format!("{}.{}.{}.{}", ip[0], ip[1], ip[2], ip[3]));
    }
    if ip.len() == 16 {
        let mut parts = Vec::with_capacity(8);
        for chunk in ip.chunks(2) {
            let val = u16::from_be_bytes([chunk[0], chunk[1]]);
            parts.push(format!("{:x}", val));
        }
        return Some(parts.join(":"));
    }
    None
}

fn subject_alt_name_to_tuple(cert: &X509Certificate<'_>) -> Option<Obj> {
    use x509_parser::extensions::GeneralName;
    let ext = cert.subject_alternative_name().ok()??;
    let mut items = Vec::new();
    for name in &ext.value.general_names {
        let pair = match name {
            GeneralName::DNSName(d) => Some(("DNS", d.to_string())),
            GeneralName::URI(u) => Some(("URI", u.to_string())),
            GeneralName::RFC822Name(e) => Some(("email", e.to_string())),
            GeneralName::IPAddress(ip) => format_ip_address(ip).map(|s| ("IP Address", s)),
            _ => None,
        };
        if let Some((kind, value)) = pair {
            items.push(objtuple::new_tuple(
                2,
                Some(&[
                    objstr::new_str(kind.as_bytes()),
                    objstr::new_str(value.as_bytes()),
                ]),
            ));
        }
    }
    if items.is_empty() {
        return None;
    }
    Some(objtuple::new_tuple(items.len(), Some(&items)))
}

fn peer_cert_to_dict(der: &[u8]) -> Obj {
    let Ok((_, cert)) = X509Certificate::from_der(der) else {
        return objdict::new_dict(0);
    };
    let dict = objdict::new_dict(7);
    objdict::dict_store(
        dict,
        obj::new_qstr(qstr::from_str("subject")),
        x509_name_to_tuple(cert.subject()),
    );
    objdict::dict_store(
        dict,
        obj::new_qstr(qstr::from_str("issuer")),
        x509_name_to_tuple(cert.issuer()),
    );
    objdict::dict_store(
        dict,
        obj::new_qstr(qstr::from_str("version")),
        obj::new_small_int((cert.version().0 + 1) as obj::Int),
    );
    objdict::dict_store(
        dict,
        obj::new_qstr(qstr::from_str("serialNumber")),
        objstr::new_str(serial_number_hex(cert.raw_serial()).as_bytes()),
    );
    objdict::dict_store(
        dict,
        obj::new_qstr(qstr::from_str("notBefore")),
        objstr::new_str(format_cert_time(cert.validity().not_before.timestamp()).as_bytes()),
    );
    objdict::dict_store(
        dict,
        obj::new_qstr(qstr::from_str("notAfter")),
        objstr::new_str(format_cert_time(cert.validity().not_after.timestamp()).as_bytes()),
    );
    if let Some(san) = subject_alt_name_to_tuple(&cert) {
        objdict::dict_store(dict, obj::new_qstr(qstr::from_str("subjectAltName")), san);
    }
    dict
}

fn ssl_socket_getpeercert(self_in: Obj, binary_form: Obj) -> Obj {
    let self_ = unsafe { &*ssl_sock_ptr(self_in) };
    if self_.conn.is_null() {
        return obj::CONST_NONE;
    }
    let conn = unsafe { &*self_.conn };
    if !conn.is_handshake_complete() {
        raise::raise(MpRaise::ValueError("handshake not done"));
    }
    let der = match conn.common().peer_certificates() {
        Some(certs) if !certs.is_empty() => certs[0].as_ref(),
        _ => return obj::CONST_NONE,
    };
    if obj::is_true(binary_form) {
        return objstr::new_bytes(der);
    }
    let ctx = unsafe { &*ctx_ptr(self_.ctx) };
    if ctx.verify_mode == CERT_NONE {
        return objdict::new_dict(0);
    }
    peer_cert_to_dict(der)
}

fn ssl_socket_cipher(self_in: Obj) -> Obj {
    let self_ = unsafe { &*ssl_sock_ptr(self_in) };
    if self_.conn.is_null() {
        raise::raise(MpRaise::ValueError("closed"));
    }
    let conn = unsafe { &*self_.conn };
    let common = conn.common();
    let suite = common
        .negotiated_cipher_suite()
        .map(|s| cipher_suite_name(s.suite()))
        .unwrap_or_else(|| "unknown".to_string());
    let version = common
        .protocol_version()
        .map(protocol_version_str)
        .unwrap_or("unknown");
    objtuple::new_tuple(
        2,
        Some(&[
            objstr::new_str(suite.as_bytes()),
            objstr::new_str(version.as_bytes()),
        ]),
    )
}

fn ssl_socket_make_new(
    ctx_in: Obj,
    sock: Obj,
    server_side: bool,
    do_handshake_on_connect: bool,
    server_hostname: Obj,
) -> Obj {
    stream::get_stream_raise(sock, STREAM_OP_READ | STREAM_OP_WRITE | STREAM_OP_IOCTL);
    let ctx = unsafe { &*ctx_ptr(ctx_in) };
    if is_dtls(ctx.protocol) {
        raise::raise(MpRaise::ValueError("DTLS not supported"));
    }
    if !server_side && ctx.verify_mode == CERT_REQUIRED && server_hostname == obj::CONST_NONE {
        raise::raise(MpRaise::ValueError(
            "CERT_REQUIRED requires server_hostname",
        ));
    }

    let o = malloc::new_obj::<ObjSslSocket>().expect("SSLSocket");
    unsafe {
        (*o).base.type_ = type_ssl_socket();
        (*o).ctx = ctx_in;
        (*o).sock = sock;
        (*o).conn = core::ptr::null_mut();
        (*o).blocking = true;
        (*o).poll_wr = false;
        (*o).last_error = 0;
    }

    let mut adapter = StreamAdapter {
        sock,
        blocking: true,
    };

    let conn = if server_side {
        let config = build_server_config(ctx);
        TlsConn::Server(ServerConnection::new(config).unwrap_or_else(|e| raise_tls_error(e)))
    } else {
        let config = build_client_config(ctx);
        let name_str = if server_hostname != obj::CONST_NONE {
            objstr::str_get_str(server_hostname)
        } else {
            "localhost".to_string()
        };
        let server_name = ServerName::try_from(name_str.to_string())
            .unwrap_or_else(|_| raise::raise(MpRaise::ValueError("server_hostname")));
        TlsConn::Client(
            ClientConnection::new(config, server_name).unwrap_or_else(|e| raise_tls_error(e)),
        )
    };

    let conn_box = Box::new(conn);
    unsafe {
        (*o).conn = Box::into_raw(conn_box);
    }

    if do_handshake_on_connect {
        let ssl =
            unsafe { &mut *ssl_sock_ptr(obj::from_ptr(o as *const ObjSslSocket as *const ())) };
        let conn = unsafe { &mut *ssl.conn };
        do_handshake(conn, &mut adapter);
    }

    obj::from_ptr(o as *const ObjSslSocket as *const ())
}

fn ssl_context_wrap_socket(n: usize, pos: &[Obj], kw: &Map) -> Obj {
    let allowed = [
        Arg {
            qst: qstr::from_str("server_side"),
            flags: ArgFlag::KwOnly as u16 | ArgFlag::Bool as u16,
            defval: ArgVal::Bool(false),
        },
        Arg {
            qst: qstr::from_str("do_handshake_on_connect"),
            flags: ArgFlag::KwOnly as u16 | ArgFlag::Bool as u16,
            defval: ArgVal::Bool(true),
        },
        Arg {
            qst: qstr::from_str("server_hostname"),
            flags: ArgFlag::KwOnly as u16 | ArgFlag::Obj as u16,
            defval: ArgVal::Obj(obj::CONST_NONE),
        },
    ];
    let mut vals = [ArgVal::default(); 3];
    let mut kw_copy = kw.clone();
    argcheck::parse_all(
        n - 2,
        &pos[2..],
        &mut kw_copy,
        allowed.len(),
        &allowed,
        &mut vals,
    );
    let server_side = match vals[0] {
        ArgVal::Bool(b) => b,
        _ => false,
    };
    let do_handshake = match vals[1] {
        ArgVal::Bool(b) => b,
        _ => true,
    };
    let server_hostname = match vals[2] {
        ArgVal::Obj(o) => o,
        _ => obj::CONST_NONE,
    };
    ssl_socket_make_new(pos[0], pos[1], server_side, do_handshake, server_hostname)
}

fn ssl_socket_read(self_in: Obj, buf: *mut u8, size: usize, errcode: *mut i32) -> usize {
    let self_ = unsafe { &mut *ssl_sock_ptr(self_in) };
    unsafe {
        *errcode = 0;
    }
    if self_.last_error != 0 {
        unsafe {
            *errcode = self_.last_error;
        }
        return STREAM_ERROR;
    }
    if self_.conn.is_null() {
        unsafe {
            *errcode = 9;
        }
        return STREAM_ERROR;
    }

    self_.poll_wr = false;
    let conn = unsafe { &mut *self_.conn };
    let mut adapter = StreamAdapter {
        sock: self_.sock,
        blocking: self_.blocking,
    };

    if !conn.is_handshake_complete() {
        match conn.complete_io(&mut adapter) {
            Ok(_) => {}
            Err(e) => {
                return map_io_stream_error(e, self_, errcode);
            }
        }
    }

    let mut reader = conn.reader();
    match reader.read(unsafe { std::slice::from_raw_parts_mut(buf, size) }) {
        Ok(0) => 0,
        Ok(n) => n,
        Err(e) => map_io_stream_error(e, self_, errcode),
    }
}

fn ssl_socket_write(self_in: Obj, buf: *const u8, size: usize, errcode: *mut i32) -> usize {
    let self_ = unsafe { &mut *ssl_sock_ptr(self_in) };
    unsafe {
        *errcode = 0;
    }
    if self_.last_error != 0 {
        unsafe {
            *errcode = self_.last_error;
        }
        return STREAM_ERROR;
    }
    if self_.conn.is_null() {
        unsafe {
            *errcode = 9;
        }
        return STREAM_ERROR;
    }

    self_.poll_wr = false;
    let conn = unsafe { &mut *self_.conn };
    let mut adapter = StreamAdapter {
        sock: self_.sock,
        blocking: self_.blocking,
    };

    if !conn.is_handshake_complete() {
        match conn.complete_io(&mut adapter) {
            Ok(_) => {}
            Err(e) => {
                return map_io_stream_error(e, self_, errcode);
            }
        }
    }

    let payload = unsafe { std::slice::from_raw_parts(buf, size) };
    let mut writer = conn.writer();
    match writer.write(payload) {
        Ok(n) => n,
        Err(e) => map_io_stream_error(e, self_, errcode),
    }
}

fn map_io_stream_error(e: io::Error, self_: &mut ObjSslSocket, errcode: *mut i32) -> usize {
    if e.kind() == io::ErrorKind::WouldBlock {
        self_.poll_wr = true;
        unsafe {
            *errcode = 11;
        }
        return STREAM_ERROR;
    }
    if let Some(code) = e.raw_os_error() {
        unsafe {
            *errcode = code;
        }
        return STREAM_ERROR;
    }
    self_.last_error = -1;
    unsafe {
        *errcode = -1;
    }
    STREAM_ERROR
}

fn ssl_socket_ioctl(self_in: Obj, request: u32, arg: usize, errcode: *mut i32) -> usize {
    let self_ = unsafe { &mut *ssl_sock_ptr(self_in) };
    unsafe {
        *errcode = 0;
    }
    let mut ret = 0usize;
    let mut saved_arg = 0usize;
    let sock = self_.sock;

    if request == STREAM_CLOSE {
        drop_conn(self_);
        if sock == obj::OBJ_NULL {
            return 0;
        }
        self_.sock = obj::OBJ_NULL;
    } else if request == STREAM_POLL {
        if sock == obj::OBJ_NULL || self_.last_error != 0 {
            return STREAM_POLL_NVAL as usize;
        }
        let mut poll_arg = arg;
        if self_.poll_wr && (arg as u32 & (STREAM_POLL_RD | STREAM_POLL_WR)) != 0 {
            saved_arg = arg & (STREAM_POLL_RD | STREAM_POLL_WR) as usize;
            poll_arg =
                (arg & !(STREAM_POLL_RD | STREAM_POLL_WR) as usize) | STREAM_POLL_WR as usize;
        }
        let stream_p = stream::get_stream(sock);
        let ioctl = stream_p.ioctl.expect("socket ioctl");
        ret = ioctl(sock, request, poll_arg, errcode);
        if self_.poll_wr && (ret & STREAM_POLL_WR as usize) != 0 {
            ret |= saved_arg;
        }
        return ret;
    } else {
        unsafe {
            *errcode = 22;
        }
        return STREAM_ERROR;
    }

    if sock == obj::OBJ_NULL {
        return 0;
    }
    let stream_p = stream::get_stream(sock);
    stream_p.ioctl.expect("socket ioctl")(sock, request, arg, errcode)
}

fn ssl_socket_setblocking(self_in: Obj, flag: Obj) -> Obj {
    let self_ = unsafe { &mut *ssl_sock_ptr(self_in) };
    let mut dest = [obj::OBJ_NULL; 3];
    runtime::load_method(
        self_.sock,
        qstr::from_str("setblocking"),
        &mut dest[..2].try_into().unwrap(),
    );
    dest[2] = flag;
    let res = runtime::call_method_n_kw(1, 0, &dest);
    self_.blocking = obj::is_true(flag);
    res
}

static SSL_SOCKET_STREAM: StreamP = StreamP {
    read: Some(ssl_socket_read as StreamIoFn),
    write: Some(ssl_socket_write),
    ioctl: Some(ssl_socket_ioctl as StreamIoctlFn),
    is_text: false,
};

static mut SSL_CONTEXT_SLOTS: [*const (); 4] = [core::ptr::null(); 4];
static mut TYPE_SSL_CONTEXT: ObjType = ObjType {
    base: ObjBase {
        type_: core::ptr::null(),
    },
    flags: obj::TYPE_FLAG_NONE,
    name: 0,
    slot_index_make_new: 1,
    slot_index_print: 0,
    slot_index_call: 0,
    slot_index_unary_op: 0,
    slot_index_binary_op: 0,
    slot_index_attr: 2,
    slot_index_subscr: 0,
    slot_index_iter: 0,
    slot_index_buffer: 0,
    slot_index_protocol: 0,
    slot_index_parent: 0,
    slot_index_locals_dict: 3,
    slots: unsafe { SSL_CONTEXT_SLOTS.as_ptr() },
};

static mut SSL_SOCKET_SLOTS: [*const (); 3] = [core::ptr::null(); 3];
static mut TYPE_SSL_SOCKET: ObjType = ObjType {
    base: ObjBase {
        type_: core::ptr::null(),
    },
    flags: TYPE_FLAG_ITER_IS_STREAM,
    name: 0,
    slot_index_make_new: 0,
    slot_index_print: 0,
    slot_index_call: 0,
    slot_index_unary_op: 0,
    slot_index_binary_op: 0,
    slot_index_attr: 0,
    slot_index_subscr: 0,
    slot_index_iter: 0,
    slot_index_buffer: 0,
    slot_index_protocol: 1,
    slot_index_parent: 0,
    slot_index_locals_dict: 2,
    slots: unsafe { SSL_SOCKET_SLOTS.as_ptr() },
};

static SSL_CONTEXT_INIT: std::sync::OnceLock<()> = std::sync::OnceLock::new();
static SSL_SOCKET_INIT: std::sync::OnceLock<()> = std::sync::OnceLock::new();

fn ssl_context_locals() -> *const () {
    static INIT: std::sync::OnceLock<()> = std::sync::OnceLock::new();
    static mut DICT: *const () = core::ptr::null();
    INIT.get_or_init(|| {
        let mut table = vec![
            MapElem {
                key: obj::new_qstr(qstr::from_str("get_ciphers")),
                value: mk1(ssl_context_get_ciphers),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("set_ciphers")),
                value: mk2(ssl_context_set_ciphers),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("load_cert_chain")),
                value: mkv(3, 3, ssl_context_load_cert_chain_call),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("load_verify_locations")),
                value: mk2(|s, c| ssl_context_load_verify_locations(s, c)),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("wrap_socket")),
                value: mk_kw(2, ssl_context_wrap_socket),
            },
        ];
        if mpconfig::PY_SSL_FINALISER {
            table.insert(
                0,
                MapElem {
                    key: obj::new_qstr(qstr::from_str("__del__")),
                    value: mk1(ssl_context_del),
                },
            );
        }
        let ptr = obj::malloc_helper(
            core::mem::size_of::<objdict::ObjDict>(),
            objdict::type_dict(),
        ) as *mut objdict::ObjDict;
        unsafe {
            map::init_fixed_table(&mut (*ptr).map, table);
            DICT = obj::from_ptr(ptr as *const objdict::ObjDict as *const ()).0 as *const ();
        }
    });
    unsafe { DICT }
}

fn ssl_socket_locals() -> *const () {
    static INIT: std::sync::OnceLock<()> = std::sync::OnceLock::new();
    static mut DICT: *const () = core::ptr::null();
    INIT.get_or_init(|| {
        let mut table = vec![
            MapElem {
                key: obj::new_qstr(qstr::from_str("read")),
                value: stream::stream_read_obj(),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("readinto")),
                value: stream::stream_readinto_obj(),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("readline")),
                value: stream::stream_unbuffered_readline_obj(),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("write")),
                value: stream::stream_write_obj(),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("setblocking")),
                value: mk2(ssl_socket_setblocking),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("getpeercert")),
                value: mk2(ssl_socket_getpeercert),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("cipher")),
                value: mk1(ssl_socket_cipher),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("close")),
                value: stream::stream_close_obj(),
            },
        ];
        if mpconfig::PY_SSL_FINALISER {
            table.push(MapElem {
                key: obj::new_qstr(qstr::from_str("__del__")),
                value: stream::stream_close_obj(),
            });
        }
        let ptr = obj::malloc_helper(
            core::mem::size_of::<objdict::ObjDict>(),
            objdict::type_dict(),
        ) as *mut objdict::ObjDict;
        unsafe {
            map::init_fixed_table(&mut (*ptr).map, table);
            DICT = obj::from_ptr(ptr as *const objdict::ObjDict as *const ()).0 as *const ();
        }
    });
    unsafe { DICT }
}

fn type_ssl_context() -> &'static ObjType {
    SSL_CONTEXT_INIT.get_or_init(|| {
        let dict = ssl_context_locals();
        unsafe {
            SSL_CONTEXT_SLOTS[0] = core::ptr::null();
            SSL_CONTEXT_SLOTS[1] = ssl_context_make_new as *const ();
            SSL_CONTEXT_SLOTS[2] = ssl_context_attr as *const ();
            SSL_CONTEXT_SLOTS[3] = dict;
            TYPE_SSL_CONTEXT.name = qstr::from_str("SSLContext");
        }
    });
    unsafe { &TYPE_SSL_CONTEXT }
}

fn type_ssl_socket() -> &'static ObjType {
    SSL_SOCKET_INIT.get_or_init(|| {
        let dict = ssl_socket_locals();
        unsafe {
            SSL_SOCKET_SLOTS[1] = &SSL_SOCKET_STREAM as *const StreamP as *const ();
            SSL_SOCKET_SLOTS[2] = dict;
            TYPE_SSL_SOCKET.name = qstr::from_str("SSLSocket");
        }
    });
    unsafe { &TYPE_SSL_SOCKET }
}

static MODULE_INIT: std::sync::OnceLock<Obj> = std::sync::OnceLock::new();

/// Register built-in `tls` module (`MP_REGISTER_MODULE`).
pub fn init_module() -> Obj {
    if !mpconfig::PY_SSL || !mpconfig::SSL_MBEDTLS {
        return obj::OBJ_NULL;
    }
    *MODULE_INIT.get_or_init(|| {
        type_ssl_context();
        type_ssl_socket();
        let table = [
            MapElem {
                key: obj::new_qstr(qstr::from_str("__name__")),
                value: obj::new_qstr(qstr::from_str("tls")),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("SSLContext")),
                value: obj::from_ptr(type_ssl_context() as *const ObjType as *const ()),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("PROTOCOL_TLS_CLIENT")),
                value: int_const(PROTOCOL_TLS_CLIENT),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("PROTOCOL_TLS_SERVER")),
                value: int_const(PROTOCOL_TLS_SERVER),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("PROTOCOL_DTLS_CLIENT")),
                value: int_const(PROTOCOL_DTLS_CLIENT),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("PROTOCOL_DTLS_SERVER")),
                value: int_const(PROTOCOL_DTLS_SERVER),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("CERT_NONE")),
                value: int_const(CERT_NONE),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("CERT_OPTIONAL")),
                value: int_const(CERT_OPTIONAL),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("CERT_REQUIRED")),
                value: int_const(CERT_REQUIRED),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("MBEDTLS_VERSION")),
                value: objstr::new_str(b"rustls 0.23 (host)"),
            },
        ];
        let ctx = malloc::new_obj::<ModuleContext>().expect("tls module");
        let dict = objdict::new_dict(table.len());
        unsafe {
            map::init_fixed_table(&mut (*objdict::dict_ptr(dict)).map, table.to_vec());
            (*ctx).module.base.type_ = objmodule::type_module();
            (*ctx).module.globals = objdict::dict_ptr(dict);
            (*ctx).constants = Default::default();
        }
        let module = obj::from_ptr(ctx as *const ModuleContext as *const ());
        objmodule::register_builtin_module(qstr::from_str("tls"), module);
        module
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    const TEST_CERT_DER: &[u8] = include_bytes!("testdata/tls_test.der");

    #[test]
    fn serial_number_hex_strips_leading_zeros() {
        assert_eq!(serial_number_hex(&[0x00, 0x95, 0xf0]), "95F0");
        assert_eq!(serial_number_hex(&[0x05]), "5");
    }

    #[test]
    fn format_cert_time_utc_gmt() {
        // 2020-01-01 00:00:00 UTC
        assert_eq!(format_cert_time(1577836800), "Jan 01 00:00:00 2020 GMT");
    }

    #[test]
    fn peer_cert_dict_parses_common_fields() {
        let (_, cert) = X509Certificate::from_der(TEST_CERT_DER).expect("parse cert");
        assert_eq!(cert.version().0 + 1, 3);
        assert_eq!(
            cert.subject()
                .iter_common_name()
                .next()
                .and_then(|cn| cn.as_str().ok()),
            Some("micropython.local")
        );
        assert!(cert.subject_alternative_name().unwrap().is_some());
        assert_eq!(
            serial_number_hex(cert.raw_serial()),
            "5362489E52CC1C47D3F28084A686A411AE8AF78E"
        );
    }
}
