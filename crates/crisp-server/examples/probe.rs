use std::time::{Duration, Instant};

use crisp_data::packets::{configuration, handshake, login, play};
use crisp_data::{PROTOCOL_VERSION, VERSION_NAME};
use crisp_protocol::nbt::Nbt;
use crisp_protocol::{Codec, Reader, Writer};
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;

struct Client {
    stream: TcpStream,
    codec: Codec,
}

impl Client {
    async fn send(&mut self, id: i32, body: impl FnOnce(&mut Writer)) {
        let mut w = Writer::new();
        w.varint(id);
        body(&mut w);
        self.stream.write_all(&self.codec.encode(w.as_slice())).await.unwrap();
    }

    async fn recv(&mut self) -> (i32, Vec<u8>) {
        let bytes = self.codec.read(&mut self.stream).await.unwrap();
        let mut r = Reader::new(&bytes);
        let id = r.varint().unwrap();
        (id, bytes[bytes.len() - r.remaining()..].to_vec())
    }
}

fn text(nbt: &Nbt) -> String {
    match nbt {
        Nbt::String(s) => s.clone(),
        Nbt::List(l) => l.iter().map(text).collect(),
        Nbt::Compound(_) => {
            let mut out = String::new();
            if let Some(t) = nbt.get("text").and_then(Nbt::as_str) {
                out.push_str(t);
            }
            if let Some(key) = nbt.get("translate").and_then(Nbt::as_str) {
                let args: Vec<String> = nbt.get("with").map(|w| w.as_list().iter().map(text).collect()).unwrap_or_default();
                out.push_str(&format!("{{{key}{}}}", if args.is_empty() { String::new() } else { format!(": {}", args.join(" | ")) }));
            }
            if let Some(extra) = nbt.get("extra") {
                out.push_str(&text(extra));
            }
            out
        }
        other => format!("{other:?}"),
    }
}

