use std::fmt::Write as _;
use std::io::Read;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use anyhow::Context;
use crisp_data::packet_name;
use crisp_data::packets::{configuration, handshake, login, play};
use crisp_protocol::Reader;
use flate2::read::ZlibDecoder;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;

struct Shared {
    state: &'static str,
    threshold: Option<usize>,
}

struct Record {
    t_ms: u64,
    serverbound: bool,
    state: &'static str,
    packet: Vec<u8>,
}

async fn read_frame(r: &mut OwnedReadHalf) -> anyhow::Result<(Vec<u8>, Vec<u8>)> {
    let mut raw = Vec::new();
    let mut len: u32 = 0;
    for i in 0..3 {
        let byte = r.read_u8().await?;
        raw.push(byte);
        len |= ((byte & 0x7F) as u32) << (7 * i);
        if byte & 0x80 == 0 {
            let mut body = vec![0; len as usize];
            r.read_exact(&mut body).await?;
            raw.extend_from_slice(&body);
            return Ok((raw, body));
        }
    }
    anyhow::bail!("bad frame length")
}

fn decode(body: &[u8], threshold: Option<usize>) -> anyhow::Result<Vec<u8>> {
    if threshold.is_none() {
        return Ok(body.to_vec());
    }
    let mut r = Reader::new(body);
    let data_len = r.varint()?;
    let rest = &body[body.len() - r.remaining()..];
    if data_len == 0 {
        return Ok(rest.to_vec());
    }
    let mut out = Vec::new();
    ZlibDecoder::new(rest).read_to_end(&mut out)?;
    Ok(out)
}

async fn pump(
    mut from: OwnedReadHalf,
    mut to: OwnedWriteHalf,
    serverbound: bool,
    shared: Arc<Mutex<Shared>>,
    start: Instant,
    log: mpsc::UnboundedSender<Record>,
) -> anyhow::Result<()> {
    loop {
        let (raw, body) = read_frame(&mut from).await?;
        to.write_all(&raw).await?;
        let mut s = shared.lock().unwrap();
        let packet = decode(&body, s.threshold)?;
        let id = Reader::new(&packet).varint()?;
        let _ = log.send(Record { t_ms: start.elapsed().as_millis() as u64, serverbound, state: s.state, packet: packet.clone() });

        match (s.state, serverbound, id) {
            ("handshake", true, handshake::serverbound::INTENTION) => {
                let mut r = Reader::new(&packet);
                r.varint()?;
                r.varint()?;
                r.string(255)?;
                r.u16()?;
                s.state = if r.varint()? == 1 { "status" } else { "login" };
            }
            ("login", false, login::clientbound::LOGIN_COMPRESSION) => {
                let mut r = Reader::new(&packet);
                r.varint()?;
                let threshold = r.varint()?;
                s.threshold = (threshold >= 0).then_some(threshold as usize);
            }
            ("login", true, login::serverbound::LOGIN_ACKNOWLEDGED) => s.state = "configuration",
            ("configuration", true, configuration::serverbound::FINISH_CONFIGURATION) => s.state = "play",
            ("play", true, play::serverbound::CONFIGURATION_ACKNOWLEDGED) => s.state = "configuration",
            _ => {}
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let [_, listen, server, out] = &args[..] else {
        anyhow::bail!("usage: proxy <listen host:port> <server host:port> <out.jsonl>");
    };
    let listener = TcpListener::bind(listen).await?;
    println!("proxying {listen} -> {server}, logging to {out}");

    let (log_tx, mut log_rx) = mpsc::unbounded_channel::<Record>();
    let out = out.clone();
    tokio::spawn(async move {
        let mut file = tokio::fs::File::create(&out).await.expect("log file");
        while let Some(rec) = log_rx.recv().await {
            let mut r = Reader::new(&rec.packet);
            let id = r.varint().unwrap_or(-1);
            let data = &rec.packet[rec.packet.len() - r.remaining()..];
            let direction = if rec.serverbound { "serverbound" } else { "clientbound" };
            let mut hex = String::with_capacity(data.len() * 2);
            data.iter().for_each(|b| write!(hex, "{b:02x}").unwrap());
            let line = serde_json::json!({
                "t_ms": rec.t_ms,
                "dir": direction,
                "state": rec.state,
                "id": id,
                "name": packet_name(rec.state, direction, id).unwrap_or("unknown"),
                "len": data.len(),
                "data": hex,
            });
            let _ = file.write_all(format!("{line}\n").as_bytes()).await;
            let _ = file.flush().await;
        }
    });

    loop {
        let (client, addr) = listener.accept().await?;
        let upstream = TcpStream::connect(server).await.context("connecting to the server")?;
        client.set_nodelay(true)?;
        upstream.set_nodelay(true)?;
        println!("{addr} connected");
        let shared = Arc::new(Mutex::new(Shared { state: "handshake", threshold: None }));
        let start = Instant::now();
        let (cr, cw) = client.into_split();
        let (sr, sw) = upstream.into_split();
        tokio::spawn(pump(cr, sw, true, shared.clone(), start, log_tx.clone()));
        tokio::spawn(pump(sr, cw, false, shared, start, log_tx.clone()));
    }
}
