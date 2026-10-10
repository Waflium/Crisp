use std::collections::HashSet;

use crate::server::jsonrpc::http_server_codec::HttpRequest;

const SUBPROTOCOL_HEADER_PREFIX: &str = "minecraft-v1,";
const BEARER_PREFIX: &str = "Bearer ";

pub enum SecurityCheckResult {
    Allowed(bool),
    Denied(&'static str),
}

pub struct AuthenticationHandler {
    secret_key: String,
    allowed_origins: HashSet<String>,
}

fn java_trim(s: &str) -> &str {
    s.trim_matches(|c: char| c <= ' ')
}

impl AuthenticationHandler {
    pub fn new(secret_key: String, allowed_origins: &str) -> AuthenticationHandler {
        AuthenticationHandler { secret_key, allowed_origins: allowed_origins.split(',').map(str::to_string).collect() }
    }

    pub fn perform_security_checks(&self, request: &HttpRequest) -> SecurityCheckResult {
        if let Some(token) = request.header("authorization").and_then(|h| h.strip_prefix(BEARER_PREFIX)) {
            return if self.is_valid_api_key(java_trim(token)) { SecurityCheckResult::Allowed(false) } else { SecurityCheckResult::Denied("Invalid API key") };
        }
        let Some(token) = request.header("sec-websocket-protocol").and_then(|h| h.strip_prefix(SUBPROTOCOL_HEADER_PREFIX)) else {
            return SecurityCheckResult::Denied("Missing API key");
        };
        if !self.is_allowed_origin_header(request) {
            return SecurityCheckResult::Denied("Origin Not Allowed");
        }
        if self.is_valid_api_key(java_trim(token)) { SecurityCheckResult::Allowed(true) } else { SecurityCheckResult::Denied("Invalid API key") }
    }

    fn is_allowed_origin_header(&self, request: &HttpRequest) -> bool {
        request.header("origin").is_some_and(|origin| !origin.is_empty() && self.allowed_origins.contains(origin))
    }

    fn is_valid_api_key(&self, supplied_key: &str) -> bool {
        if supplied_key.is_empty() {
            return false;
        }
        let (supplied, configured) = (supplied_key.as_bytes(), self.secret_key.as_bytes());
        supplied.len() == configured.len() && supplied.iter().zip(configured).fold(0u8, |acc, (a, b)| acc | (a ^ b)) == 0
    }
}
