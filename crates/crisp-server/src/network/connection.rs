use std::io;

use anyhow::bail;
use crisp_protocol::{Codec, Reader, Writer};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use std::pin::Pin;
use std::time::Duration;

use tokio::sync::mpsc;
use tokio::time::{Instant, Sleep};

use super::chat::Component;
use super::encryption::{Decryptor, Encryptor};

const READ_TIMEOUT: Duration = Duration::from_secs(30);

pub struct Packet {
    pub id: i32,
    pub data: Vec<u8>,
}

impl Packet {
    pub fn reader(&self) -> Reader<'_> {
        Reader::new(&self.data)
    }
}

pub struct PacketReader {
    half: OwnedReadHalf,
    pub codec: Codec,
    buf: Vec<u8>,
    decryptor: Option<Decryptor>,
    timeout: Pin<Box<Sleep>>,
}

impl PacketReader {
    pub async fn recv(&mut self) -> anyhow::Result<Packet> {
        loop {
            if let Some((bytes, used)) = self.codec.decode_frame(&self.buf)? {
                self.buf.drain(..used);
                let mut r = Reader::new(&bytes);
                let id = r.varint()?;
                let data = bytes[bytes.len() - r.remaining()..].to_vec();
                return Ok(Packet { id, data });
            }
            self.read().await?;
        }
    }

    pub async fn read(&mut self) -> io::Result<&[u8]> {
        let start = self.buf.len();
        self.buf.reserve(8192);
        let n = tokio::select! {
            n = self.half.read_buf(&mut self.buf) => n?,
            _ = &mut self.timeout => return Err(io::Error::from(io::ErrorKind::TimedOut)),
        };
        if n == 0 {
            return Err(io::Error::from(io::ErrorKind::UnexpectedEof));
        }
        self.timeout.as_mut().reset(Instant::now() + READ_TIMEOUT);
        if let Some(decryptor) = &mut self.decryptor {
            decryptor.apply(&mut self.buf[start..]);
        }
        Ok(&self.buf[start..])
    }

    pub fn enable_encryption(&mut self, secret: &[u8; 16]) {
        let mut decryptor = Decryptor::new(secret);
        decryptor.apply(&mut self.buf);
        self.decryptor = Some(decryptor);
    }

    pub async fn expect(&mut self, id: i32) -> anyhow::Result<Packet> {
        let packet = self.recv().await?;
        if packet.id != id {
            bail!("expected packet 0x{id:02x}, got 0x{:02x}", packet.id);
        }
        Ok(packet)
    }
}

pub struct PacketWriter {
    half: OwnedWriteHalf,
    pub codec: Codec,
    out: Vec<u8>,
    encryptor: Option<Encryptor>,
}

impl PacketWriter {
    pub fn queue(&mut self, id: i32, body: impl FnOnce(&mut Writer)) {
        let mut w = Writer::new();
        w.varint(id);
        body(&mut w);
        self.out.extend(self.codec.encode(w.as_slice()));
    }

    pub async fn flush(&mut self) -> io::Result<()> {
        if !self.out.is_empty() {
            if let Some(encryptor) = &mut self.encryptor {
                encryptor.apply(&mut self.out);
            }
            self.half.write_all(&self.out).await?;
            self.out.clear();
        }
        Ok(())
    }

    pub async fn send(&mut self, id: i32, body: impl FnOnce(&mut Writer)) -> io::Result<()> {
        self.queue(id, body);
        self.flush().await
    }

    pub async fn send_raw_and_close(&mut self, bytes: &[u8]) -> io::Result<()> {
        self.half.write_all(bytes).await?;
        self.half.shutdown().await
    }

    pub fn enable_encryption(&mut self, secret: &[u8; 16]) {
        debug_assert!(self.out.is_empty());
        self.encryptor = Some(Encryptor::new(secret));
    }
}

pub fn split(stream: TcpStream) -> io::Result<(PacketReader, PacketWriter)> {
    stream.set_nodelay(true)?;
    let (r, w) = stream.into_split();
    Ok((
        PacketReader { half: r, codec: Codec::default(), buf: Vec::new(), decryptor: None, timeout: Box::pin(tokio::time::sleep(READ_TIMEOUT)) },
        PacketWriter { half: w, codec: Codec::default(), out: Vec::new(), encryptor: None },
    ))
}

#[derive(Clone)]
pub struct Outbound(mpsc::UnboundedSender<Vec<u8>>);

impl Outbound {
    pub fn send(&self, id: i32, body: impl FnOnce(&mut Writer)) {
        let mut w = Writer::new();
        w.varint(id);
        body(&mut w);
        self.send_raw(w.into_inner());
    }

    pub fn send_raw(&self, packet: Vec<u8>) {
        let _ = self.0.send(packet);
    }
}

pub fn encode(id: i32, body: impl FnOnce(&mut Writer)) -> Vec<u8> {
    let mut w = Writer::new();
    w.varint(id);
    body(&mut w);
    w.into_inner()
}

pub fn disconnection_reason(error: &anyhow::Error) -> Component {
    match error.downcast_ref::<io::Error>().map(io::Error::kind) {
        Some(io::ErrorKind::TimedOut) => Component::translatable("disconnect.timeout"),
        _ => Component::translatable("multiplayer.disconnect.generic"),
    }
}

pub fn spawn_io(
    mut r: PacketReader,
    mut w: PacketWriter,
    on_packet: impl Fn(Result<Packet, Component>) + Send + 'static,
) -> Outbound {
    let (tx, mut rx) = mpsc::unbounded_channel::<Vec<u8>>();
    tokio::spawn(async move {
        loop {
            match r.recv().await {
                Ok(packet) => on_packet(Ok(packet)),
                Err(e) => return on_packet(Err(disconnection_reason(&e))),
            }
        }
    });
    tokio::spawn(async move {
        while let Some(first) = rx.recv().await {
            let mut frames = w.codec.encode(&first);
            while let Ok(packet) = rx.try_recv() {
                frames.extend(w.codec.encode(&packet));
            }
            if let Some(encryptor) = &mut w.encryptor {
                encryptor.apply(&mut frames);
            }
            if w.half.write_all(&frames).await.is_err() {
                break;
            }
        }
        let _ = w.half.shutdown().await;
    });
    Outbound(tx)
}

#[cfg(test)]
impl Outbound {
    pub fn detached() -> Outbound {
        Outbound(mpsc::unbounded_channel().0)
    }

    pub fn capture() -> (Outbound, mpsc::UnboundedReceiver<Vec<u8>>) {
        let (tx, rx) = mpsc::unbounded_channel();
        (Outbound(tx), rx)
    }
}
