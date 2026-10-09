use crisp_data::packets::{configuration, handshake, login, play, status};
use crisp_data::{PROTOCOL_VERSION, SYNCED_REGISTRIES, VERSION_NAME};
use crisp_protocol::{Codec, Reader, Writer};
use crisp_server::{LevelType, Listen, ServerConfig, serve};
use tokio::io::AsyncWriteExt;
use tokio::net::{TcpListener, TcpStream};

struct Client {
    stream: TcpStream,
    codec: Codec,
}

impl Client {
    async fn connect(addr: std::net::SocketAddr, protocol: i32, intent: i32) -> Client {
        let mut client = Client { stream: TcpStream::connect(addr).await.unwrap(), codec: Codec::default() };
        client
            .send(handshake::serverbound::INTENTION, |w| {
                w.varint(protocol);
                w.string("localhost");
                w.u16(addr.port());
                w.varint(intent);
            })
            .await;
        client
    }

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

    async fn recv_until(&mut self, id: i32) -> (Vec<u8>, Vec<i32>) {
        let mut skipped = Vec::new();
        loop {
            let (got, data) = self.recv().await;
            if got == id {
                return (data, skipped);
            }
            skipped.push(got);
        }
    }
}

async fn start() -> std::net::SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let config = ServerConfig { motd: "test motd".into(), online_mode: false, white_list: false, level_type: LevelType::Flat, ..ServerConfig::default() };
    tokio::spawn(serve(Listen::Bound(listener), config, crisp_server::server::services::Services::create(), None, false));
    addr
}

