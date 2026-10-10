use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use futures_util::{FutureExt, SinkExt, StreamExt};
use serde_json::{Map, Value};
use sha1::{Digest, Sha1};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf};
use tokio::sync::mpsc;
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::protocol::{Role, WebSocketConfig};

use super::gson;
use super::http_server_codec::{HttpRequest, HttpServerCodec, Inbound, MAX_CONTENT_LENGTH};
use super::incoming_rpc_methods::{self, RpcFailure};
use super::json_rpc_utils::{self, JsonRpcErrors};
use super::management_server::ManagementServer;
use super::methods::ClientInfo;
use super::security::authentication_handler::SecurityCheckResult;
use crate::network::server_connection_listener::loggable_address;

const WEBSOCKET_GUID: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";
const HIXIE_MAX_FRAME_SIZE: usize = 16384;
const TAIL_EXCEPTION: &str =
    "An exceptionCaught() event was fired, and it reached at the tail of the pipeline. It usually means the last handler in the pipeline did not handle the exception.";

pub struct Connection {
    server: Arc<ManagementServer>,
    client: ClientInfo,
    thread: String,
}

enum Handshake {
    Rfc6455(String),
    Hixie(Vec<u8>),
}

enum Closed {
    Cleanly,
    Exception(String),
}

fn java_io_exception(error: &std::io::Error) -> String {
    match error.kind() {
        std::io::ErrorKind::ConnectionReset => "java.net.SocketException: Connection reset".into(),
        std::io::ErrorKind::ConnectionAborted => "java.io.IOException: An established connection was aborted by the software in your host machine".into(),
        _ => format!("java.io.IOException: {error}"),
    }
}

impl Connection {
    pub fn new(server: Arc<ManagementServer>, connection_id: i32, thread: String) -> Connection {
        Connection { server, client: ClientInfo { connection_id }, thread }
    }

    fn log(&self, message: std::fmt::Arguments) {
        info!(thread: &self.thread, "RPC Connection #{}: {message}", self.client.connection_id);
    }

    fn exception_caught(&self, exception: &str) {
        warn!(thread: &self.thread, "{TAIL_EXCEPTION}\n{exception}");
    }

    fn close(&self, codec: &mut HttpServerCodec) -> Closed {
        if codec.close() {
            Closed::Exception("io.netty.handler.codec.PrematureChannelClosureException: Channel closed while still aggregating message".into())
        } else {
            Closed::Cleanly
        }
    }

