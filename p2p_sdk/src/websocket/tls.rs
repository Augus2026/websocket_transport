//! TLS 配置和证书管理
//!
//! 提供 TLS 证书加载和配置功能

use crate::error::{P2PError, Result};
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::sync::Arc;
use tokio_rustls::rustls::{
    self, ClientConfig, RootCertStore, ServerConfig,
    pki_types::{CertificateDer, PrivateKeyDer},
};
use tokio_rustls::{TlsAcceptor, TlsConnector};

/// 加载证书链
pub fn load_certs(path: &Path) -> Result<Vec<CertificateDer<'static>>> {
    let file = File::open(path).map_err(|e| {
        P2PError::ConfigError(format!("无法打开证书文件 {}: {}", path.display(), e))
    })?;
    let mut reader = BufReader::new(file);

    rustls_pemfile::certs(&mut reader)
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| P2PError::ConfigError(format!("解析证书失败: {}", e)))
}

/// 加载私钥
pub fn load_private_key(path: &Path) -> Result<PrivateKeyDer<'static>> {
    let file = File::open(path).map_err(|e| {
        P2PError::ConfigError(format!("无法打开私钥文件 {}: {}", path.display(), e))
    })?;
    let mut reader = BufReader::new(file);

    // 尝试加载 PKCS8 格式
    if let Some(key) = rustls_pemfile::private_key(&mut reader)
        .map_err(|e| P2PError::ConfigError(format!("解析私钥失败: {}", e)))?
    {
        return Ok(key);
    }

    Err(P2PError::ConfigError("未找到有效的私钥".to_string()))
}

/// 创建 TLS 服务端接受器
pub fn create_server_tls_acceptor(cert_path: &Path, key_path: &Path) -> Result<TlsAcceptor> {
    let certs = load_certs(cert_path)?;
    let key = load_private_key(key_path)?;

    let config = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .map_err(|e| P2PError::ConfigError(format!("创建 TLS 配置失败: {}", e)))?;

    Ok(TlsAcceptor::from(Arc::new(config)))
}

/// 创建 TLS 客户端连接器（带自定义 CA）
pub fn create_client_tls_connector_with_ca(ca_path: &Path) -> Result<TlsConnector> {
    let ca_file = File::open(ca_path).map_err(|e| {
        P2PError::ConfigError(format!("无法打开 CA 证书文件 {}: {}", ca_path.display(), e))
    })?;
    let mut reader = BufReader::new(ca_file);

    let mut root_store = RootCertStore::empty();
    let certs = rustls_pemfile::certs(&mut reader)
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| P2PError::ConfigError(format!("解析 CA 证书失败: {}", e)))?;

    for cert in certs {
        root_store
            .add(cert)
            .map_err(|e| P2PError::ConfigError(format!("添加 CA 证书失败: {}", e)))?;
    }

    let config = ClientConfig::builder()
        .with_root_certificates(root_store)
        .with_no_client_auth();

    Ok(TlsConnector::from(Arc::new(config)))
}

/// 创建 TLS 客户端连接器（使用系统 CA，可选跳过验证）
pub fn create_client_tls_connector(insecure: bool) -> Result<TlsConnector> {
    if insecure {
        // 跳过验证：创建一个不安全的连接器
        // 注意：这仅用于开发测试，生产环境不应使用
        eprintln!("[警告] 已启用 insecure 模式，将跳过证书验证");

        let config = ClientConfig::builder()
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(InsecureVerifier::new()))
            .with_no_client_auth();

        Ok(TlsConnector::from(Arc::new(config)))
    } else {
        // 使用 webpki 根证书
        let root_store = RootCertStore::empty();

        // 注意：生产环境应加载系统根证书或使用 webpki_roots
        let config = ClientConfig::builder()
            .with_root_certificates(root_store)
            .with_no_client_auth();

        Ok(TlsConnector::from(Arc::new(config)))
    }
}

/// 不安全的证书验证器（仅用于开发）
#[derive(Debug)]
struct InsecureVerifier {
    // 使用 PhantomData 来满足 Send + Sync
    _marker: std::marker::PhantomData<()>,
}

impl InsecureVerifier {
    fn new() -> Self {
        Self {
            _marker: std::marker::PhantomData,
        }
    }
}

impl rustls::client::danger::ServerCertVerifier for InsecureVerifier {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &rustls::pki_types::ServerName<'_>,
        _ocsp_response: &[u8],
        _now: rustls::pki_types::UnixTime,
    ) -> std::result::Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        // 跳过所有验证，返回成功
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> std::result::Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> std::result::Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        vec![
            rustls::SignatureScheme::RSA_PKCS1_SHA256,
            rustls::SignatureScheme::RSA_PKCS1_SHA384,
            rustls::SignatureScheme::RSA_PKCS1_SHA512,
            rustls::SignatureScheme::ECDSA_NISTP256_SHA256,
            rustls::SignatureScheme::ECDSA_NISTP384_SHA384,
            rustls::SignatureScheme::ECDSA_NISTP521_SHA512,
            rustls::SignatureScheme::RSA_PSS_SHA256,
            rustls::SignatureScheme::RSA_PSS_SHA384,
            rustls::SignatureScheme::RSA_PSS_SHA512,
            rustls::SignatureScheme::ED25519,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_certs_file_not_found() {
        let result = load_certs(Path::new("/nonexistent/cert.pem"));
        assert!(result.is_err());
    }

    #[test]
    fn test_load_private_key_file_not_found() {
        let result = load_private_key(Path::new("/nonexistent/key.pem"));
        assert!(result.is_err());
    }
}