#[tokio::test]
async fn status_and_ping() {
    let addr = start().await;
    let (mut c, id, data) = loop {
        let mut c = Client::connect(addr, PROTOCOL_VERSION, 1).await;
        c.send(status::serverbound::STATUS_REQUEST, |_| {}).await;
        if let Ok(bytes) = c.codec.read(&mut c.stream).await {
            let mut r = Reader::new(&bytes);
            let id = r.varint().unwrap();
            break (c, id, bytes[bytes.len() - r.remaining()..].to_vec());
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    };
    assert_eq!(id, status::clientbound::STATUS_RESPONSE);
    let status: serde_json::Value = serde_json::from_str(Reader::new(&data).string(32767).unwrap()).unwrap();
    assert_eq!(status["version"]["name"], VERSION_NAME);
    assert_eq!(status["version"]["protocol"], PROTOCOL_VERSION);
    assert_eq!(status["description"], "test motd");
    assert_eq!(status["players"], serde_json::json!({ "max": 20, "online": 0 }));
    assert!(status.get("enforcesSecureChat").is_none());

    c.send(status::serverbound::PING_REQUEST, |w| w.i64(0x0123_4567_89ab_cdef)).await;
    let (id, data) = c.recv().await;
    assert_eq!(id, status::clientbound::PONG_RESPONSE);
    assert_eq!(Reader::new(&data).i64().unwrap(), 0x0123_4567_89ab_cdef);
}

#[tokio::test]
async fn outdated_client_is_refused() {
    let addr = start().await;
    for (protocol, key) in [(PROTOCOL_VERSION - 1, "multiplayer.disconnect.incompatible"), (753, "multiplayer.disconnect.outdated_client")] {
        let mut c = Client::connect(addr, protocol, 2).await;
        let (id, data) = c.recv().await;
        assert_eq!(id, login::clientbound::LOGIN_DISCONNECT);
        let reason: serde_json::Value = serde_json::from_str(Reader::new(&data).string(262144).unwrap()).unwrap();
        assert_eq!(reason, serde_json::json!({ "translate": key, "with": [VERSION_NAME] }));
    }
}

#[tokio::test]
async fn transfers_are_refused_unless_accepted() {
    let addr = start().await;
    let mut c = Client::connect(addr, PROTOCOL_VERSION, 3).await;
    let (id, data) = c.recv().await;
    assert_eq!(id, login::clientbound::LOGIN_DISCONNECT);
    let reason: serde_json::Value = serde_json::from_str(Reader::new(&data).string(262144).unwrap()).unwrap();
    assert_eq!(reason, serde_json::json!({ "translate": "multiplayer.disconnect.transfers_disabled" }));
}

#[tokio::test]
async fn joins_superflat_world() {
    let addr = start().await;
    let mut c = Client::connect(addr, PROTOCOL_VERSION, 2).await;

    c.send(login::serverbound::HELLO, |w| {
        w.string("Steve");
        w.uuid(0);
    })
    .await;
    let (id, data) = c.recv().await;
    assert_eq!(id, login::clientbound::LOGIN_COMPRESSION);
    c.codec.threshold = Some(Reader::new(&data).varint().unwrap() as usize);
    let (id, data) = c.recv().await;
    assert_eq!(id, login::clientbound::LOGIN_FINISHED);
    let mut r = Reader::new(&data);
    let uuid = r.uuid().unwrap();
    assert_eq!(r.string(16).unwrap(), "Steve");
    c.send(login::serverbound::LOGIN_ACKNOWLEDGED, |_| {}).await;

    c.send(configuration::serverbound::CLIENT_INFORMATION, |w| {
        w.string("en_us");
        w.u8(2);
        w.varint(0);
        w.bool(true);
        w.u8(0x7F);
        w.varint(1);
        w.bool(false);
        w.bool(true);
        w.varint(0);
    })
    .await;
    let (data, _) = c.recv_until(configuration::clientbound::SELECT_KNOWN_PACKS).await;
    let mut r = Reader::new(&data);
    assert_eq!(r.varint().unwrap(), 1);
    assert_eq!((r.string(64).unwrap(), r.string(64).unwrap(), r.string(64).unwrap()), ("minecraft", "core", VERSION_NAME));
    c.send(configuration::serverbound::SELECT_KNOWN_PACKS, |w| {
        w.varint(1);
        w.string("minecraft");
        w.string("core");
        w.string(VERSION_NAME);
    })
    .await;
    let mut registries = 0;
    loop {
        let (id, _) = c.recv().await;
        match id {
            configuration::clientbound::REGISTRY_DATA => registries += 1,
            configuration::clientbound::UPDATE_TAGS => {}
            configuration::clientbound::FINISH_CONFIGURATION => break,
            other => panic!("unexpected configuration packet 0x{other:02x}"),
        }
    }
    assert_eq!(registries, SYNCED_REGISTRIES.len());
    c.send(configuration::serverbound::FINISH_CONFIGURATION, |_| {}).await;

    let (id, _) = c.recv().await;
    assert_eq!(id, play::clientbound::LOGIN);
    let (data, _) = c.recv_until(play::clientbound::PLAYER_POSITION).await;
    let mut r = Reader::new(&data);
    let teleport = r.varint().unwrap();
    let (x, y, z) = (r.f64().unwrap(), r.f64().unwrap(), r.f64().unwrap());
    assert!((-9.5..=10.5).contains(&x) && y == -60.0 && (-9.5..=10.5).contains(&z) && x.rem_euclid(1.0) == 0.5 && z.rem_euclid(1.0) == 0.5, "spawned within the respawn radius: {x} {y} {z}");

    let (others, _) = c.recv_until(play::clientbound::PLAYER_INFO_UPDATE).await;
    assert_eq!(others, [0xFF, 0]);
    let (data, _) = c.recv_until(play::clientbound::PLAYER_INFO_UPDATE).await;
    let mut r = Reader::new(&data);
    r.u8().unwrap();
    assert_eq!(r.varint().unwrap(), 1);
    assert_eq!(r.uuid().unwrap(), uuid);
    let (data, _) = c.recv_until(play::clientbound::GAME_EVENT).await;
    assert_eq!(data[0], 13, "start waiting for chunks");

    c.recv_until(play::clientbound::CHUNK_BATCH_START).await;
    c.send(play::serverbound::ACCEPT_TELEPORTATION, |w| {
        w.varint(teleport);
        w.f64(x);
        w.f64(y);
        w.f64(z);
        w.f32(0.0);
        w.f32(0.0);
    })
    .await;

    let mut chunks = 0;
    let mut first = None;
    let mut batch_started = true;
    while chunks < 49 {
        if !batch_started {
            c.recv_until(play::clientbound::CHUNK_BATCH_START).await;
        }
        batch_started = false;
        let (data, skipped) = c.recv_until(play::clientbound::CHUNK_BATCH_FINISHED).await;
        let batch = Reader::new(&data).varint().unwrap();
        assert!(skipped.iter().all(|&id| id == play::clientbound::LEVEL_CHUNK_WITH_LIGHT));
        assert_eq!(skipped.len(), batch as usize);
        first.get_or_insert(batch);
        chunks += batch;
        c.send(play::serverbound::CHUNK_BATCH_RECEIVED, |w| w.f32(64.0)).await;
    }
    assert_eq!(first, Some(9));
    assert_eq!(chunks, 49, "view distance 2, as measured on vanilla");

    c.send(play::serverbound::PLAYER_LOADED, |_| {}).await;

    let target_x = if x < 0.0 { 0.5 } else if x < 8.0 { -0.5 } else { 16.5 };
    c.send(play::serverbound::MOVE_PLAYER_POS, |w| {
        w.f64(target_x);
        w.f64(y);
        w.f64(z);
        w.u8(1);
    })
    .await;
    let (data, _) = c.recv_until(play::clientbound::SET_CHUNK_CACHE_CENTER).await;
    let mut r = Reader::new(&data);
    assert_eq!((r.varint().unwrap(), r.varint().unwrap()), ((target_x.floor() as i32) >> 4, (z.floor() as i32) >> 4));
    let (_, skipped) = c.recv_until(play::clientbound::CHUNK_BATCH_FINISHED).await;
    let forgotten = skipped.iter().filter(|&&id| id == play::clientbound::FORGET_LEVEL_CHUNK).count();
    let loaded = skipped.iter().filter(|&&id| id == play::clientbound::LEVEL_CHUNK_WITH_LIGHT).count();
    assert!(loaded > 0);
    assert_eq!(forgotten, loaded, "the view area keeps its shape when it moves");
}

#[tokio::test]
async fn whitelist_refuses_strangers() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(serve(Listen::Bound(listener), ServerConfig { online_mode: false, ..ServerConfig::default() }, crisp_server::server::services::Services::create(), None, false));
    let mut c = Client::connect(addr, PROTOCOL_VERSION, 2).await;
    c.send(login::serverbound::HELLO, |w| {
        w.string("Steve");
        w.uuid(0);
    })
    .await;
    let (id, data) = c.recv().await;
    assert_eq!(id, login::clientbound::LOGIN_DISCONNECT);
    let reason: serde_json::Value = serde_json::from_str(Reader::new(&data).string(262144).unwrap()).unwrap();
    assert_eq!(reason, serde_json::json!({ "translate": "multiplayer.disconnect.not_whitelisted" }));
}