    pub async fn run<S: AsyncRead + AsyncWrite + Unpin + Send + 'static>(self, stream: S, remote: SocketAddr) {
        let address = loggable_address(remote, true);
        self.log(format_args!("Management connection opened for {address}"));
        if let Closed::Exception(exception) = self.serve(stream).await {
            self.exception_caught(&exception);
        }
        self.server.on_disconnected(self.client.connection_id);
        self.log(format_args!("Management connection closed for {address}"));
    }

    pub async fn run_tls<S: AsyncRead + AsyncWrite + Unpin + Send + 'static>(self, stream: S, remote: SocketAddr, acceptor: tokio_rustls::TlsAcceptor) {
        let address = loggable_address(remote, true);
        self.log(format_args!("Management connection opened for {address}"));
        match acceptor.accept(stream).await {
            Ok(tls) => {
                if let Closed::Exception(exception) = self.serve(tls).await {
                    self.exception_caught(&exception);
                }
            }
            Err(e) => {
                warn!(thread: &self.thread, "{TAIL_EXCEPTION}\nio.netty.handler.codec.DecoderException: javax.net.ssl.SSLHandshakeException: {e}");
            }
        }
        self.server.on_disconnected(self.client.connection_id);
        self.log(format_args!("Management connection closed for {address}"));
    }

    async fn serve<S: AsyncRead + AsyncWrite + Unpin + Send + 'static>(&self, mut stream: S) -> Closed {
        let mut codec = HttpServerCodec::new();
        let mut chunk = [0u8; 4096];
        loop {
            let request = match codec.poll() {
                Some(Inbound::Request(request)) => request,
                Some(Inbound::Write(response)) => {
                    if stream.write_all(response).await.is_err() {
                        return self.close(&mut codec);
                    }
                    continue;
                }
                Some(Inbound::WriteAndClose(response)) => {
                    let _ = stream.write_all(response).await;
                    return Closed::Cleanly;
                }
                None => match stream.read(&mut chunk).await {
                    Ok(0) => return self.close(&mut codec),
                    Ok(n) => {
                        codec.feed(&chunk[..n]);
                        continue;
                    }
                    Err(e) => {
                        self.exception_caught(&java_io_exception(&e));
                        return self.close(&mut codec);
                    }
                },
            };
            let check = self.server.authentication_handler().perform_security_checks(&request);
            let token_in_protocol = match check {
                SecurityCheckResult::Denied(reason) => {
                    let body = format!("{{\"error\":\"Unauthorized\",\"message\":\"{reason}\"}}");
                    let response = format!(
                        "HTTP/1.1 401 Unauthorized\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                        body.len()
                    );
                    let _ = stream.write_all(response.as_bytes()).await;
                    return Closed::Cleanly;
                }
                SecurityCheckResult::Allowed(token_in_protocol) => token_in_protocol,
            };
            if request.uri != "/" {
                continue;
            }
            let handshake = match handshake(&request, token_in_protocol) {
                Ok(Some(handshake)) => handshake,
                Ok(None) => {
                    if let Err(e) = stream.write_all(b"HTTP/1.1 426 Upgrade Required\r\nsec-websocket-version: 13\r\ncontent-length: 0\r\n\r\n").await {
                        return Closed::Exception(java_io_exception(&e));
                    }
                    continue;
                }
                Err(message) => {
                    let _ = stream.write_all(format!("HTTP/1.1 400 Bad Request\r\n\r\n{message}").as_bytes()).await;
                    return Closed::Cleanly;
                }
            };
            return match handshake {
                Handshake::Rfc6455(response) => {
                    if let Err(e) = stream.write_all(response.as_bytes()).await {
                        return Closed::Exception(java_io_exception(&e));
                    }
                    let config = WebSocketConfig::default().max_message_size(Some(MAX_CONTENT_LENGTH)).max_frame_size(Some(MAX_CONTENT_LENGTH));
                    let websocket = WebSocketStream::from_partially_read(ReadBatch { stream, open: true }, codec.into_remaining(), Role::Server, Some(config)).await;
                    self.websocket(websocket).await
                }
                Handshake::Hixie(response) => {
                    if let Err(e) = stream.write_all(&response).await {
                        return Closed::Exception(java_io_exception(&e));
                    }
                    self.hixie(stream, codec.into_remaining()).await
                }
            };
        }
    }

    async fn websocket<S: AsyncRead + AsyncWrite + Unpin + Send + 'static>(&self, mut websocket: WebSocketStream<ReadBatch<S>>) -> Closed {
        let (sender, mut notifications) = mpsc::unbounded_channel::<Arc<str>>();
        self.server.on_connected(self.client.connection_id, sender);
        loop {
            while let Some(message) = websocket.next().now_or_never() {
                if let Some(closed) = self.on_websocket_message(&mut websocket, message).await {
                    return closed;
                }
            }
            while let Ok(text) = notifications.try_recv() {
                if websocket.send(Message::text(text.to_string())).await.is_err() {
                    return Closed::Cleanly;
                }
            }
            websocket.get_mut().open = true;
            tokio::select! {
                biased;
                message = websocket.next() => {
                    if let Some(closed) = self.on_websocket_message(&mut websocket, message).await {
                        return closed;
                    }
                }
                notification = notifications.recv() => match notification {
                    Some(text) => {
                        if websocket.send(Message::text(text.to_string())).await.is_err() {
                            return Closed::Cleanly;
                        }
                    }
                    None => return Closed::Cleanly,
                },
            }
        }
    }

    async fn on_websocket_message<S: AsyncRead + AsyncWrite + Unpin>(
        &self,
        websocket: &mut WebSocketStream<ReadBatch<S>>,
        message: Option<Result<Message, tokio_tungstenite::tungstenite::Error>>,
    ) -> Option<Closed> {
        match message {
            Some(Ok(Message::Text(text))) => match self.handle_text(text.as_str()).await {
                Some(Ok(response)) => websocket.send(Message::text(response)).await.err().map(|e| Closed::Exception(format!("{e}"))),
                Some(Err(exception)) => Some(Closed::Exception(exception)),
                None => None,
            },
            Some(Ok(_)) => None,
            Some(Err(tokio_tungstenite::tungstenite::Error::Io(e))) => Some(Closed::Exception(java_io_exception(&e))),
            Some(Err(_)) | None => Some(Closed::Cleanly),
        }
    }

    async fn hixie<S: AsyncRead + AsyncWrite + Unpin + Send + 'static>(&self, mut stream: S, mut buffer: Vec<u8>) -> Closed {
        let (sender, mut notifications) = mpsc::unbounded_channel::<Arc<str>>();
        self.server.on_connected(self.client.connection_id, sender);
        let mut chunk = [0u8; 4096];
        let mut drained = true;
        loop {
            while let Some(frame) = take_hixie_frame(&mut buffer) {
                match frame {
                    HixieFrame::Text(text) => match self.handle_text(&text).await {
                        Some(Ok(response)) => {
                            if let Err(e) = write_hixie(&mut stream, &response).await {
                                return Closed::Exception(java_io_exception(&e));
                            }
                        }
                        Some(Err(exception)) => return Closed::Exception(exception),
                        None => {}
                    },
                    HixieFrame::Close => {
                        let _ = stream.write_all(&[0xFF, 0x00]).await;
                        return Closed::Cleanly;
                    }
                    HixieFrame::TooLong => return Closed::Exception("io.netty.handler.codec.TooLongFrameException".into()),
                    HixieFrame::Skip => {}
                }
            }
            if drained {
                while let Ok(text) = notifications.try_recv() {
                    if write_hixie(&mut stream, &text).await.is_err() {
                        return Closed::Cleanly;
                    }
                }
            }
            tokio::select! {
                biased;
                read = stream.read(&mut chunk) => match read {
                    Ok(0) => return Closed::Cleanly,
                    Ok(n) => {
                        buffer.extend_from_slice(&chunk[..n]);
                        drained = n < chunk.len();
                    }
                    Err(e) => return Closed::Exception(java_io_exception(&e)),
                },
                notification = notifications.recv() => match notification {
                    Some(text) => {
                        if write_hixie(&mut stream, &text).await.is_err() {
                            return Closed::Cleanly;
                        }
                    }
                    None => return Closed::Cleanly,
                },
            }
        }
    }

    async fn handle_text(&self, text: &str) -> Option<Result<String, String>> {
        let element = match gson::parse_string(text) {
            Ok(element) => element,
            Err(data) => return Some(Ok(JsonRpcErrors::ParseError.create_with_unknown_id(Some(&data)).to_string())),
        };
        match element {
            Value::Object(object) => self.handle_json_object(&object).await.map(|r| Ok(r.to_string())),
            Value::Array(entries) => {
                let mut responses = Vec::new();
                for entry in entries {
                    let Value::Object(object) = entry else {
                        return Some(Err(format!("java.lang.IllegalStateException: Not a JSON Object: {entry}")));
                    };
                    if let Some(response) = self.handle_json_object(&object).await {
                        responses.push(response);
                    }
                }
                Some(Ok(Value::Array(responses).to_string()))
            }
            _ => Some(Ok(JsonRpcErrors::InvalidRequest.create_with_unknown_id(None).to_string())),
        }
    }

    fn internal_error(&self, exception: String) -> Option<Value> {
        error!(thread: &self.thread, "Error while handling rpc request\n{exception}");
        Some(JsonRpcErrors::InternalError.create_with_unknown_id(Some("Unknown error handling request - check server logs for stack trace")))
    }

    async fn handle_json_object(&self, object: &Map<String, Value>) -> Option<Value> {
        let id = object.get("id");
        let method = match object.get("method") {
            None => None,
            Some(value) => match gson::as_string(value) {
                Some(method) => Some(method),
                None => {
                    return self.internal_error(format!(
                        "com.google.gson.JsonSyntaxException: Expected method to be a string, was {}",
                        gson::type_name(Some(value))
                    ));
                }
            },
        };
        let result = object.get("result");
        let params = object.get("params");
        let error = match object.get("error") {
            None => None,
            Some(Value::Object(error)) => Some(error),
            Some(value) => {
                return self.internal_error(format!("com.google.gson.JsonSyntaxException: Expected error to be a JsonObject, was {}", gson::type_name(Some(value))));
            }
        };
        if let Some(method) = &method
            && result.is_none()
            && error.is_none()
        {
            if id.is_some_and(|id| !is_valid_request_id(id)) {
                return Some(JsonRpcErrors::InvalidRequest.create_with_unknown_id(Some("Invalid request id - only String, Number and NULL supported")));
            }
            return self.handle_incoming_request(id, method, params).await;
        }
        if method.is_none()
            && let Some(result) = result
            && error.is_none()
            && let Some(id) = id
        {
            match id {
                Value::Number(n) => warn!(thread: &self.thread, "Received unknown response (id: {}): {result}", json_int(n)),
                _ => warn!(thread: &self.thread, "Received respose {result} with id {id} we did not request"),
            }
            return None;
        }
        if method.is_none()
            && result.is_none()
            && let Some(error) = error
        {
            let id = id.map_or_else(|| "null".to_string(), Value::to_string);
            error!(thread: &self.thread, "Received error (id: {id}): {}", Value::Object(error.clone()));
            return None;
        }
        Some(JsonRpcErrors::InvalidRequest.create_without_data(id.cloned().unwrap_or(Value::Null)))
    }

    async fn handle_incoming_request(&self, id: Option<&Value>, method: &str, params: Option<&Value>) -> Option<Value> {
        let result = self.dispatch_incoming_request(method, params).await?;
        let id = id?.clone();
        Some(match result {
            Ok(result) => json_rpc_utils::create_success_result(id, result),
            Err(RpcFailure::InvalidParams(message)) => JsonRpcErrors::InvalidParams.create(id, &message),
            Err(RpcFailure::InvalidRequest(message)) => JsonRpcErrors::InvalidRequest.create(id, &message),
            Err(RpcFailure::MethodNotFound(message)) => JsonRpcErrors::MethodNotFound.create(id, &message),
        })
    }

    async fn dispatch_incoming_request(&self, method: &str, params: Option<&Value>) -> Option<Result<Value, RpcFailure>> {
        let Ok(identifier) = crate::resources::identifier::parse(method) else {
            return Some(Err(RpcFailure::InvalidRequest(format!("Failed to parse method value: {method}"))));
        };
        let Some(incoming) = incoming_rpc_methods::get(&identifier) else {
            return Some(Err(RpcFailure::MethodNotFound(format!("Method not found: {method}"))));
        };
        if !self.server.has_server() && !incoming.allow_pre_server_init {
            return Some(Err(RpcFailure::InvalidRequest(format!("Method cannot be dispatched pre server initialization: {method}"))));
        }
        if !incoming.run_on_main_thread {
            return Some(incoming.apply(None, params, self.client));
        }
        let params = params.cloned();
        let client = self.client;
        let reply = self.server.submit(move |server| incoming.apply(Some(server), params.as_ref(), client))?;
        reply.await.ok()
    }
}

