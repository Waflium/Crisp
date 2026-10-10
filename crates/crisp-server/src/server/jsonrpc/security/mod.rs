pub mod authentication_handler;
pub mod json_rpc_ssl_context_provider;

pub fn is_valid(secret_key: &str) -> bool {
    secret_key.len() == 40 && secret_key.bytes().all(|b| b.is_ascii_alphanumeric())
}
