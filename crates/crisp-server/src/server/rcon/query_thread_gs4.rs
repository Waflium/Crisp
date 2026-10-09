use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr, ToSocketAddrs};
use std::sync::{Arc, RwLock};
use std::time::Duration;

use rand::RngExt;
use tokio::net::UdpSocket;
use tokio::task::AbortHandle;

use super::network_data_output_stream::NetworkDataOutputStream;
use super::start_generic_thread;

const GAME_TYPE: &str = "SMP";
const GAME_ID: &str = "MINECRAFT";
const CHALLENGE_CHECK_INTERVAL: i64 = 30000;
const RESPONSE_CACHE_TIME: i64 = 5000;
const MAX_PACKET_SIZE: usize = 1460;
const SERVER_THREAD: &str = "Server thread";

pub struct QueryServerInterface {
    pub server_ip: String,
    pub server_port: u16,
    pub server_name: String,
    pub max_players: u32,
    pub level_id_name: String,
    pub player_names_array: Arc<RwLock<Arc<[String]>>>,
}

struct RequestChallenge {
    time: i64,
    challenge: i32,
    ident_bytes: [u8; 4],
    challenge_bytes: Vec<u8>,
}

impl RequestChallenge {
    fn new(buf: &[u8]) -> RequestChallenge {
        let ident_bytes = [buf[3], buf[4], buf[5], buf[6]];
        let ident = String::from_utf8_lossy(&ident_bytes);
        let challenge = rand::rng().random_range(0..16777216);
        let challenge_bytes = format!("\t{ident}{challenge}\0").into_bytes();
        RequestChallenge { time: crate::world::entity::player::profile_public_key::now_millis(), challenge, ident_bytes, challenge_bytes }
    }

    fn before(&self, time: i64) -> bool {
        self.time < time
    }
}

struct QueryThreadGs4 {
    last_challenge_check: i64,
    port: u16,
    server: QueryServerInterface,
    socket: UdpSocket,
    buffer: [u8; MAX_PACKET_SIZE],
    host_ip: String,
    server_ip: String,
    valid_challenges: HashMap<SocketAddr, RequestChallenge>,
    rules_response: NetworkDataOutputStream,
    last_rules_response: i64,
}

pub struct QueryThreadHandle {
    task: AbortHandle,
}

impl QueryThreadHandle {
    pub fn create(server: QueryServerInterface, port: i32, runtime: &tokio::runtime::Handle) -> Option<QueryThreadHandle> {
        if !(1..=65535).contains(&port) {
            warn!("Invalid query port {port} found in server.properties (queries disabled)");
            return None;
        }
        let port = port as u16;
        let mut server_ip = server.server_ip.clone();
        let mut host_ip = "0.0.0.0".to_string();
        if !server_ip.is_empty() && host_ip != server_ip {
            host_ip = server_ip.clone();
        } else {
            server_ip = "0.0.0.0".to_string();
            match get_local_host() {
                Some(address) => host_ip = address,
                None => warn!("Unable to determine local host IP, please set server-ip in server.properties"),
            }
        }
        let socket = init_socket(&server_ip, port, runtime)?;
        let name = start_generic_thread(SERVER_THREAD, "Query Listener");
        let thread = QueryThreadGs4 {
            last_challenge_check: 0,
            port,
            server,
            socket,
            buffer: [0; MAX_PACKET_SIZE],
            host_ip,
            server_ip,
            valid_challenges: HashMap::new(),
            rules_response: NetworkDataOutputStream::new(MAX_PACKET_SIZE),
            last_rules_response: 0,
        };
        let task = runtime.spawn(thread.run(name, runtime.clone()));
        Some(QueryThreadHandle { task: task.abort_handle() })
    }

    pub fn stop(&self) {
        self.task.abort();
        info!("Thread Query Listener stopped");
    }
}

fn init_socket(server_ip: &str, port: u16, runtime: &tokio::runtime::Handle) -> Option<UdpSocket> {
    let _guard = runtime.enter();
    let result = (server_ip, port)
        .to_socket_addrs()
        .and_then(|mut addresses| addresses.next().ok_or_else(|| std::io::Error::other("unknown host")))
        .and_then(std::net::UdpSocket::bind)
        .and_then(|socket| {
            socket.set_nonblocking(true)?;
            UdpSocket::from_std(socket)
        });
    match result {
        Ok(socket) => Some(socket),
        Err(e) => {
            warn!("Unable to initialise query system on {server_ip}:{port}\n{}", super::java_bind_exception(&e, true));
            None
        }
    }
}

fn get_local_host() -> Option<String> {
    let name = gethostname::gethostname().into_string().ok()?;
    let addresses: Vec<SocketAddr> = (name.as_str(), 0).to_socket_addrs().ok()?.collect();
    let address = addresses.iter().find(|a| a.is_ipv4()).or(addresses.first())?;
    Some(match address.ip() {
        IpAddr::V4(v4) => v4.to_string(),
        IpAddr::V6(v6) => v6.segments().map(|s| format!("{s:x}")).join(":"),
    })
}