pub struct ReadBatch<S> {
    stream: S,
    open: bool,
}

impl<S: AsyncRead + Unpin> AsyncRead for ReadBatch<S> {
    fn poll_read(mut self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut ReadBuf<'_>) -> Poll<std::io::Result<()>> {
        if !self.open {
            return Poll::Pending;
        }
        let requested = buf.remaining();
        let before = buf.filled().len();
        let result = Pin::new(&mut self.stream).poll_read(cx, buf);
        if result.is_ready() && buf.filled().len() - before < requested {
            self.open = false;
        }
        result
    }
}

impl<S: AsyncWrite + Unpin> AsyncWrite for ReadBatch<S> {
    fn poll_write(mut self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<std::io::Result<usize>> {
        Pin::new(&mut self.stream).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.stream).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.stream).poll_shutdown(cx)
    }
}

fn json_int(number: &serde_json::Number) -> i32 {
    if let Some(v) = number.as_i64() {
        v as i32
    } else if let Some(v) = number.as_u64() {
        v as i32
    } else {
        number.as_f64().unwrap_or(0.0).trunc() as i128 as i32
    }
}

fn is_valid_request_id(id: &Value) -> bool {
    matches!(id, Value::Null | Value::Number(_) | Value::String(_))
}

fn handshake(request: &HttpRequest, token_in_protocol: bool) -> Result<Option<Handshake>, String> {
    let protocol = if token_in_protocol { "sec-websocket-protocol: minecraft-v1\r\n" } else { "" };
    match request.header("sec-websocket-version") {
        Some("13" | "8" | "7") => {
            if request.method != "GET" {
                return Err(format!("Invalid WebSocket handshake method: {}", request.method));
            }
            if !request.contains_token("connection", "upgrade") {
                return Err("not a WebSocket request: a |Connection| header must include a token 'Upgrade'".into());
            }
            if !request.contains_value("upgrade", "websocket") {
                return Err("not a WebSocket request: an |Upgrade| header must containing the value 'websocket'".into());
            }
            let Some(key) = request.header("sec-websocket-key") else { return Err("not a WebSocket request: missing key".into()) };
            let mut sha1 = Sha1::new();
            sha1.update(key.as_bytes());
            sha1.update(WEBSOCKET_GUID.as_bytes());
            let accept = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, sha1.finalize());
            Ok(Some(Handshake::Rfc6455(format!(
                "HTTP/1.1 101 Switching Protocols\r\nupgrade: websocket\r\nconnection: upgrade\r\nsec-websocket-accept: {accept}\r\n{protocol}\r\n"
            ))))
        }
        Some(_) => Ok(None),
        None => hixie_handshake(request, protocol).map(|h| Some(Handshake::Hixie(h))),
    }
}