#[tokio::main]
async fn main() {
    let addr = std::env::args().nth(1).unwrap_or_else(|| "127.0.0.1:25599".into());
    let commands: Vec<String> = std::env::args().skip(2).collect();
    let mut c = Client { stream: TcpStream::connect(&addr).await.unwrap(), codec: Codec::default() };
    c.send(handshake::serverbound::INTENTION, |w| {
        w.varint(PROTOCOL_VERSION);
        w.string("localhost");
        w.u16(25599);
        w.varint(2);
    })
    .await;
    c.send(login::serverbound::HELLO, |w| {
        w.string("Probe");
        w.uuid(0);
    })
    .await;
    loop {
        let (id, data) = c.recv().await;
        if id == login::clientbound::LOGIN_COMPRESSION {
            c.codec.threshold = Some(Reader::new(&data).varint().unwrap() as usize);
        } else if id == login::clientbound::LOGIN_FINISHED {
            break;
        } else if id == login::clientbound::LOGIN_DISCONNECT {
            println!("refused: {}", String::from_utf8_lossy(&data));
            return;
        }
    }
    c.send(login::serverbound::LOGIN_ACKNOWLEDGED, |_| {}).await;
    c.send(configuration::serverbound::CLIENT_INFORMATION, |w| {
        w.string("en_us");
        w.u8(8);
        w.varint(0);
        w.bool(true);
        w.u8(0x7F);
        w.varint(1);
        w.bool(false);
        w.bool(true);
        w.varint(0);
    })
    .await;
    let mut configuration_packets = 0;
    loop {
        let (id, data) = c.recv().await;
        if let Ok(dir) = std::env::var("PROBE_DUMP_CONFIGURATION") {
            configuration_packets += 1;
            let mut w = Writer::new();
            w.varint(id);
            w.bytes(&data);
            std::fs::write(format!("{dir}/{configuration_packets:03}_{id}.bin"), w.as_slice()).unwrap();
        }
        if id == configuration::clientbound::CODE_OF_CONDUCT {
            println!("code of conduct: {:?}", Reader::new(&data).string(32767).unwrap());
            c.send(configuration::serverbound::ACCEPT_CODE_OF_CONDUCT, |_| {}).await;
        } else if id == configuration::clientbound::RESOURCE_PACK_PUSH {
            let pack = Reader::new(&data).uuid().unwrap();
            let action = std::env::var("PROBE_RESOURCE_PACK_ACTIONS").unwrap_or_else(|_| "3,4,0".into());
            for action in action.split(',') {
                c.send(configuration::serverbound::RESOURCE_PACK, |w| {
                    w.uuid(pack);
                    w.varint(action.parse().unwrap());
                })
                .await;
            }
        } else if id == configuration::clientbound::DISCONNECT {
            println!("disconnected: {:?}", Nbt::read(&mut Reader::new(&data)).map(|n| text(&n)));
            return;
        } else if id == configuration::clientbound::SELECT_KNOWN_PACKS {
            c.send(configuration::serverbound::SELECT_KNOWN_PACKS, |w| {
                w.varint(1);
                w.string("minecraft");
                w.string("core");
                w.string(VERSION_NAME);
            })
            .await;
        } else if id == configuration::clientbound::FINISH_CONFIGURATION {
            break;
        }
    }
    c.send(configuration::serverbound::FINISH_CONFIGURATION, |_| {}).await;
    let joined = Instant::now();
    println!("joined");

    let mut chunks = 0;
    let mut loaded = false;
    let mut next_command = 0;
    let mut pos = (0.0, 0.0, 0.0);
    let mut counts = std::collections::BTreeMap::new();
    let mut changed: std::collections::HashMap<u32, u32> = std::collections::HashMap::new();
    let deadline = Instant::now() + Duration::from_secs(25 + 2 * commands.len() as u64);
    while Instant::now() < deadline {
        if loaded && next_command < commands.len() && joined.elapsed() > Duration::from_secs(4 + 2 * next_command as u64) {
            let command = &commands[next_command];
            println!("> /{command}");
            c.send(play::serverbound::CHAT_COMMAND, |w| w.string(command)).await;
            next_command += 1;
        }
        let Ok((id, data)) = tokio::time::timeout(Duration::from_millis(200), c.recv()).await else { continue };
        *counts.entry(id).or_insert(0) += 1;
        if let Ok(dir) = std::env::var("PROBE_DUMP_ALL") {
            let mut w = Writer::new();
            w.varint(id);
            w.bytes(&data);
            std::fs::write(format!("{dir}/{:06}_{id}.bin", counts.values().sum::<i32>()), w.as_slice()).unwrap();
        }
        let mut r = Reader::new(&data);
        if id == play::clientbound::LEVEL_CHUNK_WITH_LIGHT
            && let Ok(dir) = std::env::var("PROBE_DUMP")
        {
            let (x, z) = (r.i32().unwrap(), r.i32().unwrap());
            std::fs::write(format!("{dir}/chunk_{x}_{z}.bin"), &data).unwrap();
        } else if id == play::clientbound::SECTION_BLOCKS_UPDATE {
            r.i64().unwrap();
            let n = r.varint().unwrap();
            for _ in 0..n {
                let v = r.varlong().unwrap();
                *changed.entry((v >> 12) as u32).or_insert(0) += 1;
            }
        } else if id == play::clientbound::BLOCK_UPDATE {
            r.i64().unwrap();
            *changed.entry(r.varint().unwrap() as u32).or_insert(0) += 1;
        } else         if id == play::clientbound::KEEP_ALIVE {
            let v = r.i64().unwrap();
            c.send(play::serverbound::KEEP_ALIVE, |w| w.i64(v)).await;
        } else if id == play::clientbound::PLAYER_POSITION {
            let teleport = r.varint().unwrap();
            pos = (r.f64().unwrap(), r.f64().unwrap(), r.f64().unwrap());
            println!("teleported to {pos:?}");
            c.send(play::serverbound::ACCEPT_TELEPORTATION, |w| {
                w.varint(teleport);
                w.f64(pos.0);
                w.f64(pos.1);
                w.f64(pos.2);
                w.f32(0.0);
                w.f32(0.0);
            })
            .await;
        } else if id == play::clientbound::CHUNK_BATCH_FINISHED {
            chunks += r.varint().unwrap();
            c.send(play::serverbound::CHUNK_BATCH_RECEIVED, |w| w.f32(64.0)).await;
            if !loaded && chunks >= 9 {
                loaded = true;
                c.send(play::serverbound::PLAYER_LOADED, |_| {}).await;
                println!("loaded after {:?}, {chunks} chunks", joined.elapsed());
            }
        } else if id == play::clientbound::SYSTEM_CHAT {
            if let Ok(nbt) = Nbt::read(&mut r) {
                println!("chat: {}", text(&nbt));
            }
        } else if id == play::clientbound::DISCONNECT {
            println!("disconnected: {:?}", Nbt::read(&mut r).map(|n| text(&n)));
            return;
        }
    }
    println!("{chunks} chunks; packets by id: {counts:?}");
    let mut changed: Vec<_> = changed.into_iter().collect();
    changed.sort_by_key(|c| std::cmp::Reverse(c.1));
    for (state, n) in changed.into_iter().take(12) {
        println!("{n:6} x {:?}", crisp_data::BlockState::by_id(state));
    }
}
