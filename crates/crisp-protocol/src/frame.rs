use std::io::{self, Read, Write};

use flate2::Compression;
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::{Reader, Writer};

const MAX_UNCOMPRESSED_LEN: usize = 1 << 23;

fn invalid(msg: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg.to_owned())
}

async fn read_raw<R: AsyncRead + Unpin>(r: &mut R) -> io::Result<Vec<u8>> {
    let mut len: u32 = 0;
    for i in 0..3 {
        let byte = r.read_u8().await?;
        len |= ((byte & 0x7F) as u32) << (7 * i);
        if byte & 0x80 == 0 {
            let mut body = vec![0; len as usize];
            r.read_exact(&mut body).await?;
            return Ok(body);
        }
    }
    Err(invalid("packet length VarInt exceeds 3 bytes"))
}

pub async fn read_frame<R: AsyncRead + Unpin>(r: &mut R) -> io::Result<Vec<u8>> {
    read_raw(r).await
}

pub async fn write_frame<W: AsyncWrite + Unpin>(w: &mut W, packet: &[u8]) -> io::Result<()> {
    w.write_all(&Codec::default().encode(packet)).await
}

#[derive(Clone, Copy, Default)]
pub struct Codec {
    pub threshold: Option<usize>,
}

impl Codec {
    pub async fn read<R: AsyncRead + Unpin>(&self, r: &mut R) -> io::Result<Vec<u8>> {
        let frame = read_raw(r).await?;
        self.decode_body(frame)
    }

    pub fn decode_frame(&self, buf: &[u8]) -> io::Result<Option<(Vec<u8>, usize)>> {
        let mut len: usize = 0;
        for i in 0..3 {
            let Some(&byte) = buf.get(i) else { return Ok(None) };
            len |= ((byte & 0x7F) as usize) << (7 * i);
            if byte & 0x80 == 0 {
                let end = i + 1 + len;
                if buf.len() < end {
                    return Ok(None);
                }
                return Ok(Some((self.decode_body(buf[i + 1..end].to_vec())?, end)));
            }
        }
        Err(invalid("packet length VarInt exceeds 3 bytes"))
    }

    fn decode_body(&self, frame: Vec<u8>) -> io::Result<Vec<u8>> {
        let Some(threshold) = self.threshold else {
            return Ok(frame);
        };
        let mut reader = Reader::new(&frame);
        let data_len = reader.varint().map_err(|e| invalid(&e.to_string()))?;
        let rest = &frame[frame.len() - reader.remaining()..];
        if data_len == 0 {
            return Ok(rest.to_vec());
        }
        let data_len = usize::try_from(data_len).map_err(|_| invalid("negative data length"))?;
        if data_len < threshold {
            return Err(invalid("compressed packet is below the compression threshold"));
        }
        if data_len > MAX_UNCOMPRESSED_LEN {
            return Err(invalid("packet is too large"));
        }
        let mut packet = Vec::with_capacity(data_len);
        ZlibDecoder::new(rest).take(data_len as u64 + 1).read_to_end(&mut packet)?;
        if packet.len() != data_len {
            return Err(invalid("decompressed length does not match data length"));
        }
        Ok(packet)
    }

    pub fn encode(&self, packet: &[u8]) -> Vec<u8> {
        let mut body = Writer::new();
        match self.threshold {
            None => body.bytes(packet),
            Some(threshold) if packet.len() < threshold => {
                body.varint(0);
                body.bytes(packet);
            }
            Some(_) => {
                body.varint(packet.len() as i32);
                let mut z = ZlibEncoder::new(Vec::new(), Compression::default());
                z.write_all(packet).expect("writing to a Vec cannot fail");
                body.bytes(&z.finish().expect("writing to a Vec cannot fail"));
            }
        }
        let mut frame = Writer::new();
        frame.varint(body.as_slice().len() as i32);
        frame.bytes(body.as_slice());
        frame.into_inner()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn roundtrip(codec: Codec, packet: &[u8]) -> Vec<u8> {
        let frame = codec.encode(packet);
        codec.read(&mut frame.as_slice()).await.unwrap()
    }

    #[tokio::test]
    async fn compressed_roundtrip() {
        let codec = Codec { threshold: Some(256) };
        let small = vec![7u8; 10];
        let large: Vec<u8> = (0..5000u32).map(|i| (i % 13) as u8).collect();
        assert_eq!(roundtrip(codec, &small).await, small);
        assert_eq!(roundtrip(codec, &large).await, large);
        assert_eq!(&codec.encode(&small)[..2], &[11, 0]);
    }

    #[tokio::test]
    async fn rejects_compressed_packet_below_threshold() {
        let frame = Codec { threshold: Some(1) }.encode(&[1, 2, 3]);
        let err = Codec { threshold: Some(256) }.read(&mut frame.as_slice()).await.unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
    }
}