fn hixie_handshake(request: &HttpRequest, protocol: &str) -> Result<Vec<u8>, String> {
    if request.method != "GET" {
        return Err(format!("Invalid WebSocket handshake method: {}", request.method));
    }
    if !request.contains_token("connection", "upgrade") || !request.header("upgrade").is_some_and(|u| u.eq_ignore_ascii_case("websocket")) {
        return Err("not a WebSocket handshake request: missing upgrade".into());
    }
    let hixie76 = request.has_header("sec-websocket-key1") && request.has_header("sec-websocket-key2");
    let origin = request.header("origin");
    let Some(origin) = origin.or(hixie76.then_some("")) else { return Err(format!("Missing origin header, got only {}", request.names())) };
    let location = format!("ws://{}/", request.header("host").unwrap_or("null"));
    let mut response = Vec::new();
    if hixie76 {
        let key = |name: &str| -> Option<u32> {
            let value = request.header(name)?;
            let digits: String = value.chars().filter(char::is_ascii_digit).collect();
            let spaces = value.chars().filter(|&c| c == ' ').count() as u64;
            Some((digits.parse::<u64>().ok()? / spaces.max(1)) as u32)
        };
        let (Some(a), Some(b)) = (key("sec-websocket-key1"), key("sec-websocket-key2")) else { return Err(String::new()) };
        let Some(c) = request.content.get(..8) else { return Err(String::new()) };
        let mut input = Vec::with_capacity(16);
        input.extend_from_slice(&a.to_be_bytes());
        input.extend_from_slice(&b.to_be_bytes());
        input.extend_from_slice(c);
        let challenge = <md5::Md5 as md5::Digest>::digest(&input);
        response.extend_from_slice(
            format!(
                "HTTP/1.1 101 WebSocket Protocol Handshake\r\nupgrade: websocket\r\nconnection: upgrade\r\nsec-websocket-origin: {origin}\r\nsec-websocket-location: {location}\r\n{protocol}\r\n"
            )
            .as_bytes(),
        );
        response.extend_from_slice(&challenge);
    } else {
        response.extend_from_slice(
            format!(
                "HTTP/1.1 101 Web Socket Protocol Handshake\r\nupgrade: websocket\r\nconnection: upgrade\r\nwebsocket-origin: {origin}\r\nwebsocket-location: {location}\r\n{protocol}\r\n"
            )
            .as_bytes(),
        );
    }
    Ok(response)
}

