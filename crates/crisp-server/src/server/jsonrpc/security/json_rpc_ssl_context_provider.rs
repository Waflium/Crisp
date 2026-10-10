use std::sync::Arc;

use p12_keystore::{KeyStore, Pkcs12ImportPolicy};
use tokio_rustls::TlsAcceptor;
use tokio_rustls::rustls::ServerConfig;
use tokio_rustls::rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};

const PASSWORD_ENV_VARIABLE_KEY: &str = "MINECRAFT_MANAGEMENT_TLS_KEYSTORE_PASSWORD";

pub fn create_from(keystore_path: &str, keystore_password_from_server_properties: &str) -> Result<TlsAcceptor, String> {
    if keystore_path.is_empty() {
        return Err("java.lang.IllegalArgumentException: TLS is enabled but keystore is not configured".into());
    }
    if !std::path::Path::new(keystore_path).is_file() {
        return Err(format!("java.lang.IllegalArgumentException: Supplied keystore is not a file or does not exist: '{keystore_path}'"));
    }
    let password = std::env::var(PASSWORD_ENV_VARIABLE_KEY).unwrap_or_else(|_| keystore_password_from_server_properties.to_string());
    load_keystore_from_path(keystore_path, &password)
}

fn load_keystore_from_path(path: &str, password: &str) -> Result<TlsAcceptor, String> {
    let data = std::fs::read(path).map_err(|e| format!("java.io.IOException: {e}"))?;
    let keystore = KeyStore::from_pkcs12(&data, password, Pkcs12ImportPolicy::Strict).map_err(|e| match e {
        p12_keystore::error::Error::MacError(_) => "java.io.IOException: keystore password was incorrect".to_string(),
        other => format!("java.io.IOException: {other}"),
    })?;
    let Some((_, chain)) = keystore.private_key_chain() else { return Err("java.lang.IllegalArgumentException: No private key in keystore".into()) };
    let certs: Vec<CertificateDer<'static>> = chain.certs().iter().map(|c| CertificateDer::from(c.as_der().to_vec())).collect();
    let key = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(chain.key().as_der().to_vec()));
    let provider = Arc::new(tokio_rustls::rustls::crypto::ring::default_provider());
    let config = ServerConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .map_err(|e| format!("javax.net.ssl.SSLException: {e}"))?
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .map_err(|e| format!("javax.net.ssl.SSLException: {e}"))?;
    Ok(TlsAcceptor::from(Arc::new(config)))
}

pub fn print_instructions() {
    info!("To use TLS for the management server, please follow these steps:");
    info!("1. Set the server property 'management-server-tls-enabled' to 'true' to enable TLS");
    info!("2. Create a keystore file of type PKCS12 containing your server certificate and private key");
    info!("3. Set the server property 'management-server-tls-keystore' to the path of your keystore file");
    info!(
        "4. Set the keystore password via the environment variable 'MINECRAFT_MANAGEMENT_TLS_KEYSTORE_PASSWORD', or system property 'management.tls.keystore.password', or server property 'management-server-tls-keystore-password'"
    );
    info!("5. Restart the server to apply the changes.");
}
