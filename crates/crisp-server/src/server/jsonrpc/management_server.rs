use std::sync::atomic::{AtomicBool, AtomicI32, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use serde_json::Value;
use tokio::net::TcpListener;
use tokio::sync::{mpsc, oneshot};
use tokio_rustls::TlsAcceptor;

use super::connection::Connection;
use super::json_rpc_utils;
use super::methods::server_state_service;
use super::outgoing_rpc_methods::{self, OutgoingRpcMethod};
use super::security::authentication_handler::AuthenticationHandler;
use crate::server::level::chunk_workers::WorkResult;
use crate::server::{Server, ServerTask};

pub struct ManagementServer {
    host: String,
    port: AtomicI32,
    authentication_handler: AuthenticationHandler,
    tls: Option<TlsAcceptor>,
    connections: Mutex<Vec<(i32, mpsc::UnboundedSender<Arc<str>>)>>,
    next_connection_id: AtomicI32,
    next_event_loop: AtomicUsize,
    event_loops: usize,
    heartbeat: Mutex<Option<tokio::task::AbortHandle>>,
    listener: Mutex<Option<tokio::task::AbortHandle>>,
    runtime: tokio::runtime::Handle,
    tasks: OnceLock<std::sync::mpsc::Sender<ServerTask>>,
    waker: OnceLock<std::sync::mpsc::Sender<WorkResult>>,
    ready: AtomicBool,
}

impl ManagementServer {
    pub fn new(host: String, port: i32, authentication_handler: AuthenticationHandler, tls: Option<TlsAcceptor>, runtime: tokio::runtime::Handle) -> Arc<ManagementServer> {
        let event_loops = std::thread::available_parallelism().map_or(1, |n| n.get()) * 2;
        Arc::new(ManagementServer {
            host,
            port: AtomicI32::new(port),
            authentication_handler,
            tls,
            connections: Mutex::new(Vec::new()),
            next_connection_id: AtomicI32::new(0),
            next_event_loop: AtomicUsize::new(0),
            event_loops,
            heartbeat: Mutex::new(None),
            listener: Mutex::new(None),
            runtime,
            tasks: OnceLock::new(),
            waker: OnceLock::new(),
            ready: AtomicBool::new(false),
        })
    }

    pub fn authentication_handler(&self) -> &AuthenticationHandler {
        &self.authentication_handler
    }

    fn next_event_loop(&self) -> usize {
        self.next_event_loop.fetch_add(1, Ordering::Relaxed) % self.event_loops
    }

    pub fn start(self: &Arc<ManagementServer>) -> std::io::Result<()> {
        let port = self.port.load(Ordering::Relaxed) as u16;
        let listener = self.runtime.block_on(async {
            let mut addresses: Vec<std::net::SocketAddr> = tokio::net::lookup_host((self.host.as_str(), port)).await?.collect();
            addresses.sort_by_key(|a| a.is_ipv6());
            let address = addresses.first().copied().ok_or_else(|| std::io::Error::other("Unresolved address"))?;
            TcpListener::bind(address).await
        })?;
        self.next_event_loop();
        self.port.store(listener.local_addr()?.port() as i32, Ordering::Relaxed);
        info!("Json-RPC Management connection listening on {}:{}", self.host, self.port.load(Ordering::Relaxed));
        let server = self.clone();
        let task = self.runtime.spawn(async move {
            while let Ok((stream, remote)) = listener.accept().await {
                let _ = stream.set_nodelay(true);
                let id = server.next_connection_id.fetch_add(1, Ordering::Relaxed) + 1;
                let thread = format!("Management server IO #{}", server.next_event_loop());
                let connection = Connection::new(server.clone(), id, thread);
                match server.tls.clone() {
                    Some(acceptor) => tokio::spawn(connection.run_tls(stream, remote, acceptor)),
                    None => tokio::spawn(connection.run(stream, remote)),
                };
            }
        });
        *self.listener.lock().unwrap() = Some(task.abort_handle());
        Ok(())
    }

    pub fn schedule_heartbeat(self: &Arc<ManagementServer>, period: i32) -> bool {
        if let Some(heartbeat) = self.heartbeat.lock().unwrap().take() {
            heartbeat.abort();
        }
        if period > 0 {
            self.next_event_loop();
            let server = self.clone();
            let period = Duration::from_secs(period as u64);
            let task = self.runtime.spawn(async move {
                let mut interval = tokio::time::interval_at(tokio::time::Instant::now() + period, period);
                loop {
                    interval.tick().await;
                    server.status_heartbeat();
                }
            });
            *self.heartbeat.lock().unwrap() = Some(task.abort_handle());
        }
        true
    }

    fn status_heartbeat(self: &Arc<ManagementServer>) {
        if !self.ready.load(Ordering::Acquire) {
            self.broadcast(&outgoing_rpc_methods::STATUS_HEARTBEAT, || Some(server_state_service::not_started()));
            return;
        }
        let server = self.clone();
        self.submit(move |s| server.broadcast(&outgoing_rpc_methods::STATUS_HEARTBEAT, || Some(server_state_service::current_status(s))));
    }

    pub fn on_connected(&self, id: i32, sender: mpsc::UnboundedSender<Arc<str>>) {
        self.connections.lock().unwrap().push((id, sender));
    }

    pub fn on_disconnected(&self, id: i32) {
        self.connections.lock().unwrap().retain(|(i, _)| *i != id);
    }

    pub fn set_server(&self, tasks: std::sync::mpsc::Sender<ServerTask>) {
        let _ = self.tasks.set(tasks);
    }

    pub fn set_waker(&self, waker: std::sync::mpsc::Sender<WorkResult>) {
        let _ = self.waker.set(waker);
    }

    pub fn has_server(&self) -> bool {
        self.tasks.get().is_some()
    }

    pub fn set_ready(&self) {
        self.ready.store(true, Ordering::Release);
    }

    pub fn submit<R: Send + 'static>(&self, task: impl FnOnce(&mut Server) -> R + Send + Sync + 'static) -> Option<oneshot::Receiver<R>> {
        let tasks = self.tasks.get()?;
        let (reply, receiver) = oneshot::channel();
        tasks
            .send(Box::new(move |server| {
                let _ = reply.send(task(server));
            }))
            .ok()?;
        if let Some(waker) = self.waker.get() {
            let _ = waker.send(WorkResult::Wake);
        }
        Some(receiver)
    }

    pub fn has_connections(&self) -> bool {
        !self.connections.lock().unwrap().is_empty()
    }

    pub fn broadcast(&self, method: &OutgoingRpcMethod, params: impl FnOnce() -> Option<Value>) {
        if !self.has_connections() || (!self.has_server() && !method.allow_pre_server_init) {
            return;
        }
        let text: Arc<str> = json_rpc_utils::create_notification(method.name, params()).to_string().into();
        for (_, sender) in self.connections.lock().unwrap().iter() {
            let _ = sender.send(text.clone());
        }
    }

    pub fn stop(&self) {
        if let Some(listener) = self.listener.lock().unwrap().take() {
            listener.abort();
        }
        if let Some(heartbeat) = self.heartbeat.lock().unwrap().take() {
            heartbeat.abort();
        }
        self.connections.lock().unwrap().clear();
    }
}
