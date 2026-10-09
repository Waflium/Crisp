use std::io::{self, Read, Write};
use std::sync::atomic::{AtomicU8, Ordering};

use crisp_core::fastutil::Object2ObjectOpenHashMap;
use flate2::Compression;
use flate2::read::{GzDecoder, ZlibDecoder};
use flate2::write::{GzEncoder, ZlibEncoder};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RegionFileVersion {
    Gzip = 1,
    Deflate = 2,
    None = 3,
    Lz4 = 4,
}

static SELECTED: AtomicU8 = AtomicU8::new(RegionFileVersion::Deflate as u8);

impl RegionFileVersion {
    pub fn from_id(id: u8) -> Option<RegionFileVersion> {
        Some(match id {
            1 => RegionFileVersion::Gzip,
            2 => RegionFileVersion::Deflate,
            3 => RegionFileVersion::None,
            4 => RegionFileVersion::Lz4,
            _ => return None,
        })
    }

    pub fn id(self) -> u8 {
        self as u8
    }

    pub fn configure(option_name: &str) {
        let mut by_name = Object2ObjectOpenHashMap::new(16, 0.75);
        by_name.insert("deflate".to_string(), RegionFileVersion::Deflate);
        by_name.insert("none".to_string(), RegionFileVersion::None);
        by_name.insert("lz4".to_string(), RegionFileVersion::Lz4);
        match by_name.get(option_name) {
            Some(version) => SELECTED.store(version.id(), Ordering::Relaxed),
            None => {
                let names: Vec<&str> = by_name.iter().map(|(name, _)| name).collect();
                error!("Invalid `region-file-compression` value `{option_name}` in server.properties. Please use one of: {}", names.join(", "));
            }
        }
    }

    pub fn selected() -> RegionFileVersion {
        RegionFileVersion::from_id(SELECTED.load(Ordering::Relaxed)).unwrap_or(RegionFileVersion::Deflate)
    }

    pub fn decompress(self, data: &[u8]) -> io::Result<Vec<u8>> {
        let mut bytes = Vec::new();
        match self {
            RegionFileVersion::Gzip => {
                GzDecoder::new(data).read_to_end(&mut bytes)?;
            }
            RegionFileVersion::Deflate => {
                ZlibDecoder::new(data).read_to_end(&mut bytes)?;
            }
            RegionFileVersion::None => bytes.extend_from_slice(data),
            RegionFileVersion::Lz4 => bytes = lz4_block::decompress(data)?,
        }
        Ok(bytes)
    }

    pub fn compress(self, data: &[u8]) -> io::Result<Vec<u8>> {
        Ok(match self {
            RegionFileVersion::Gzip => {
                let mut out = GzEncoder::new(Vec::new(), Compression::default());
                out.write_all(data)?;
                out.finish()?
            }
            RegionFileVersion::Deflate => {
                let mut out = ZlibEncoder::new(Vec::new(), Compression::default());
                out.write_all(data)?;
                out.finish()?
            }
            RegionFileVersion::None => data.to_vec(),
            RegionFileVersion::Lz4 => lz4_block::compress(data),
        })
    }
}

mod lz4_block {
    use std::io;

    const MAGIC: &[u8; 8] = b"LZ4Block";
    const HEADER_LENGTH: usize = MAGIC.len() + 1 + 4 + 4 + 4;
    const COMPRESSION_METHOD_RAW: u8 = 0x10;
    const COMPRESSION_METHOD_LZ4: u8 = 0x20;
    const BLOCK_SIZE: usize = 1 << 16;
    const COMPRESSION_LEVEL: u8 = 16 - 10;
    const DEFAULT_SEED: u32 = 0x9747_b28c;

    fn checksum(data: &[u8]) -> u32 {
        xxh32(data, DEFAULT_SEED) & 0x0fff_ffff
    }

    pub fn compress(data: &[u8]) -> Vec<u8> {
        let mut out = Vec::with_capacity(data.len() / 2 + HEADER_LENGTH * 2);
        for block in data.chunks(BLOCK_SIZE) {
            let compressed = lz4_flex::block::compress(block);
            let (method, payload) =
                if compressed.len() >= block.len() { (COMPRESSION_METHOD_RAW, block) } else { (COMPRESSION_METHOD_LZ4, compressed.as_slice()) };
            out.extend_from_slice(MAGIC);
            out.push(method | COMPRESSION_LEVEL);
            out.extend((payload.len() as i32).to_le_bytes());
            out.extend((block.len() as i32).to_le_bytes());
            out.extend(checksum(block).to_le_bytes());
            out.extend_from_slice(payload);
        }
        out.extend_from_slice(MAGIC);
        out.push(COMPRESSION_METHOD_RAW | COMPRESSION_LEVEL);
        out.extend([0u8; 12]);
        out
    }

