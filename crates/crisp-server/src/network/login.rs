use std::net::SocketAddr;
use std::sync::LazyLock;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use anyhow::bail;
use crisp_data::packets::login::{clientbound, serverbound};
use md5::{Digest, Md5};
use tokio::sync::{mpsc, oneshot};

use crate::server::services::Services;
use crate::ServerConfig;
use crate::network::chat::{Arg, Component};
use crate::network::connection::{PacketReader, PacketWriter, disconnection_reason};
use crate::network::encryption::{ServerKeyPair, server_hash};
use crate::network::server_connection_listener::loggable_address;
use crate::server::Event;

#[derive(Clone, Debug, PartialEq)]
pub struct ProfileProperty {
    pub name: String,
    pub value: String,
    pub signature: Option<String>,
}

#[derive(Clone, Debug)]
pub struct Profile {
    pub uuid: u128,
    pub name: String,
    pub properties: Vec<ProfileProperty>,
}

impl Profile {
    pub fn write(&self, p: &mut crisp_protocol::Writer) {
        p.uuid(self.uuid);
        p.string(&self.name);
        self.write_properties(p);
    }

    pub fn write_properties(&self, p: &mut crisp_protocol::Writer) {
        p.varint(self.properties.len() as i32);
        for property in &self.properties {
            p.string(&property.name);
            p.string(&property.value);
            p.bool(property.signature.is_some());
            if let Some(signature) = &property.signature {
                p.string(signature);
            }
        }
    }
}

pub fn offline_uuid(name: &str) -> u128 {
    let mut bytes: [u8; 16] = Md5::digest(format!("OfflinePlayer:{name}")).into();
    bytes[6] = bytes[6] & 0x0F | 0x30;
    bytes[8] = bytes[8] & 0x3F | 0x80;
    u128::from_be_bytes(bytes)
}

fn is_valid_player_name(name: &str) -> bool {
    name.encode_utf16().count() <= 16 && !name.encode_utf16().any(|c| c <= 32 || c >= 127)
}

const LOGIN_TIMEOUT: Duration = Duration::from_secs(30);

pub enum Verdict {
    Accept { session_id: u128 },
    Disconnect(Component),
}

pub async fn login(
    r: &mut PacketReader,
    w: &mut PacketWriter,
    config: &ServerConfig,
    keys: &ServerKeyPair,
    services: &Services,
    events: &mpsc::UnboundedSender<Event>,
    address: SocketAddr,
) -> anyhow::Result<Option<Profile>> {
    let mut user = LoginUser { requested_username: None, address: loggable_address(address, config.log_ips) };
    match tokio::time::timeout(LOGIN_TIMEOUT, run(r, w, config, keys, services, events, address, &mut user)).await {
        Ok(Err(e)) if e.downcast_ref::<std::io::Error>().is_some() => {
            user.lost_connection(&disconnection_reason(&e).get_string());
            Ok(None)
        }
        Ok(result) => result,
        Err(_) => {
            disconnect(w, &user, SERVER_THREAD, Component::translatable("multiplayer.disconnect.slow_login")).await?;
            Ok(None)
        }
    }
}

const SERVER_THREAD: &str = "Server thread";
static UNIQUE_THREAD_ID: AtomicUsize = AtomicUsize::new(0);

struct LoginUser {
    requested_username: Option<String>,
    address: String,
}

impl LoginUser {
    fn name(&self) -> String {
        match &self.requested_username {
            Some(name) => format!("{name} ({})", self.address),
            None => self.address.clone(),
        }
    }

    fn lost_connection(&self, reason: &str) {
        info!(thread: SERVER_THREAD, "{} lost connection: {reason}", self.name());
    }
}