enum HixieFrame {
    Text(String),
    Close,
    Skip,
    TooLong,
}

fn take_hixie_frame(buffer: &mut Vec<u8>) -> Option<HixieFrame> {
    let kind = *buffer.first()?;
    if kind & 0x80 == 0x80 {
        let mut length: usize = 0;
        let mut i = 1;
        loop {
            let b = *buffer.get(i)?;
            length = (length << 7) | (b & 0x7F) as usize;
            i += 1;
            if b & 0x80 == 0 {
                break;
            }
        }
        if kind == 0xFF && length == 0 {
            buffer.drain(..i);
            return Some(HixieFrame::Close);
        }
        if buffer.len() < i + length {
            return None;
        }
        buffer.drain(..i + length);
        return Some(HixieFrame::Skip);
    }
    match buffer[1..].iter().position(|&b| b == 0xFF) {
        Some(end) => {
            if end > HIXIE_MAX_FRAME_SIZE {
                return Some(HixieFrame::TooLong);
            }
            let text = String::from_utf8_lossy(&buffer[1..1 + end]).into_owned();
            buffer.drain(..end + 2);
            Some(HixieFrame::Text(text))
        }
        None if buffer.len() - 1 > HIXIE_MAX_FRAME_SIZE => Some(HixieFrame::TooLong),
        None => None,
    }
}

async fn write_hixie<S: AsyncWrite + Unpin>(stream: &mut S, text: &str) -> std::io::Result<()> {
    let mut frame = Vec::with_capacity(text.len() + 2);
    frame.push(0x00);
    frame.extend_from_slice(text.as_bytes());
    frame.push(0xFF);
    stream.write_all(&frame).await
}
