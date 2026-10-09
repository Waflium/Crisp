use aes::Aes128;
use cfb8::cipher::generic_array::GenericArray;
use cfb8::cipher::{BlockDecryptMut, BlockEncryptMut, KeyIvInit};
use crisp_data::PROTOCOL_VERSION;
use crisp_data::packets::{handshake, login};
use crisp_protocol::{Codec, Reader, Writer};
use crisp_server::{Listen, ServerConfig, serve};
use rsa::pkcs8::DecodePublicKey;
use rsa::{Pkcs1v15Encrypt, RsaPublicKey};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

struct Client {
    stream: TcpStream,
    codec: Codec,
    buf: Vec<u8>,
    cipher: Option<(cfb8::Encryptor<Aes128>, cfb8::Decryptor<Aes128>)>,
}

impl Client {
    async fn send(&mut self, id: i32, body: impl FnOnce(&mut Writer)) {
        let mut w = Writer::new();
        w.varint(id);
        body(&mut w);
        let mut frame = self.codec.encode(w.as_slice());
        if let Some((enc, _)) = &mut self.cipher {
            frame.chunks_exact_mut(1).for_each(|b| enc.encrypt_block_mut(GenericArray::from_mut_slice(b)));
        }
        self.stream.write_all(&frame).await.unwrap();
    }

    async fn recv(&mut self) -> (i32, Vec<u8>) {
        loop {
            if let Some((bytes, used)) = self.codec.decode_frame(&self.buf).unwrap() {
                self.buf.drain(..used);
                let mut r = Reader::new(&bytes);
                let id = r.varint().unwrap();
                return (id, bytes[bytes.len() - r.remaining()..].to_vec());
            }
            let mut chunk = [0u8; 4096];
            let n = self.stream.read(&mut chunk).await.unwrap();
            assert!(n > 0, "connection closed");
            let chunk = &mut chunk[..n];
            if let Some((_, dec)) = &mut self.cipher {
                chunk.chunks_exact_mut(1).for_each(|b| dec.decrypt_block_mut(GenericArray::from_mut_slice(b)));
            }
            self.buf.extend_from_slice(chunk);
        }
    }
}

#[tokio::test]
async fn online_mode_encrypts_then_asks_the_session_server() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(serve(Listen::Bound(listener), ServerConfig::default(), crisp_server::server::services::Services::create(), None, false));

    let mut c = Client { stream: TcpStream::connect(addr).await.unwrap(), codec: Codec::default(), buf: Vec::new(), cipher: None };
    c.send(handshake::serverbound::INTENTION, |w| {
        w.varint(PROTOCOL_VERSION);
        w.string("localhost");
        w.u16(addr.port());
        w.varint(2);
    })
    .await;
    c.send(login::serverbound::HELLO, |w| {
        w.string("Steve");
        w.uuid(0);
    })
    .await;

    let (id, data) = c.recv().await;
    assert_eq!(id, login::clientbound::HELLO);
    let mut r = Reader::new(&data);
    assert_eq!(r.string(20).unwrap(), "");
    let key_len = r.varint().unwrap() as usize;
    let public_key = RsaPublicKey::from_public_key_der(r.take(key_len).unwrap()).unwrap();
    let challenge_len = r.varint().unwrap() as usize;
    let challenge = r.take(challenge_len).unwrap().to_vec();
    assert_eq!(challenge.len(), 4);
    assert!(r.bool().unwrap(), "should authenticate");

    let secret = [42u8; 16];
    let mut rng = rand_core::OsRng;
    let encrypted_secret = public_key.encrypt(&mut rng, Pkcs1v15Encrypt, &secret).unwrap();
    let encrypted_challenge = public_key.encrypt(&mut rng, Pkcs1v15Encrypt, &challenge).unwrap();
    c.send(login::serverbound::KEY, |w| {
        w.varint(encrypted_secret.len() as i32);
        w.bytes(&encrypted_secret);
        w.varint(encrypted_challenge.len() as i32);
        w.bytes(&encrypted_challenge);
    })
    .await;
    c.cipher = Some((cfb8::Encryptor::new(&secret.into(), &secret.into()), cfb8::Decryptor::new(&secret.into(), &secret.into())));

    let (id, data) = c.recv().await;
    assert_eq!(id, login::clientbound::LOGIN_DISCONNECT);
    let reason: serde_json::Value = serde_json::from_str(Reader::new(&data).string(262144).unwrap()).unwrap();
    let key = reason["translate"].as_str().unwrap();
    assert!(key == "multiplayer.disconnect.unverified_username" || key == "multiplayer.disconnect.authservers_down", "{reason}");
}