    pub fn decompress(mut data: &[u8]) -> io::Result<Vec<u8>> {
        let corrupted = || io::Error::new(io::ErrorKind::InvalidData, "Stream is corrupted");
        let mut out = Vec::new();
        while !data.is_empty() {
            if data.len() < HEADER_LENGTH || &data[..8] != MAGIC {
                return Err(corrupted());
            }
            let token = data[8];
            let int = |at: usize| i32::from_le_bytes(data[at..at + 4].try_into().unwrap());
            let (compressed_length, original_length, check) = (int(9), int(13), int(17) as u32);
            if compressed_length < 0 || original_length < 0 {
                return Err(corrupted());
            }
            let (compressed_length, original_length) = (compressed_length as usize, original_length as usize);
            data = &data[HEADER_LENGTH..];
            if original_length == 0 && compressed_length == 0 {
                if check != 0 {
                    return Err(corrupted());
                }
                break;
            }
            if data.len() < compressed_length {
                return Err(corrupted());
            }
            let start = out.len();
            match token & 0xf0 {
                COMPRESSION_METHOD_RAW => out.extend_from_slice(&data[..compressed_length]),
                COMPRESSION_METHOD_LZ4 => {
                    let block = lz4_flex::block::decompress(&data[..compressed_length], original_length).map_err(|_| corrupted())?;
                    out.extend(block);
                }
                _ => return Err(corrupted()),
            }
            if out.len() - start != original_length || checksum(&out[start..]) != check {
                return Err(corrupted());
            }
            data = &data[compressed_length..];
        }
        Ok(out)
    }

    fn xxh32(input: &[u8], seed: u32) -> u32 {
        const P1: u32 = 2_654_435_761;
        const P2: u32 = 2_246_822_519;
        const P3: u32 = 3_266_489_917;
        const P4: u32 = 668_265_263;
        const P5: u32 = 374_761_393;
        let read = |at: usize| u32::from_le_bytes(input[at..at + 4].try_into().unwrap());
        let round = |acc: u32, lane: u32| acc.wrapping_add(lane.wrapping_mul(P2)).rotate_left(13).wrapping_mul(P1);
        let mut offset = 0;
        let mut hash = if input.len() >= 16 {
            let mut v = [seed.wrapping_add(P1).wrapping_add(P2), seed.wrapping_add(P2), seed, seed.wrapping_sub(P1)];
            while offset + 16 <= input.len() {
                for (lane, value) in v.iter_mut().enumerate() {
                    *value = round(*value, read(offset + lane * 4));
                }
                offset += 16;
            }
            v[0].rotate_left(1).wrapping_add(v[1].rotate_left(7)).wrapping_add(v[2].rotate_left(12)).wrapping_add(v[3].rotate_left(18))
        } else {
            seed.wrapping_add(P5)
        };
        hash = hash.wrapping_add(input.len() as u32);
        while offset + 4 <= input.len() {
            hash = hash.wrapping_add(read(offset).wrapping_mul(P3)).rotate_left(17).wrapping_mul(P4);
            offset += 4;
        }
        for &byte in &input[offset..] {
            hash = hash.wrapping_add((byte as u32).wrapping_mul(P5)).rotate_left(11).wrapping_mul(P1);
        }
        hash ^= hash >> 15;
        hash = hash.wrapping_mul(P2);
        hash ^= hash >> 13;
        hash = hash.wrapping_mul(P3);
        hash ^ (hash >> 16)
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn xxh32_matches_the_reference() {
            assert_eq!(xxh32(b"", 0), 0x02cc5d05);
            assert_eq!(xxh32(b"Nobody inspects the spammish repetition", 0), 0xe2293b2f);
        }

        #[test]
        fn blocks_round_trip() {
            let data: Vec<u8> = (0..200_000u32).map(|i| (i % 251) as u8).collect();
            assert_eq!(decompress(&compress(&data)).unwrap(), data);
            assert_eq!(decompress(&compress(b"")).unwrap(), b"");
        }
    }
}