use std::sync::{Arc, Mutex};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, oneshot, Notify};
use tokio::task::{AbortHandle, JoinHandle};

use super::{java_inet_address, start_generic_thread};
use crate::server::Event;

const MAX_PACKET_SIZE: usize = 1460;
const SERVERDATA_AUTH: i32 = 3;
const SERVERDATA_EXECCOMMAND: i32 = 2;
const SERVERDATA_RESPONSE_VALUE: i32 = 0;
const SERVERDATA_AUTH_RESPONSE: i32 = 2;
const SERVERDATA_AUTH_FAILURE: i32 = -1;
const SERVER_THREAD: &str = "Server thread";

struct RconClientHandle {
    name: String,
    shutdown: Arc<Notify>,
    task: JoinHandle<()>,
}

pub struct RconThread {
    listener: AbortHandle,
    clients: Arc<Mutex<Vec<RconClientHandle>>>,
    runtime: tokio::runtime::Handle,
}

impl RconThread {
    pub fn create(server_ip: &str, port: i32, password: &str, events: mpsc::UnboundedSender<Event>, runtime: &tokio::runtime::Handle) -> Option<RconThread> {
        let server_ip = if server_ip.is_empty() { "0.0.0.0" } else { server_ip };
        if !(1..=65535).contains(&port) {
            warn!("Invalid rcon port {port} found in server.properties, rcon disabled!");
            return None;
        }
        if password.is_empty() {
            warn!("No rcon password set in server.properties, rcon disabled!");
            return None;
        }
        let socket = match runtime.block_on(TcpListener::bind((server_ip, port as u16))) {
            Ok(socket) => socket,
            Err(e) => {
                warn!("Unable to initialise RCON on {server_ip}:{port}\n{}", super::java_bind_exception(&e, false));
                return None;
            }
        };
        let listener_name = start_generic_thread(SERVER_THREAD, "RCON Listener");
        let clients: Arc<Mutex<Vec<RconClientHandle>>> = Arc::default();
        let password = password.to_string();
        let tracked = clients.clone();
        let listener = runtime.spawn(async move {
            while let Ok((stream, address)) = socket.accept().await {
                let name = format!("RCON Client {}", java_inet_address(address.ip()));
                let thread = start_generic_thread(&listener_name, &name);
                let shutdown = Arc::new(Notify::new());
                let task = tokio::spawn(run_client(stream, password.clone(), events.clone(), name.clone(), thread, shutdown.clone()));
                let mut clients = tracked.lock().unwrap();
                clients.retain(|c| !c.task.is_finished());
                clients.push(RconClientHandle { name, shutdown, task });
            }
        });
        info!("RCON running on {server_ip}:{port}");
        Some(RconThread { listener: listener.abort_handle(), clients, runtime: runtime.clone() })
    }

    pub fn stop(&self) {
        self.listener.abort();
        info!("Thread RCON Listener stopped");
        for client in self.clients.lock().unwrap().drain(..) {
            if !client.task.is_finished() {
                client.shutdown.notify_one();
                let _ = self.runtime.block_on(client.task);
                info!("Thread {} stopped", client.name);
            }
        }
    }
}

async fn run_client(mut stream: TcpStream, password: String, events: mpsc::UnboundedSender<Event>, name: String, thread: String, shutdown: Arc<Notify>) {
    tokio::select! {
        _ = handle_client(&mut stream, password, events) => {}
        _ = shutdown.notified() => {}
    }
    let _ = stream.shutdown().await;
    info!(thread: &thread, "Thread {name} shutting down");
}

async fn handle_client(stream: &mut TcpStream, password: String, events: mpsc::UnboundedSender<Event>) {
    let mut authed = false;
    let mut buf = [0u8; MAX_PACKET_SIZE];
    loop {
        let read = match stream.read(&mut buf).await {
            Ok(read) => read,
            Err(_) => break,
        };
        if read < 10 {
            break;
        }
        let packet_size = int_from_byte_array(&buf, 0, read);
        if packet_size != read as i32 - 4 {
            break;
        }
        let request_id = int_from_byte_array(&buf, 4, read);
        let cmd = int_from_byte_array(&buf, 8, buf.len());
        let result = match cmd {
            SERVERDATA_EXECCOMMAND => {
                if authed {
                    let command = string_from_byte_array(&buf, 12, read);
                    let (reply, response) = oneshot::channel();
                    if events.send(Event::Rcon { command: command.clone(), reply }).is_err() {
                        break;
                    }
                    let response = response.await.unwrap_or_else(|_| format!("Error executing: {command} (null)"));
                    send_cmd_response(stream, request_id, &response).await
                } else {
                    send(stream, SERVERDATA_AUTH_FAILURE, SERVERDATA_AUTH_RESPONSE, "").await
                }
            }
            SERVERDATA_AUTH => {
                let given = string_from_byte_array(&buf, 12, read);
                if !given.is_empty() && given == password {
                    authed = true;
                    send(stream, request_id, SERVERDATA_AUTH_RESPONSE, "").await
                } else {
                    authed = false;
                    send(stream, SERVERDATA_AUTH_FAILURE, SERVERDATA_AUTH_RESPONSE, "").await
                }
            }
            other => send_cmd_response(stream, request_id, &format!("Unknown request {:x}", other as u32)).await,
        };
        if result.is_err() {
            break;
        }
    }
}

fn int_from_byte_array(b: &[u8], offset: usize, length: usize) -> i32 {
    if length < offset + 4 {
        return 0;
    }
    i32::from_le_bytes(b[offset..offset + 4].try_into().unwrap())
}

fn string_from_byte_array(b: &[u8], offset: usize, length: usize) -> String {
    let max = length - 1;
    let mut i = offset.min(max);
    while b[i] != 0 && i < max {
        i += 1;
    }
    String::from_utf8_lossy(&b[offset.min(i)..i]).into_owned()
}

async fn send(stream: &mut TcpStream, request_id: i32, cmd: i32, text: &str) -> std::io::Result<()> {
    let bytes = text.as_bytes();
    let mut packet = Vec::with_capacity(bytes.len() + 14);
    packet.extend((bytes.len() as i32 + 10).to_le_bytes());
    packet.extend(request_id.to_le_bytes());
    packet.extend(cmd.to_le_bytes());
    packet.extend_from_slice(bytes);
    packet.extend([0, 0]);
    stream.write_all(&packet).await
}

async fn send_cmd_response(stream: &mut TcpStream, request_id: i32, response: &str) -> std::io::Result<()> {
    let units: Vec<u16> = response.encode_utf16().collect();
    let mut start = 0;
    loop {
        let end = (start + 4096).min(units.len());
        send(stream, request_id, SERVERDATA_RESPONSE_VALUE, &String::from_utf16_lossy(&units[start..end])).await?;
        start = end;
        if start >= units.len() {
            return Ok(());
        }
    }
}