impl QueryThreadGs4 {
    async fn run(mut self, name: String, runtime: tokio::runtime::Handle) {
        info!(thread: &name, "Query running on {}:{}", self.server_ip, self.port);
        self.last_challenge_check = crate::util::get_millis();
        loop {
            match tokio::time::timeout(Duration::from_millis(500), self.socket.recv_from(&mut self.buffer)).await {
                Err(_) => self.prune_challenges(),
                Ok(Ok((len, src))) => {
                    self.prune_challenges();
                    let _ = self.process_packet(len, src).await;
                }
                Ok(Err(e)) if e.raw_os_error() == Some(10040) => {}
                Ok(Err(e)) if e.kind() == std::io::ErrorKind::ConnectionReset => {}
                Ok(Err(e)) => {
                    warn!(thread: &name, "Unexpected exception\n{e}");
                    match init_socket(&self.server_ip, self.port, &runtime) {
                        Some(socket) => self.socket = socket,
                        None => {
                            error!(thread: &name, "Failed to recover from exception, shutting down!");
                            return;
                        }
                    }
                }
            }
        }
    }

    async fn send_to(&self, data: &[u8], src: SocketAddr) -> std::io::Result<()> {
        self.socket.send_to(data, src).await.map(|_| ())
    }

    async fn process_packet(&mut self, len: usize, src: SocketAddr) -> std::io::Result<bool> {
        let buf = self.buffer;
        if !(3 <= len && buf[0] == 0xFE && buf[1] == 0xFD) {
            return Ok(false);
        }
        match buf[2] {
            0 => {
                if !self.valid_challenge(len, src) {
                    return Ok(false);
                }
                if len == 15 {
                    let response = self.build_rule_response(src);
                    self.send_to(&response, src).await?;
                } else {
                    let mut dos = NetworkDataOutputStream::new(MAX_PACKET_SIZE);
                    dos.write(0);
                    dos.write_bytes(&self.get_ident_bytes(src));
                    dos.write_string(&self.server.server_name);
                    dos.write_string(GAME_TYPE);
                    dos.write_string(&self.server.level_id_name);
                    dos.write_string(&self.server.player_names_array.read().unwrap().len().to_string());
                    dos.write_string(&self.server.max_players.to_string());
                    dos.write_short(self.server.server_port as i16);
                    dos.write_string(&self.host_ip);
                    self.send_to(dos.to_byte_array(), src).await?;
                }
                Ok(true)
            }
            9 => {
                self.send_challenge(src).await?;
                Ok(true)
            }
            _ => Ok(true),
        }
    }

    fn build_rule_response(&mut self, src: SocketAddr) -> Vec<u8> {
        let now = crate::util::get_millis();
        let ident = self.get_ident_bytes(src);
        if now < self.last_rules_response + RESPONSE_CACHE_TIME {
            let mut data = self.rules_response.to_byte_array().to_vec();
            data[1..5].copy_from_slice(&ident);
            return data;
        }
        self.last_rules_response = now;
        let players = self.server.player_names_array.read().unwrap().clone();
        let response = &mut self.rules_response;
        response.reset();
        response.write(0);
        response.write_bytes(&ident);
        response.write_string("splitnum");
        response.write(128);
        response.write(0);
        response.write_string("hostname");
        response.write_string(&self.server.server_name);
        response.write_string("gametype");
        response.write_string(GAME_TYPE);
        response.write_string("game_id");
        response.write_string(GAME_ID);
        response.write_string("version");
        response.write_string(crisp_data::VERSION_NAME);
        response.write_string("plugins");
        response.write_string("");
        response.write_string("map");
        response.write_string(&self.server.level_id_name);
        response.write_string("numplayers");
        response.write_string(&players.len().to_string());
        response.write_string("maxplayers");
        response.write_string(&self.server.max_players.to_string());
        response.write_string("hostport");
        response.write_string(&self.server.server_port.to_string());
        response.write_string("hostip");
        response.write_string(&self.host_ip);
        response.write(0);
        response.write(1);
        response.write_string("player_");
        response.write(0);
        for player in players.iter() {
            response.write_string(player);
        }
        response.write(0);
        response.to_byte_array().to_vec()
    }

    fn get_ident_bytes(&self, src: SocketAddr) -> [u8; 4] {
        self.valid_challenges[&src].ident_bytes
    }

    fn valid_challenge(&self, len: usize, src: SocketAddr) -> bool {
        self.valid_challenges.get(&src).is_some_and(|c| c.challenge == int_from_network_byte_array(&self.buffer, 7, len))
    }

    async fn send_challenge(&mut self, src: SocketAddr) -> std::io::Result<()> {
        let challenge = RequestChallenge::new(&self.buffer);
        let bytes = challenge.challenge_bytes.clone();
        self.valid_challenges.insert(src, challenge);
        self.send_to(&bytes, src).await
    }

    fn prune_challenges(&mut self) {
        let now = crate::util::get_millis();
        if now >= self.last_challenge_check + CHALLENGE_CHECK_INTERVAL {
            self.last_challenge_check = now;
            self.valid_challenges.retain(|_, c| !c.before(now));
        }
    }
}

fn int_from_network_byte_array(b: &[u8], offset: usize, length: usize) -> i32 {
    if length < offset + 4 {
        return 0;
    }
    i32::from_be_bytes(b[offset..offset + 4].try_into().unwrap())
}
