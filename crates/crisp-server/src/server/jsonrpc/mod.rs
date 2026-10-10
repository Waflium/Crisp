pub mod api;
pub mod codec;
pub mod connection;
pub mod gson;
pub mod http_server_codec;
pub mod incoming_rpc_methods;
pub mod json_rpc_utils;
pub mod management_server;
pub mod methods;
pub mod outgoing_rpc_methods;
pub mod security;

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use self::management_server::ManagementServer;
use self::security::authentication_handler::AuthenticationHandler;
use self::security::json_rpc_ssl_context_provider;
use super::dedicated::dedicated_server_properties::DedicatedServerProperties;

fn host_and_port(host: &str, port: i32) -> Result<String, String> {
    if !(0..=65535).contains(&port) {
        return Err(format!("java.lang.IllegalArgumentException: Port out of range: {port}"));
    }
    let unbracketed = host.strip_prefix('[').and_then(|h| h.strip_suffix(']')).unwrap_or(host);
    if !unbracketed.contains(':') && host.contains(':') || host.starts_with('[') && !host.ends_with(']') {
        return Err(format!("java.lang.IllegalArgumentException: Host has a port: {host}"));
    }
    Ok(if unbracketed.contains(':') { format!("[{unbracketed}]:{port}") } else { format!("{unbracketed}:{port}") })
}

pub fn create(properties: &DedicatedServerProperties) -> Result<Option<(Arc<ManagementServer>, tokio::runtime::Runtime)>, String> {
    if !properties.management_server_enabled {
        return Ok(None);
    }
    let secret = &properties.management_server_secret;
    if !security::is_valid(secret) {
        return Err("java.lang.IllegalStateException: Invalid management server secret, must be 40 alphanumeric characters".into());
    }
    let host = properties.management_server_host.clone();
    let port = properties.management_server_port;
    let display = host_and_port(&host, port)?;
    let authentication_handler = AuthenticationHandler::new(secret.clone(), &properties.management_server_allowed_origins);
    info!("Starting json RPC server on {display}");
    let next_thread = AtomicUsize::new(0);
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .thread_name_fn(move || format!("Management server IO #{}", next_thread.fetch_add(1, Ordering::Relaxed)))
        .build()
        .map_err(|e| format!("java.lang.IllegalStateException: {e}"))?;
    let tls = if properties.management_server_tls_enabled {
        match json_rpc_ssl_context_provider::create_from(&properties.management_server_tls_keystore, &properties.management_server_tls_keystore_password) {
            Ok(acceptor) => Some(acceptor),
            Err(e) => {
                json_rpc_ssl_context_provider::print_instructions();
                runtime.shutdown_background();
                return Err(format!("java.lang.IllegalStateException: Failed to configure TLS for the server management protocol\nCaused by: {e}"));
            }
        }
    } else {
        None
    };
    let host = host.strip_prefix('[').and_then(|h| h.strip_suffix(']')).unwrap_or(&host).to_string();
    let server = ManagementServer::new(host, port, authentication_handler, tls, runtime.handle().clone());
    if let Err(e) = server.start() {
        runtime.shutdown_background();
        return Err(super::rcon::java_bind_exception(&e, false));
    }
    server.schedule_heartbeat(properties.status_heartbeat_interval);
    Ok(Some((server, runtime)))
}