#[allow(clippy::too_many_arguments)]
async fn run(
    r: &mut PacketReader,
    w: &mut PacketWriter,
    config: &ServerConfig,
    keys: &ServerKeyPair,
    services: &Services,
    events: &mpsc::UnboundedSender<Event>,
    address: SocketAddr,
    user: &mut LoginUser,
) -> anyhow::Result<Option<Profile>> {
    let hello = r.expect(serverbound::HELLO).await?;
    let name = hello.reader().string(16)?.to_owned();
    if !is_valid_player_name(&name) {
        return protocol_error(w, user, "java.lang.IllegalStateException: Invalid characters in username").await;
    }
    user.requested_username = Some(name.clone());
    let profile = if config.online_mode {
        let challenge = rand::random::<i32>().to_be_bytes();
        w.send(clientbound::HELLO, |p| {
            p.string("");
            p.varint(keys.public_der.len() as i32);
            p.bytes(&keys.public_der);
            p.varint(4);
            p.bytes(&challenge);
            p.bool(true);
        })
        .await?;
        let key = r.expect(serverbound::KEY).await?;
        let mut kr = key.reader();
        let secret_len = kr.varint()? as usize;
        let encrypted_secret = kr.take(secret_len)?.to_vec();
        let challenge_len = kr.varint()? as usize;
        let encrypted_challenge = kr.take(challenge_len)?;
        let valid = keys.decrypt(encrypted_challenge).is_ok_and(|c| c == challenge);
        let secret: Option<[u8; 16]> = keys.decrypt(&encrypted_secret).ok().and_then(|s| s.try_into().ok());
        let (true, Some(secret)) = (valid, secret) else {
            return protocol_error(w, user, "java.lang.IllegalStateException: Protocol error").await;
        };
        let hash = server_hash("", &secret, &keys.public_der);
        r.enable_encryption(&secret);
        w.enable_encryption(&secret);
        let thread = format!("User Authenticator #{}", UNIQUE_THREAD_ID.fetch_add(1, Ordering::Relaxed) + 1);
        let ip = config.prevent_proxy_connections.then(|| host_address(address.ip()));
        match has_joined_server(services, &name, &hash, ip.as_deref()).await {
            Ok(Some(profile)) => {
                info!(thread: &thread, "UUID of player {} is {}", profile.name, crate::world::level::storage::format_uuid(profile.uuid));
                profile
            }
            Ok(None) => {
                error!(thread: &thread, "Username '{name}' tried to join with an invalid session");
                disconnect(w, user, &thread, Component::translatable("multiplayer.disconnect.unverified_username")).await?;
                return Ok(None);
            }
            Err(_) => {
                error!(thread: &thread, "Couldn't verify username because servers are unavailable");
                disconnect(w, user, &thread, Component::translatable("multiplayer.disconnect.authservers_down")).await?;
                return Ok(None);
            }
        }
    } else {
        Profile { uuid: offline_uuid(&name), name, properties: Vec::new() }
    };

    let (reply, verdict) = oneshot::channel();
    events.send(Event::Verify { profile: profile.clone(), address, reply })?;
    let session_id = match verdict.await? {
        Verdict::Accept { session_id } => session_id,
        Verdict::Disconnect(reason) => {
            disconnect(w, user, SERVER_THREAD, reason).await?;
            return Ok(None);
        }
    };
    if let Some(threshold) = config.compression_threshold {
        w.send(clientbound::LOGIN_COMPRESSION, |p| p.varint(threshold as i32)).await?;
        w.codec.threshold = Some(threshold);
        r.codec.threshold = Some(threshold);
    }
    w.send(clientbound::LOGIN_FINISHED, |p| {
        profile.write(p);
        p.uuid(session_id);
    })
    .await?;
    r.expect(serverbound::LOGIN_ACKNOWLEDGED).await?;
    Ok(Some(profile))
}

async fn disconnect(w: &mut PacketWriter, user: &LoginUser, thread: &str, reason: Component) -> anyhow::Result<()> {
    let reason_text = reason.get_string();
    info!(thread: thread, "Disconnecting {}: {reason_text}", user.name());
    let sent = w.send(clientbound::LOGIN_DISCONNECT, |p| p.string(&reason.to_json())).await;
    user.lost_connection(&reason_text);
    warn!(thread: SERVER_THREAD, "handleDisconnection() called twice");
    Ok(sent?)
}

async fn protocol_error(w: &mut PacketWriter, user: &LoginUser, exception: &str) -> anyhow::Result<Option<Profile>> {
    let reason = Component::translatable_with("disconnect.genericReason", vec![Arg::String(format!("Internal Exception: {exception}"))]);
    let _ = w.send(clientbound::LOGIN_DISCONNECT, |p| p.string(&reason.to_json())).await;
    user.lost_connection(&reason.get_string());
    Ok(None)
}

static HTTP: LazyLock<reqwest::Client> = LazyLock::new(|| reqwest::Client::builder().timeout(Duration::from_secs(10)).build().expect("an HTTP client"));

fn host_address(ip: std::net::IpAddr) -> String {
    match ip {
        std::net::IpAddr::V4(v4) => v4.to_string(),
        std::net::IpAddr::V6(v6) => v6.segments().map(|s| format!("{s:x}")).join(":"),
    }
}

async fn has_joined_server(services: &Services, name: &str, server_hash: &str, ip: Option<&str>) -> anyhow::Result<Option<Profile>> {
    let Some(url) = services.url_async("session", "verify").await else { bail!("Services are unavailable") };
    let mut request = HTTP.get(url).query(&[("username", name), ("serverId", server_hash)]);
    if let Some(ip) = ip {
        request = request.query(&[("ip", ip)]);
    }
    let response = request.send().await?;
    let status = response.status();
    if status.is_server_error() {
        bail!("session server returned {status}");
    }
    if status != reqwest::StatusCode::OK {
        return Ok(None);
    }
    let json: serde_json::Value = response.json().await?;
    let (Some(id), Some(name)) = (json["id"].as_str(), json["name"].as_str()) else { return Ok(None) };
    let uuid = u128::from_str_radix(id, 16)?;
    let properties = json["properties"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|p| {
            Some(ProfileProperty {
                name: p["name"].as_str()?.to_string(),
                value: p["value"].as_str()?.to_string(),
                signature: p["signature"].as_str().map(str::to_string),
            })
        })
        .collect();
    Ok(Some(Profile { uuid, name: name.to_string(), properties }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offline_uuid_matches_vanilla() {
        assert_eq!(offline_uuid("Notch"), 0xb50ad385_829d_3141_a216_7e7d7539ba7f);
    }

    #[test]
    fn player_names() {
        assert!(is_valid_player_name("Steve_01"));
        assert!(is_valid_player_name("!~"));
        assert!(!is_valid_player_name("has space"));
        assert!(!is_valid_player_name("ä"));
        assert!(!is_valid_player_name("seventeen_chars_x"));
    }
}
